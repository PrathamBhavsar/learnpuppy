//! Workspace export/import as a zip (docs/web/02 §Registry, export/import).
//!
//! The archive is artifacts-free and holds no SQLite files:
//!
//! ```text
//! manifest.json                    format, version, schema, counts, sha256 + size of every file
//! workspace.json
//! graph/{nodes,chains,coverage,sessions}.jsonl  graph/positions.json
//! chats/<id>.json
//! tracker/<table>.jsonl            tracker.db rows, one JSON object per row
//! tracker/sync-<device>.jsonl      FSRS replication logs, verbatim
//! ```
//!
//! Import validates the manifest, every checksum and the schema, stages into a hidden directory,
//! rebuilds `tracker.db` (and, via the graph store, its FTS index) from the JSONL, then renames
//! the result into place under a **new** id. It never overwrites an existing workspace.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use lp_core::ports::{ChatStore, GraphStore, TrackerStore};
use rusqlite::types::{Value, ValueRef};
use rusqlite::{Connection, OpenFlags};
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipArchive, ZipWriter};

use super::meta::{self, unique_name, unique_slug, WorkspaceMeta, META_FILE, SCHEMA};
use super::registry::WorkspaceRegistry;
use super::{fsx, Result, WorkspaceError};
use crate::stores::Stores;

/// `manifest.json` `format` value.
pub const FORMAT: &str = "learnpuppy-workspace";
/// Archive layout version this build writes and reads.
pub const ARCHIVE_VERSION: u32 = 1;
const MANIFEST: &str = "manifest.json";

/// Zip-bomb guards for imports.
const MAX_MANIFEST_BYTES: u64 = 16 << 20;
const MAX_ENTRY_BYTES: u64 = 2 << 30;
const MAX_TOTAL_BYTES: u64 = 8 << 30;
const MAX_ENTRIES: usize = 200_000;

/// Graph files that travel; `graph.db` is a rebuildable index and stays behind.
const GRAPH_FILES: [&str; 5] = [
    "nodes.jsonl",
    "chains.jsonl",
    "coverage.jsonl",
    "sessions.jsonl",
    "positions.json",
];

/// `tracker.db` tables exported as rows. `transcripts_fts` is rebuilt from `sessions`.
const TRACKER_TABLES: [&str; 6] = [
    "ingest_cursors",
    "sessions",
    "decisions",
    "book_chunks",
    "cards",
    "fsrs_reviews",
];

/// Where row dumps are unpacked during import, inside the staging dir.
const ROWS_DIR: &str = ".tracker-rows";

/// What an archive holds (export) or what landed in the stores (import).
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExportCounts {
    /// `nodes.jsonl` lines (including link-appends).
    pub nodes: usize,
    pub chains: usize,
    pub chats: usize,
    pub cards: usize,
    pub reviews: usize,
    pub decisions: usize,
    pub sessions: usize,
}

/// One archived file.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ManifestFile {
    pub path: String,
    pub bytes: u64,
    /// Lowercase hex SHA-256 of the uncompressed bytes.
    pub sha256: String,
}

/// `manifest.json`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub format: String,
    pub version: u32,
    /// `workspace.json` schema of the exporting build.
    pub schema: u32,
    pub exported_at: i64,
    pub source_id: String,
    pub name: String,
    pub counts: ExportCounts,
    pub files: Vec<ManifestFile>,
}

/// Result of [`WorkspaceRegistry::export`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExportReport {
    pub path: PathBuf,
    /// Size of the zip.
    pub bytes: u64,
    pub manifest: Manifest,
}

/// Result of [`WorkspaceRegistry::import`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportReport {
    /// The new workspace (fresh id; name suffixed ` (2)` on collision).
    pub meta: WorkspaceMeta,
    /// Counts read back from the rebuilt stores.
    pub counts: ExportCounts,
    /// The id the workspace had in the exporting install.
    pub source_id: String,
}

fn invalid(msg: impl Into<String>) -> WorkspaceError {
    WorkspaceError::InvalidArchive(msg.into())
}

