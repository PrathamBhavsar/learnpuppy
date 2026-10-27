//! Antigravity: read `conversation_summaries` from a temp snapshot, never the live WAL-mode db.
use super::{redact::redact, transcript::clip};
use chrono::DateTime;
use lp_core::ports::CodingSession;
use std::path::Path;

pub(crate) fn parse_agy(root: &Path) -> Result<Vec<CodingSession>, String> {
    let snap = tempfile::tempdir().map_err(|e| e.to_string())?;
    for name in [
        "conversation_summaries.db",
        "conversation_summaries.db-wal",
        "conversation_summaries.db-shm",
    ] {
        let src = root.join(name);
        if src.exists() {
            std::fs::copy(&src, snap.path().join(name)).map_err(|e| e.to_string())?;
        }
    }
    let db = rusqlite::Connection::open(snap.path().join("conversation_summaries.db"))
        .map_err(|e| e.to_string())?;
    let mut stmt = db
        .prepare("SELECT conversation_id,title,preview,workspace_uris,last_modified_time FROM conversation_summaries")
        .map_err(|e| e.to_string())?;
    let rows = stmt
        .query_map([], |r| {
            Ok(std::array::from_fn::<String, 5, _>(|i| {
                r.get(i).unwrap_or_default()
            }))
        })
        .map_err(|e| e.to_string())?;
    let mut out = vec![];
    for [id, title, preview, uris, modified] in rows.flatten() {
        let Ok(ts) = DateTime::parse_from_rfc3339(&modified) else {
            continue;
        };
        let uris: Vec<String> = serde_json::from_str(&uris).unwrap_or_default();
        let cwd = uris
            .first()
            .map(|u| u.trim_start_matches("file://").to_string())
            .unwrap_or_default();
        let mut transcript = format!(
            "U: {}\n",
            clip(if preview.is_empty() { &title } else { &preview }, 500)
        );
        let mut arts: Vec<_> = std::fs::read_dir(root.join("brain").join(&id))
            .into_iter()
            .flatten()
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|x| x == "md"))
            .collect();
        arts.sort();
        for p in arts.iter().take(4) {
            if let Ok(text) = std::fs::read_to_string(p) {
                let name = p.file_name().unwrap_or_default().to_string_lossy();
                transcript.push_str(&format!("A: {name}: {}\n", clip(&text, 1500)));
            }
        }
        out.push(CodingSession {
            id: format!("agy:{id}").into(),
            source: "agy".into(),
            cwd,
            git_branch: None,
            started_at: ts.timestamp_millis(),
            transcript: redact(&transcript),
        });
    }
    Ok(out)
}
