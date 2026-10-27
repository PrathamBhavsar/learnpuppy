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
    /// The sessions stored by this run, for distilling.
    pub stored: Vec<CodingSession>,
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
    // Ok(false): a session fell outside `since`, so the file must be re-read when the window widens.
    let save = |rep: &mut IngestReport, sessions: Vec<CodingSession>| -> Result<bool, String> {
        let mut complete = true;
        for s in &sessions {
            if s.started_at < since_ms {
                complete = false;
            } else if allow.iter().any(|a| Path::new(&s.cwd).starts_with(a)) {
                store.upsert_session(s).map_err(|e| e.to_string())?;
                rep.sessions += 1;
                rep.stored.push(s.clone());
            }
        }
        Ok(complete)
    };
    // Cursors are keyed on the allowlist too, so adding a repo re-reads files skipped for it.
    let scope = {
        let mut a: Vec<_> = allow.iter().map(|p| p.to_string_lossy()).collect();
        a.sort();
        crate::distill::short_hash(&a.iter().map(|p| &**p).collect::<Vec<_>>())
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
            format!("{source}:{}#{scope}", path.display()),
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
        if let Some(cur) = stamp(&live, format!("agy:{}#{scope}", db.display())) {
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
    res: Result<bool, String>,
) {
    let set = |complete| match complete {
        true => store.set_cursor(&cur).map_err(|e| e.to_string()),
        false => Ok(()),
    };
    match res.and_then(set) {
        Ok(()) => rep.files += 1,
        Err(e) => rep.errors.push(format!("{}: {e}", path.display())),
    }
}

/// Start of a `--since` window: `<n>h`, `<n>d` or `<n>w` before `now_ms`.
pub fn since_ms(spec: &str, now_ms: i64) -> Result<i64, String> {
    let bad = || format!("bad --since `{spec}`: use <n>h, <n>d or <n>w");
    let (n, hours) = [("h", 1), ("d", 24), ("w", 24 * 7)]
        .iter()
        .find_map(|(u, h)| spec.strip_suffix(u).map(|n| (n, *h)))
        .ok_or_else(bad)?;
    let n: i64 = n.parse().map_err(|_| bad())?;
    n.checked_mul(hours * 3_600_000)
        .and_then(|d| now_ms.checked_sub(d))
        .ok_or_else(bad)
}