/// Writes through to `inner` while hashing and counting bytes and newlines.
struct Tee<'a, W: Write> {
    inner: &'a mut W,
    hasher: Sha256,
    bytes: u64,
    lines: usize,
}

impl<'a, W: Write> Tee<'a, W> {
    fn new(inner: &'a mut W) -> Self {
        Self {
            inner,
            hasher: Sha256::new(),
            bytes: 0,
            lines: 0,
        }
    }
}

impl<W: Write> Write for Tee<'_, W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let n = self.inner.write(buf)?;
        self.hasher.update(&buf[..n]);
        self.bytes += n as u64;
        self.lines += buf[..n].iter().filter(|b| **b == b'\n').count();
        Ok(n)
    }
    fn flush(&mut self) -> io::Result<()> {
        self.inner.flush()
    }
}

fn zip_err(e: zip::result::ZipError) -> WorkspaceError {
    match e {
        zip::result::ZipError::Io(io) => io.into(),
        other => WorkspaceError::Internal(other.to_string()),
    }
}

/// Builds the zip, recording every file for the manifest.
struct Packer {
    zip: ZipWriter<File>,
    files: Vec<ManifestFile>,
}

impl Packer {
    fn start(&mut self, path: &str, len_hint: u64) -> Result<()> {
        let opts = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o600)
            .large_file(len_hint >= u64::from(u32::MAX));
        self.zip.start_file(path, opts).map_err(zip_err)
    }

    /// Stream `src` into the archive as `path`; returns its line count.
    fn add_reader(&mut self, path: &str, src: &mut dyn Read, len_hint: u64) -> Result<usize> {
        self.start(path, len_hint)?;
        let mut tee = Tee::new(&mut self.zip);
        io::copy(src, &mut tee)?;
        let (bytes, lines) = (tee.bytes, tee.lines);
        self.files.push(ManifestFile {
            path: path.to_owned(),
            bytes,
            sha256: hex::encode(tee.hasher.finalize()),
        });
        Ok(lines)
    }

    fn add_file(&mut self, path: &str, src: &Path) -> Result<usize> {
        let mut f = File::open(src)?;
        let len = f.metadata()?.len();
        self.add_reader(path, &mut f, len)
    }
}

/// Files directly in `dir` whose names satisfy `keep`, sorted; empty when `dir` is missing.
fn files_in(dir: &Path, keep: impl Fn(&str) -> bool) -> Result<Vec<(String, PathBuf)>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(vec![]),
        Err(e) => return Err(e.into()),
    };
    let mut out = vec![];
    for e in entries {
        let e = e?;
        if let Some(name) = e.file_name().to_str() {
            if e.file_type()?.is_file() && keep(name) {
                out.push((name.to_owned(), e.path()));
            }
        }
    }
    out.sort();
    Ok(out)
}

