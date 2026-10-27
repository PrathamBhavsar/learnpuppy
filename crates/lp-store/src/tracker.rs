//! `SqliteTrackerStore`: tracker tables in one WAL database, plus `sync-<device>.jsonl` replication.
use crate::db::{self, fts_query};
use crate::jsonl::{append, be, check_id, create_dir, read_all};
use lp_core::fsrs::Rating;
use lp_core::ports::*;
use rusqlite::{params, Connection, OptionalExtension};
use serde::de::DeserializeOwned;
use smol_str::SmolStr;
use std::path::PathBuf;
use std::sync::Mutex;

const MIGRATIONS: &[&str] = &["
CREATE TABLE ingest_cursors(source_key TEXT PRIMARY KEY, mtime_ms INTEGER NOT NULL, size_bytes INTEGER NOT NULL);
CREATE TABLE sessions(id TEXT PRIMARY KEY, source TEXT NOT NULL, cwd TEXT NOT NULL, git_branch TEXT,
  started_at INTEGER NOT NULL, transcript TEXT NOT NULL);
CREATE VIRTUAL TABLE transcripts_fts USING fts5(session_id UNINDEXED, text);
CREATE TABLE decisions(id TEXT PRIMARY KEY, json TEXT NOT NULL);
CREATE TABLE book_chunks(id TEXT PRIMARY KEY, source TEXT NOT NULL, text TEXT NOT NULL);
CREATE TABLE cards(id TEXT PRIMARY KEY, json TEXT NOT NULL);
CREATE TABLE fsrs_reviews(card_id TEXT NOT NULL, ts INTEGER NOT NULL, rating INTEGER NOT NULL,
  device_id TEXT NOT NULL, PRIMARY KEY(card_id, ts, device_id));
"];

/// One `sync-<device>.jsonl` line.
#[derive(serde::Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
enum SyncLine {
    Review(FsrsReviewLog),
    Card(FlashCard),
}

pub struct SqliteTrackerStore {
    dir: PathBuf,
    db: Mutex<Connection>,
}

fn json_rows<T: DeserializeOwned>(db: &Connection, sql: &str) -> StoreResult<Vec<T>> {
    let mut stmt = db.prepare(sql).map_err(be)?;
    let rows = stmt.query_map([], |r| r.get::<_, String>(0)).map_err(be)?;
    rows.map(|j| serde_json::from_str(&j.map_err(be)?).map_err(be))
        .collect()
}

impl SqliteTrackerStore {
    /// `dir` holds `tracker.db` and the `sync-<device>.jsonl` files.
    pub fn open(dir: impl Into<PathBuf>) -> StoreResult<Self> {
        let dir = dir.into();
        create_dir(&dir)?;
        let db = db::open(&dir.join("tracker.db"), MIGRATIONS)?;
        Ok(Self {
            dir,
            db: Mutex::new(db),
        })
    }

    /// BM25-ranked prefix search over transcripts; returns session ids.
    pub fn search_transcripts(&self, query: &str, limit: usize) -> StoreResult<Vec<SmolStr>> {
        let Some(q) = fts_query(query) else {
            return Ok(vec![]);
        };
        let db = self.db.lock().map_err(be)?;
        let mut stmt = db
            .prepare("SELECT session_id FROM transcripts_fts WHERE transcripts_fts MATCH ?1 ORDER BY bm25(transcripts_fts) LIMIT ?2")
            .map_err(be)?;
        let rows = stmt
            .query_map(params![q, limit as i64], |r| r.get::<_, String>(0))
            .map_err(be)?;
        rows.map(|s| s.map(SmolStr::from).map_err(be)).collect()
    }

    /// Merge every other device's `sync-<device>.jsonl` into the database. Idempotent: reviews are
    /// keyed by (card, ts, device) and shipped cards never overwrite a local one.
    pub fn import_sync(&self, own_device: &str) -> StoreResult<()> {
        check_id(own_device)?;
        let own = format!("sync-{own_device}.jsonl");
        for entry in std::fs::read_dir(&self.dir).map_err(be)? {
            let path = entry.map_err(be)?.path();
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if !name.starts_with("sync-") || !name.ends_with(".jsonl") || name == own {
                continue;
            }
            for line in read_all::<SyncLine>(&path)? {
                match line {
                    SyncLine::Review(r) => self.append_review(&r)?,
                    SyncLine::Card(c) => {
                        let json = serde_json::to_string(&c).map_err(be)?;
                        let db = self.db.lock().map_err(be)?;
                        db.execute(
                            "INSERT OR IGNORE INTO cards(id, json) VALUES (?1, ?2)",
                            params![c.id.as_str(), json],
                        )
                        .map_err(be)?;
                    }
                }
            }
        }
        Ok(())
    }
}

impl TrackerStore for SqliteTrackerStore {
    fn cursor(&self, source_key: &str) -> StoreResult<Option<IngestCursor>> {
        let db = self.db.lock().map_err(be)?;
        db.query_row(
            "SELECT mtime_ms, size_bytes FROM ingest_cursors WHERE source_key = ?1",
            [source_key],
            |r| {
                Ok(IngestCursor {
                    source_key: source_key.into(),
                    mtime_ms: r.get(0)?,
                    size_bytes: r.get(1)?,
                })
            },
        )
        .optional()
        .map_err(be)
    }
    fn set_cursor(&self, c: &IngestCursor) -> StoreResult<()> {
        let db = self.db.lock().map_err(be)?;
        db.execute(
            "INSERT INTO ingest_cursors VALUES (?1, ?2, ?3) ON CONFLICT(source_key) DO UPDATE SET mtime_ms = ?2, size_bytes = ?3",
            params![c.source_key, c.mtime_ms, c.size_bytes],
        )
        .map_err(be)?;
        Ok(())
    }
    fn upsert_session(&self, s: &CodingSession) -> StoreResult<()> {
        let mut db = self.db.lock().map_err(be)?;
        let tx = db.transaction().map_err(be)?;
        tx.execute(
            "INSERT INTO sessions VALUES (?1, ?2, ?3, ?4, ?5, ?6) ON CONFLICT(id) DO UPDATE SET
             source = ?2, cwd = ?3, git_branch = ?4, started_at = ?5, transcript = ?6",
            params![
                s.id.as_str(),
                s.source.as_str(),
                s.cwd,
                s.git_branch,
                s.started_at,
                s.transcript
            ],
        )
        .map_err(be)?;
        tx.execute(
            "DELETE FROM transcripts_fts WHERE session_id = ?1",
            [s.id.as_str()],
        )
        .map_err(be)?;
        tx.execute(
            "INSERT INTO transcripts_fts VALUES (?1, ?2)",
            params![s.id.as_str(), s.transcript],
        )
        .map_err(be)?;
        tx.commit().map_err(be)
    }
    fn session(&self, id: &str) -> StoreResult<Option<CodingSession>> {
        let db = self.db.lock().map_err(be)?;
        db.query_row(
            "SELECT source, cwd, git_branch, started_at, transcript FROM sessions WHERE id = ?1",
            [id],
            |r| {
                Ok(CodingSession {
                    id: id.into(),
                    source: r.get::<_, String>(0)?.into(),
                    cwd: r.get(1)?,
                    git_branch: r.get(2)?,
                    started_at: r.get(3)?,
                    transcript: r.get(4)?,
                })
            },
        )
        .optional()
        .map_err(be)
    }
    fn upsert_decision(&self, d: &DecisionRecord) -> StoreResult<()> {
        let db = self.db.lock().map_err(be)?;
        db.execute(
            "INSERT INTO decisions VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET json = ?2",
            params![d.id.as_str(), serde_json::to_string(d).map_err(be)?],
        )
        .map_err(be)?;
        Ok(())
    }
    fn decisions(&self) -> StoreResult<Vec<DecisionRecord>> {
        let db = self.db.lock().map_err(be)?;
        json_rows(&db, "SELECT json FROM decisions ORDER BY rowid")
    }
    fn upsert_book_chunk(&self, c: &BookChunk) -> StoreResult<()> {
        let db = self.db.lock().map_err(be)?;
        db.execute(
            "INSERT INTO book_chunks VALUES (?1, ?2, ?3) ON CONFLICT(id) DO UPDATE SET source = ?2, text = ?3",
            params![c.id.as_str(), c.source, c.text],
        )
        .map_err(be)?;
        Ok(())
    }
    fn upsert_card(&self, c: &FlashCard) -> StoreResult<()> {
        let db = self.db.lock().map_err(be)?;
        db.execute(
            "INSERT INTO cards VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET json = ?2",
            params![c.id.as_str(), serde_json::to_string(c).map_err(be)?],
        )
        .map_err(be)?;
        Ok(())
    }
    fn cards(&self) -> StoreResult<Vec<FlashCard>> {
        let db = self.db.lock().map_err(be)?;
        json_rows(&db, "SELECT json FROM cards ORDER BY rowid")
    }
    fn card(&self, id: &str) -> StoreResult<Option<FlashCard>> {
        let db = self.db.lock().map_err(be)?;
        let json: Option<String> = db
            .query_row("SELECT json FROM cards WHERE id = ?1", [id], |r| r.get(0))
            .optional()
            .map_err(be)?;
        json.map(|j| serde_json::from_str(&j).map_err(be))
            .transpose()
    }
    fn append_review(&self, r: &FsrsReviewLog) -> StoreResult<()> {
        let db = self.db.lock().map_err(be)?;
        db.execute(
            "INSERT OR IGNORE INTO fsrs_reviews VALUES (?1, ?2, ?3, ?4)",
            params![
                r.card_id.as_str(),
                r.ts,
                r.rating as i64,
                r.device_id.as_str()
            ],
        )
        .map_err(be)?;
        Ok(())
    }
    fn reviews(&self) -> StoreResult<Vec<FsrsReviewLog>> {
        let db = self.db.lock().map_err(be)?;
        let mut stmt = db
            .prepare(
                "SELECT card_id, ts, rating, device_id FROM fsrs_reviews ORDER BY ts, device_id",
            )
            .map_err(be)?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, i64>(1)?,
                    r.get::<_, i64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(be)?;
        rows.map(|row| {
            let (card_id, ts, rating, device_id) = row.map_err(be)?;
            let rating = rating_from(rating)?;
            Ok(FsrsReviewLog {
                card_id: card_id.into(),
                ts,
                rating,
                device_id: device_id.into(),
            })
        })
        .collect()
    }
    fn commit_review(&self, c: &FlashCard, r: &FsrsReviewLog) -> StoreResult<()> {
        let mut db = self.db.lock().map_err(be)?;
        let tx = db.transaction().map_err(be)?;
        tx.execute(
            "INSERT INTO cards VALUES (?1, ?2) ON CONFLICT(id) DO UPDATE SET json = ?2",
            params![c.id.as_str(), serde_json::to_string(c).map_err(be)?],
        )
        .map_err(be)?;
        tx.execute(
            "INSERT OR IGNORE INTO fsrs_reviews VALUES (?1, ?2, ?3, ?4)",
            params![
                r.card_id.as_str(),
                r.ts,
                r.rating as i64,
                r.device_id.as_str()
            ],
        )
        .map_err(be)?;
        tx.commit().map_err(be)
    }
    fn append_sync(&self, device_id: &str, line: &serde_json::Value) -> StoreResult<()> {
        check_id(device_id)?;
        append(&self.dir.join(format!("sync-{device_id}.jsonl")), line)
    }
}

fn rating_from(n: i64) -> StoreResult<Rating> {
    Ok(match n {
        1 => Rating::Again,
        2 => Rating::Hard,
        3 => Rating::Good,
        4 => Rating::Easy,
        _ => return Err(be(format!("bad rating {n}"))),
    })
}
