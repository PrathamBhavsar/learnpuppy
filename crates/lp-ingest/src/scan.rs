//! Incremental scan of all agent roots; unchanged files are skipped via `mtime+size` cursors.
use super::{agy::parse_agy, transcript};
use lp_core::ports::{CodingSession, IngestCursor, TrackerStore};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub struct Roots {
    /// Every Claude `projects` dir (one per account).
    pub claude: Vec<PathBuf>,
    pub codex: PathBuf,
    pub agy: PathBuf,
}

impl Roots {
    pub fn from_home(home: &Path) -> Self {
        let mut claude = vec![
            home.join(".claude/projects"),
            home.join(".claude-work/projects"),
        ];
        for e in std::fs::read_dir(home.join(".claude-accounts"))
            .into_iter()
            .flatten()
            .flatten()
        {
            claude.push(e.path().join("projects"));
        }
        Self {
            claude,
            codex: home.join(".codex/sessions"),
            agy: home.join(".gemini/antigravity-cli"),
        }
    }
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct IngestReport {
    pub files: usize,
    pub sessions: usize,
    pub errors: Vec<String>,
}

fn stamp(paths: &[PathBuf], key: String) -> Option<IngestCursor> {
    let (mut mtime_ms, mut size_bytes) = (0, 0);
    for m in paths.iter().filter_map(|p| p.metadata().ok()) {
        let t = m
            .modified()
            .ok()?
            .duration_since(UNIX_EPOCH)
            .ok()?
            .as_millis() as i64;
        mtime_ms = mtime_ms.max(t);
        size_bytes += m.len();
    }
    Some(IngestCursor {
        source_key: key,
        mtime_ms,
        size_bytes,
    })
}

type Parser = fn(&Path) -> std::io::Result<Option<CodingSession>>;

/// Ingests sessions started at/after `since_ms` whose `cwd` is inside an `allow`listed repo.
/// A failing source is reported and does not stop the others; its cursor is left unset so it retries.
pub fn ingest(
    store: &dyn TrackerStore,
    roots: &Roots,
    allow: &[PathBuf],
    since_ms: i64,
) -> IngestReport {
    let mut rep = IngestReport::default();
    let save = |rep: &mut IngestReport, sessions: Vec<CodingSession>| -> Result<(), String> {
        for s in sessions.iter().filter(|s| {
            s.started_at >= since_ms && allow.iter().any(|a| Path::new(&s.cwd).starts_with(a))
        }) {
            store.upsert_session(s).map_err(|e| e.to_string())?;
            rep.sessions += 1;
        }
        Ok(())
    };
    let mut jsonl: Vec<(&str, PathBuf, Parser)> = vec![];
    for dir in &roots.claude {
        jsonl.extend(
            transcript::jsonl_files(dir)
                .into_iter()
                .map(|p| ("claude", p, transcript::parse_claude as _)),
        );
    }
    jsonl.extend(
        transcript::jsonl_files(&roots.codex)
            .into_iter()
            .map(|p| ("codex", p, transcript::parse_codex as _)),
    );
    for (source, path, parse) in jsonl {
        let Some(cur) = stamp(
            std::slice::from_ref(&path),
            format!("{source}:{}", path.display()),
        ) else {
            continue;
        };
        if matches!(store.cursor(&cur.source_key), Ok(Some(prev)) if prev == cur) {
            continue;
        }
        let res = parse(&path)
            .map_err(|e| e.to_string())
            .and_then(|s| save(&mut rep, s.into_iter().collect()));
        finish(store, &mut rep, &path, cur, res);
    }
    let db = roots.agy.join("conversation_summaries.db");
    let live = [db.clone(), roots.agy.join("conversation_summaries.db-wal")].to_vec();
    if db.exists() {
        if let Some(cur) = stamp(&live, format!("agy:{}", db.display())) {
            if !matches!(store.cursor(&cur.source_key), Ok(Some(prev)) if prev == cur) {
                let res = parse_agy(&roots.agy).and_then(|s| save(&mut rep, s));
                finish(store, &mut rep, &db, cur, res);
            }
        }
    }
    rep
}

fn finish(
    store: &dyn TrackerStore,
    rep: &mut IngestReport,
    path: &Path,
    cur: IngestCursor,
    res: Result<(), String>,
) {
    match res.and_then(|()| store.set_cursor(&cur).map_err(|e| e.to_string())) {
        Ok(()) => rep.files += 1,
        Err(e) => rep.errors.push(format!("{}: {e}", path.display())),
    }
}