/// `[A-Za-z0-9_-]+`, the store id alphabet.
fn is_plain_id(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 128
        && s.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

fn is_chat_file(name: &str) -> bool {
    name.strip_suffix(".json").is_some_and(is_plain_id)
}

fn is_sync_file(name: &str) -> bool {
    name.strip_prefix("sync-")
        .and_then(|n| n.strip_suffix(".jsonl"))
        .is_some_and(is_plain_id)
}

/// Whether `path` is a file an archive may contain (relative, no traversal, known shape).
fn allowed_path(path: &str) -> bool {
    if path == META_FILE {
        return true;
    }
    match path.split_once('/') {
        Some(("graph", f)) => GRAPH_FILES.contains(&f),
        Some(("chats", f)) => is_chat_file(f),
        Some(("tracker", f)) => {
            is_sync_file(f)
                || f.strip_suffix(".jsonl")
                    .is_some_and(|t| TRACKER_TABLES.contains(&t))
        }
        _ => false,
    }
}

/// Serialize one SQLite value; tracker tables only hold integers, text and nulls.
fn to_json(v: ValueRef<'_>) -> Result<serde_json::Value> {
    Ok(match v {
        ValueRef::Null => serde_json::Value::Null,
        ValueRef::Integer(i) => json!(i),
        ValueRef::Real(f) => json!(f),
        ValueRef::Text(t) => json!(String::from_utf8_lossy(t)),
        ValueRef::Blob(_) => {
            return Err(WorkspaceError::Internal(
                "unexpected blob column in tracker.db".into(),
            ))
        }
    })
}

fn from_json(v: &serde_json::Value) -> Result<Value> {
    Ok(match v {
        serde_json::Value::Null => Value::Null,
        serde_json::Value::Bool(b) => Value::Integer(i64::from(*b)),
        serde_json::Value::Number(n) => match n.as_i64() {
            Some(i) => Value::Integer(i),
            None => Value::Real(
                n.as_f64()
                    .ok_or_else(|| invalid("bad number in tracker row"))?,
            ),
        },
        serde_json::Value::String(s) => Value::Text(s.clone()),
        _ => return Err(invalid("nested value in tracker row")),
    })
}

fn sql_err(e: rusqlite::Error) -> WorkspaceError {
    lp_core::ports::StoreError::Backend(e.to_string()).into()
}

/// Dump every [`TRACKER_TABLES`] table as JSONL bytes, from one consistent read snapshot.
fn dump_tracker(db: &Path) -> Result<Vec<(&'static str, Vec<u8>, usize)>> {
    let conn =
        Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(sql_err)?;
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(sql_err)?;
    conn.execute_batch("BEGIN").map_err(sql_err)?;
    let mut out = vec![];
    for table in TRACKER_TABLES {
        let mut stmt = conn
            .prepare(&format!("SELECT * FROM {table} ORDER BY rowid"))
            .map_err(sql_err)?;
        let cols: Vec<String> = stmt
            .column_names()
            .iter()
            .map(|c| (*c).to_owned())
            .collect();
        let mut rows = stmt.query([]).map_err(sql_err)?;
        let (mut buf, mut n) = (vec![], 0);
        while let Some(row) = rows.next().map_err(sql_err)? {
            let mut obj = serde_json::Map::with_capacity(cols.len());
            for (i, c) in cols.iter().enumerate() {
                obj.insert(c.clone(), to_json(row.get_ref(i).map_err(sql_err)?)?);
            }
            serde_json::to_writer(&mut buf, &obj)?;
            buf.push(b'\n');
            n += 1;
        }
        out.push((table, buf, n));
    }
    conn.execute_batch("COMMIT").map_err(sql_err)?;
    Ok(out)
}

/// Load row dumps from `rows_dir` into a freshly created `tracker.db`, then rebuild its FTS.
fn load_tracker(db: &Path, rows_dir: &Path) -> Result<()> {
    let mut conn = Connection::open(db).map_err(sql_err)?;
    let tx = conn.transaction().map_err(sql_err)?;
    for table in TRACKER_TABLES {
        let path = rows_dir.join(format!("{table}.jsonl"));
        let Ok(f) = File::open(&path) else { continue };
        let known: BTreeSet<String> = {
            let mut stmt = tx
                .prepare(&format!("SELECT name FROM pragma_table_info('{table}')"))
                .map_err(sql_err)?;
            let names = stmt
                .query_map([], |r| r.get::<_, String>(0))
                .map_err(sql_err)?;
            names.collect::<rusqlite::Result<_>>().map_err(sql_err)?
        };
        let mut reader = io::BufReader::new(f);
        let mut line = String::new();
        let mut lineno = 0;
        loop {
            line.clear();
            if io::BufRead::read_line(&mut reader, &mut line)? == 0 {
                break;
            }
            lineno += 1;
            if line.trim().is_empty() {
                continue;
            }
            let row: serde_json::Map<String, serde_json::Value> = serde_json::from_str(&line)
                .map_err(|e| invalid(format!("tracker/{table}.jsonl line {lineno}: {e}")))?;
            if row.is_empty() || row.keys().any(|k| !known.contains(k)) {
                return Err(invalid(format!(
                    "tracker/{table}.jsonl line {lineno}: unknown columns"
                )));
            }
            let cols: Vec<&String> = row.keys().collect();
            let sql = format!(
                "INSERT OR REPLACE INTO {table}({}) VALUES ({})",
                cols.iter()
                    .map(|c| format!("\"{c}\""))
                    .collect::<Vec<_>>()
                    .join(","),
                vec!["?"; cols.len()].join(",")
            );
            let vals = row.values().map(from_json).collect::<Result<Vec<_>>>()?;
            tx.execute(&sql, rusqlite::params_from_iter(vals))
                .map_err(|e| invalid(format!("tracker/{table}.jsonl line {lineno}: {e}")))?;
        }
    }
    tx.execute_batch(
        "DELETE FROM transcripts_fts;
         INSERT INTO transcripts_fts(session_id, text) SELECT id, transcript FROM sessions;",
    )
    .map_err(sql_err)?;
    tx.commit().map_err(sql_err)
}

fn session_count(db: &Path) -> Result<usize> {
    let conn =
        Connection::open_with_flags(db, OpenFlags::SQLITE_OPEN_READ_ONLY).map_err(sql_err)?;
    let n: i64 = conn
        .query_row("SELECT count(*) FROM sessions", [], |r| r.get(0))
        .map_err(sql_err)?;
    Ok(n as usize)
}

/// Counts as the stores see them.
pub(crate) fn store_counts(root: &Path, s: &Stores) -> Result<ExportCounts> {
    Ok(ExportCounts {
        nodes: s.graph.nodes()?.len(),
        chains: s.graph.chains()?.len(),
        chats: s.chats.list_chats()?.len(),
        cards: s.tracker.cards()?.len(),
        reviews: s.tracker.reviews()?.len(),
        decisions: s.tracker.decisions()?.len(),
        sessions: session_count(&root.join("tracker").join("tracker.db"))?,
    })
}

impl WorkspaceRegistry {
    /// Write workspace `id` to the zip at `dest` (replaced atomically). Works on archived
    /// workspaces too; it only reads.
    pub fn export(&self, id: &str, dest: &Path) -> Result<ExportReport> {
        let handle = self.open(id)?;
        let _busy = handle.busy();
        let root = handle.root();
        let meta = handle.meta();
        let part = dest.with_file_name(format!(
            ".{}.part",
            dest.file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("export.zip")
        ));
        let res = (|| -> Result<Manifest> {
            let mut p = Packer {
                zip: ZipWriter::new(File::create(&part)?),
                files: vec![],
            };
            let mut counts = ExportCounts::default();
            p.add_file(META_FILE, &root.join(META_FILE))?;
            let graph = root.join("graph");
            for f in GRAPH_FILES {
                let src = graph.join(f);
                if src.is_file() {
                    let lines = p.add_file(&format!("graph/{f}"), &src)?;
                    match f {
                        "nodes.jsonl" => counts.nodes = lines,
                        "chains.jsonl" => counts.chains = lines,
                        _ => {}
                    }
                }
            }
            for (name, src) in files_in(&root.join("chats"), is_chat_file)? {
                p.add_file(&format!("chats/{name}"), &src)?;
                counts.chats += 1;
            }
            let tracker = root.join("tracker");
            for (table, rows, n) in dump_tracker(&tracker.join("tracker.db"))? {
                let len = rows.len() as u64;
                p.add_reader(&format!("tracker/{table}.jsonl"), &mut rows.as_slice(), len)?;
                match table {
                    "cards" => counts.cards = n,
                    "fsrs_reviews" => counts.reviews = n,
                    "decisions" => counts.decisions = n,
                    "sessions" => counts.sessions = n,
                    _ => {}
                }
            }
            for (name, src) in files_in(&tracker, is_sync_file)? {
                p.add_file(&format!("tracker/{name}"), &src)?;
            }
            let manifest = Manifest {
                format: FORMAT.into(),
                version: ARCHIVE_VERSION,
                schema: SCHEMA,
                exported_at: crate::now_ms(),
                source_id: meta.id.clone(),
                name: meta.name.clone(),
                counts,
                files: p.files.clone(),
            };
            let bytes = serde_json::to_vec_pretty(&manifest)?;
            p.start(MANIFEST, bytes.len() as u64)?;
            p.zip.write_all(&bytes)?;
            p.zip.finish().map_err(zip_err)?.sync_all()?;
            Ok(manifest)
        })();
        let manifest = match res {
            Ok(m) => m,
            Err(e) => {
                let _ = std::fs::remove_file(&part);
                return Err(e);
            }
        };
        if let Err(e) = fsx::rename(&part, dest) {
            let _ = std::fs::remove_file(&part);
            return Err(e);
        }
        Ok(ExportReport {
            path: dest.to_owned(),
            bytes: std::fs::metadata(dest)?.len(),
            manifest,
        })
    }

    /// Import the zip at `src` as a new workspace. Emits `workspace.created`.
    pub fn import(&self, src: &Path) -> Result<ImportReport> {
        let mut archive = ZipArchive::new(File::open(src)?)
            .map_err(|e| invalid(format!("not a zip file: {e}")))?;
        if archive.len() > MAX_ENTRIES {
            return Err(invalid(format!("more than {MAX_ENTRIES} entries")));
        }
        let manifest = read_manifest(&mut archive)?;

        let id = self.fresh_id()?;
        let staging = crate::paths::workspaces_dir(self.root()).join(format!(".importing-{id}"));
        let res = self.import_into(&mut archive, &manifest, &staging, &id);
        if res.is_err() {
            let _ = std::fs::remove_dir_all(&staging);
        }
        res
    }

    fn import_into(
        &self,
        archive: &mut ZipArchive<File>,
        manifest: &Manifest,
        staging: &Path,
        id: &str,
    ) -> Result<ImportReport> {
        fsx::create_dir(staging)?;
        let expected: BTreeMap<&str, &ManifestFile> = manifest
            .files
            .iter()
            .map(|f| (f.path.as_str(), f))
            .collect();
        let mut seen = BTreeSet::new();
        let mut total = 0u64;
        for i in 0..archive.len() {
            let mut entry = archive.by_index(i).map_err(|e| invalid(e.to_string()))?;
            if entry.is_dir() {
                continue;
            }
            let name = entry
                .name()
                .map_err(|e| invalid(e.to_string()))?
                .into_owned();
            if name == MANIFEST {
                continue;
            }
            let Some(want) = expected.get(name.as_str()) else {
                return Err(invalid(format!("{name} is not in the manifest")));
            };
            if !seen.insert(name.clone()) {
                return Err(invalid(format!("{name} appears twice")));
            }
            if entry.size() != want.bytes {
                return Err(invalid(format!("{name}: size does not match the manifest")));
            }
            total += want.bytes;
            if total > MAX_TOTAL_BYTES {
                return Err(invalid("archive is too large"));
            }
            let dest = match name
                .strip_prefix("tracker/")
                .and_then(|f| f.strip_suffix(".jsonl"))
            {
                Some(table) if TRACKER_TABLES.contains(&table) => {
                    staging.join(ROWS_DIR).join(format!("{table}.jsonl"))
                }
                _ => staging.join(&name),
            };
            if let Some(dir) = dest.parent() {
                fsx::create_dir(dir)?;
            }
            let mut out = File::create(&dest)?;
            let mut tee = Tee::new(&mut out);
            io::copy(&mut (&mut entry).take(want.bytes + 1), &mut tee)?;
            let (bytes, sha) = (tee.bytes, hex::encode(tee.hasher.finalize()));
            out.sync_all()?;
            if bytes != want.bytes || sha != want.sha256 {
                return Err(invalid(format!("{name}: checksum mismatch")));
            }
        }
        if let Some(missing) = expected.keys().find(|p| !seen.contains(**p)) {
            return Err(invalid(format!("{missing} is listed but missing")));
        }

        let mut m: WorkspaceMeta = fsx::read_json(&staging.join(META_FILE))
            .map_err(|e| invalid(format!("{META_FILE}: {e}")))?
            .ok_or_else(|| invalid(format!("{META_FILE} is missing")))?;
        if m.schema > SCHEMA {
            return Err(invalid(format!(
                "{META_FILE} schema {} is newer than this build ({SCHEMA})",
                m.schema
            )));
        }

        // Create the store tree (tracker schema, graph FTS from the ledgers), then load rows.
        drop(Stores::open(staging)?);
        let rows = staging.join(ROWS_DIR);
        load_tracker(&staging.join("tracker").join("tracker.db"), &rows)?;
        let _ = std::fs::remove_dir_all(&rows);
        fsx::create_dir(&staging.join("artifacts"))?;
        let counts = store_counts(staging, &Stores::open(staging)?)?;

        let _ops = self.ops_lock();
        let id = if fsx::exists(&self.dir(id)?) {
            self.fresh_id()?
        } else {
            id.to_owned()
        };
        let now = crate::now_ms();
        m.id = id.clone();
        let name = meta::validate_name(&m.name)
            .map_err(|_| invalid(format!("{META_FILE} has an invalid name")))?;
        m.name = unique_name(&name, &self.live_names(None)?);
        m.slug = unique_slug(&meta::slugify(&m.name), &self.slugs(None)?);
        if meta::validate_icon(&m.icon).is_err() {
            m.icon = meta::DEFAULT_ICON.into();
        }
        if meta::validate_hue(&m.hue).is_err() {
            m.hue = meta::DEFAULT_HUE.into();
        }
        m.sources = meta::validate_sources(m.sources.clone()).unwrap_or_default();
        m.archived = false;
        m.created_at = now;
        m.last_opened_at = now;
        m.schema = SCHEMA;
        m.save(staging)?;
        fsx::rename(staging, &self.dir(&id)?)?;
        self.emit_global("workspace.created", json!({ "id": id }));
        Ok(ImportReport {
            meta: m,
            counts,
            source_id: manifest.source_id.clone(),
        })
    }
}

fn read_manifest(archive: &mut ZipArchive<File>) -> Result<Manifest> {
    let mut entry = archive
        .by_name(MANIFEST)
        .map_err(|_| invalid(format!("{MANIFEST} is missing")))?;
    if entry.size() > MAX_MANIFEST_BYTES {
        return Err(invalid(format!("{MANIFEST} is too large")));
    }
    let mut buf = vec![];
    (&mut entry)
        .take(MAX_MANIFEST_BYTES)
        .read_to_end(&mut buf)?;
    let m: Manifest =
        serde_json::from_slice(&buf).map_err(|e| invalid(format!("{MANIFEST}: {e}")))?;
    if m.format != FORMAT {
        return Err(invalid(format!("unknown format {:?}", m.format)));
    }
    if m.version != ARCHIVE_VERSION {
        return Err(invalid(format!(
            "archive version {} is not supported (expected {ARCHIVE_VERSION})",
            m.version
        )));
    }
    if m.schema > SCHEMA {
        return Err(invalid(format!(
            "schema {} is newer than this build ({SCHEMA})",
            m.schema
        )));
    }
    let mut paths = BTreeSet::new();
    for f in &m.files {
        if !allowed_path(&f.path) {
            return Err(invalid(format!("unexpected path {:?}", f.path)));
        }
        if f.bytes > MAX_ENTRY_BYTES {
            return Err(invalid(format!("{} is too large", f.path)));
        }
        if f.sha256.len() != 64 || !f.sha256.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(invalid(format!("{}: malformed checksum", f.path)));
        }
        if !paths.insert(f.path.as_str()) {
            return Err(invalid(format!("{} is listed twice", f.path)));
        }
    }
    if !paths.contains(META_FILE) {
        return Err(invalid(format!("{META_FILE} is missing")));
    }
    Ok(m)
}

#[cfg(test)]
mod tests {
    use super::allowed_path;

    #[test]
    fn only_known_paths_are_accepted() {
        for ok in [
            "workspace.json",
            "graph/nodes.jsonl",
            "graph/positions.json",
            "chats/c_1a17ef24da3.json",
            "tracker/cards.jsonl",
            "tracker/sync-dev123.jsonl",
        ] {
            assert!(allowed_path(ok), "{ok}");
        }
        for bad in [
            "../x",
            "/etc/passwd",
            "graph/graph.db",
            "graph/../workspace.json",
            "chats/../../x.json",
            "chats/sub/c.json",
            "tracker/tracker.db",
            "tracker/sqlite_master.jsonl",
            "jobs/job_1.json",
            "artifacts/a.svg",
        ] {
            assert!(!allowed_path(bad), "{bad}");
        }
    }
}
