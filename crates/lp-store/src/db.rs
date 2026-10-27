//! SQLite open helper: WAL, busy timeout, `user_version` migrations.
use crate::jsonl::be;
use lp_core::ports::StoreResult;
use rusqlite::Connection;
use std::fs::OpenOptions;
use std::os::unix::fs::OpenOptionsExt;
use std::path::Path;

pub(crate) fn open(path: &Path, migrations: &[&str]) -> StoreResult<Connection> {
    // Pre-create owner-only; SQLite gives the -wal/-shm files the same mode.
    OpenOptions::new()
        .create(true)
        .append(true)
        .mode(0o600)
        .open(path)
        .map_err(be)?;
    let conn = Connection::open(path).map_err(be)?;
    // Before any pragma that needs a lock, so a second process waits instead of failing at once.
    conn.busy_timeout(std::time::Duration::from_secs(5))
        .map_err(be)?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(be)?;
    conn.pragma_update(None, "synchronous", "NORMAL")
        .map_err(be)?;
    let done: usize = conn
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(be)?;
    for (i, sql) in migrations.iter().enumerate().skip(done) {
        conn.execute_batch(&format!(
            "BEGIN;{sql};PRAGMA user_version={};COMMIT;",
            i + 1
        ))
        .map_err(be)?;
    }
    Ok(conn)
}

/// Turn free text into a safe FTS5 query: each word quoted, last word prefix-matched.
pub(crate) fn fts_query(text: &str) -> Option<String> {
    let words: Vec<String> = text
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(|w| format!("\"{w}\""))
        .collect();
    let last = words.len().checked_sub(1)?;
    Some(
        words
            .iter()
            .enumerate()
            .map(|(i, w)| {
                if i == last {
                    format!("{w}*")
                } else {
                    w.clone()
                }
            })
            .collect::<Vec<_>>()
            .join(" "),
    )
}
