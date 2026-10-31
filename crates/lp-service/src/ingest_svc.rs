//! Workspace-scoped ingest configuration, discovery, run reservation and durable history.
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, Write};
use std::path::Path;
use std::sync::Arc;

use lp_ingest::Roots;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tokio_util::sync::CancellationToken;

use crate::workspace::{Sources, WorkspaceHandle, WorkspaceRegistry};

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Report {
    pub sessions: u64,
    pub new_decisions: u64,
    pub new_cards: u64,
    pub skipped: u64,
    pub errors: Vec<String>,
}
impl From<&Report> for crate::events::IngestReport {
    fn from(r: &Report) -> Self {
        Self {
            sessions: r.sessions,
            new_decisions: r.new_decisions,
            new_cards: r.new_cards,
            skipped: r.skipped,
            errors: r.errors.clone(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Run {
    pub id: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub report: Report,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    pub path: String,
    pub sessions: usize,
    pub last_seen: i64,
    pub source: String,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Overlap {
    pub workspace_id: String,
    pub path: String,
    pub other_path: String,
}

/// One active scan/distillation per workspace. Reservation drops on every completion path.
#[derive(Default)]
pub struct IngestRuns {
    active: Mutex<HashMap<String, (String, CancellationToken)>>,
}
pub struct Reservation {
    owner: Arc<IngestRuns>,
    workspace: String,
    pub id: String,
    pub cancel: CancellationToken,
}
impl Drop for Reservation {
    fn drop(&mut self) {
        self.owner.active.lock().remove(&self.workspace);
    }
}
impl IngestRuns {
    pub fn reserve(self: &Arc<Self>, ws: &WorkspaceHandle) -> Option<Reservation> {
        let mut active = self.active.lock();
        if active.contains_key(ws.id()) {
            return None;
        }
        let id = format!("run_{:032x}", rand::random::<u128>());
        let cancel = ws.cancel_token();
        active.insert(ws.id().to_owned(), (id.clone(), cancel.clone()));
        Some(Reservation {
            owner: self.clone(),
            workspace: ws.id().to_owned(),
            id,
            cancel,
        })
    }
    pub fn cancel(&self, ws: &str, id: &str) -> bool {
        let active = self.active.lock();
        match active.get(ws) {
            Some((run, cancel)) if run == id => {
                cancel.cancel();
                true
            }
            _ => false,
        }
    }
    pub fn current(&self, ws: &str) -> Option<String> {
        self.active.lock().get(ws).map(|(id, _)| id.clone())
    }
}

pub fn overlaps(
    registry: &WorkspaceRegistry,
    ws: &str,
    sources: &Sources,
) -> crate::workspace::Result<Vec<Overlap>> {
    let mut warnings = Vec::new();
    for other in registry.list(false)?.workspaces {
        if other.meta.id == ws {
            continue;
        }
        for path in &sources.allow {
            for other_path in &other.meta.sources.allow {
                if path.starts_with(other_path) || other_path.starts_with(path) {
                    warnings.push(Overlap {
                        workspace_id: other.meta.id.clone(),
                        path: path.display().to_string(),
                        other_path: other_path.display().to_string(),
                    });
                }
            }
        }
    }
    Ok(warnings)
}

/// Disable sources by pointing at a guaranteed absent private path (never an empty path).
pub fn scoped_roots(mut roots: Roots, sources: &Sources, workspace: &Path) -> Roots {
    let absent = workspace.join("ingest").join("disabled-sources");
    if !sources.claude {
        roots.claude.clear();
    }
    if !sources.codex {
        roots.codex = absent.join("codex");
    }
    if !sources.agy {
        roots.agy = absent.join("agy");
    }
    roots
}

pub fn history(root: &Path) -> anyhow::Result<Vec<Run>> {
    let file = match std::fs::File::open(root.join("ingest/runs.jsonl")) {
        Ok(f) => f,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e.into()),
    };
    let mut reader = std::io::BufReader::new(file);
    let mut line = String::new();
    let mut runs: Vec<Run> = Vec::new();
    loop {
        line.clear();
        if reader.read_line(&mut line)? == 0 {
            break;
        }
        if !line.trim().is_empty() {
            runs.push(serde_json::from_str(&line)?);
        }
    }
    runs.sort_by_key(|a| std::cmp::Reverse(a.started_at));
    Ok(runs)
}
pub fn finish(root: &Path, run: &Run) -> anyhow::Result<()> {
    std::fs::create_dir_all(root.join("ingest"))?;
    let mut bytes = serde_json::to_vec(run)?;
    bytes.push(b'\n');
    let mut f = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(root.join("ingest/runs.jsonl"))?;
    f.write_all(&bytes)?;
    f.sync_data()?;
    Ok(())
}

/// Reads only transcript metadata, reusing one line buffer and never storing message text.
pub fn candidates(roots: &Roots, since: i64) -> anyhow::Result<Vec<Candidate>> {
    let mut found: BTreeMap<(String, String), Candidate> = BTreeMap::new();
    let dirs = roots
        .claude
        .iter()
        .map(|p| ("claude", p))
        .chain(std::iter::once(("codex", &roots.codex)));
    let mut seen = std::collections::HashSet::new();
    for (source, dir) in dirs {
        let mut stack = vec![dir.clone()];
        while let Some(dir) = stack.pop() {
            let entries = match std::fs::read_dir(&dir) {
                Ok(e) => e,
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
                Err(e) => return Err(e.into()),
            };
            for entry in entries {
                let entry = entry?;
                let path = entry.path();
                let kind = entry.file_type()?;
                if kind.is_dir() && entry.file_name() != "subagents" {
                    stack.push(path);
                    continue;
                }
                if !kind.is_file()
                    || path.extension().is_none_or(|e| e != "jsonl")
                    || !seen.insert(path.clone())
                {
                    continue;
                }
                let mut reader = std::io::BufReader::new(std::fs::File::open(path)?);
                let mut line = String::new();
                let (mut cwd, mut last_seen) = (None, 0);
                loop {
                    line.clear();
                    if reader.read_line(&mut line)? == 0 {
                        break;
                    }
                    let Ok(v) = serde_json::from_str::<serde_json::Value>(&line) else {
                        continue;
                    };
                    if let Some(p) = v
                        .get("cwd")
                        .or_else(|| v.get("payload").and_then(|p| p.get("cwd")))
                        .and_then(|p| p.as_str())
                    {
                        if Path::new(p).is_absolute() {
                            cwd = Some(p.to_owned());
                        }
                    }
                    if let Some(ts) = v
                        .get("timestamp")
                        .and_then(|t| t.as_str())
                        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                    {
                        last_seen = last_seen.max(ts.timestamp_millis());
                    }
                }
                if last_seen < since {
                    continue;
                }
                if let Some(path) = cwd {
                    let c = found
                        .entry((path.clone(), source.to_owned()))
                        .or_insert(Candidate {
                            path,
                            source: source.to_owned(),
                            sessions: 0,
                            last_seen,
                        });
                    c.sessions += 1;
                    c.last_seen = c.last_seen.max(last_seen);
                }
            }
        }
    }
    let mut result: Vec<_> = found.into_values().collect();
    result.extend(agy_candidates(&roots.agy, since)?);
    Ok(result)
}

/// AGY metadata comes from a snapshot; never open the live WAL database.
fn agy_candidates(root: &Path, since: i64) -> anyhow::Result<Vec<Candidate>> {
    if !root.join("conversation_summaries.db").exists() {
        return Ok(vec![]);
    }
    let snapshot = tempfile::tempdir()?;
    for name in [
        "conversation_summaries.db",
        "conversation_summaries.db-wal",
        "conversation_summaries.db-shm",
    ] {
        let path = root.join(name);
        match std::fs::copy(path, snapshot.path().join(name)) {
            Ok(_) => {}
            Err(e)
                if e.kind() == std::io::ErrorKind::NotFound
                    && name != "conversation_summaries.db" => {}
            Err(e) => return Err(e.into()),
        }
    }
    let db = rusqlite::Connection::open(snapshot.path().join("conversation_summaries.db"))?;
    let mut query =
        db.prepare("SELECT workspace_uris,last_modified_time FROM conversation_summaries")?;
    let rows = query.query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?;
    let mut candidates: BTreeMap<String, Candidate> = BTreeMap::new();
    for row in rows {
        let (uris, timestamp) = row?;
        let Ok(timestamp) = chrono::DateTime::parse_from_rfc3339(&timestamp) else {
            continue;
        };
        let last_seen = timestamp.timestamp_millis();
        if last_seen < since {
            continue;
        }
        let uris: Vec<String> = serde_json::from_str(&uris)?;
        if let Some(path) = uris
            .first()
            .map(|p| p.trim_start_matches("file://"))
            .filter(|p| Path::new(p).is_absolute())
        {
            let c = candidates.entry(path.into()).or_insert(Candidate {
                path: path.into(),
                sessions: 0,
                last_seen,
                source: "agy".into(),
            });
            c.sessions += 1;
            c.last_seen = c.last_seen.max(last_seen);
        }
    }
    Ok(candidates.into_values().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{NewWorkspace, NoopSink};

    #[test]
    fn agy_discovery_reads_wal_snapshot_and_filters_old_sessions() {
        let root = tempfile::tempdir().expect("tempdir");
        let db =
            rusqlite::Connection::open(root.path().join("conversation_summaries.db")).expect("db");
        db.execute_batch("PRAGMA journal_mode=WAL; CREATE TABLE conversation_summaries(workspace_uris TEXT, last_modified_time TEXT);").expect("schema");
        for ts in [
            "2026-10-30T00:00:00Z",
            "2026-10-31T00:00:00Z",
            "2020-01-01T00:00:00Z",
        ] {
            db.execute(
                "INSERT INTO conversation_summaries VALUES (?1,?2)",
                ["[\"file:///repo\"]", ts],
            )
            .expect("row");
        }
        let since = chrono::DateTime::parse_from_rfc3339("2026-10-01T00:00:00Z")
            .expect("timestamp")
            .timestamp_millis();
        let found = agy_candidates(root.path(), since).expect("candidates");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].sessions, 2);
        assert_eq!(found[0].source, "agy");
        assert_eq!(found[0].path, "/repo");
        let count: i64 = db
            .query_row("SELECT COUNT(*) FROM conversation_summaries", [], |r| {
                r.get(0)
            })
            .expect("count");
        assert_eq!(count, 3, "live database untouched");
    }

    #[test]
    fn overlap_is_component_based_and_ignores_archived_workspaces() {
        let root = tempfile::tempdir().expect("tempdir");
        let registry = WorkspaceRegistry::new(root.path(), Arc::new(NoopSink)).expect("registry");
        let a = registry
            .create(NewWorkspace {
                name: "A".into(),
                sources: Some(Sources {
                    allow: vec!["/repo/app".into()],
                    ..Default::default()
                }),
                ..Default::default()
            })
            .expect("workspace");
        let sources = Sources {
            allow: vec!["/repo/apple".into()],
            ..Default::default()
        };
        assert!(overlaps(&registry, "ws_ffffffff", &sources)
            .expect("warnings")
            .is_empty());
        let sources = Sources {
            allow: vec!["/repo".into()],
            ..Default::default()
        };
        assert_eq!(
            overlaps(&registry, "ws_ffffffff", &sources)
                .expect("warnings")
                .len(),
            1
        );
        assert!(overlaps(&registry, a.id(), &sources)
            .expect("warnings")
            .is_empty());
        registry.archive(a.id()).expect("archive");
        assert!(overlaps(&registry, "ws_ffffffff", &sources)
            .expect("warnings")
            .is_empty());
    }

    #[test]
    fn run_reservations_survive_cancel_until_work_exits_and_history_reloads() {
        let root = tempfile::tempdir().expect("tempdir");
        let registry = WorkspaceRegistry::new(root.path(), Arc::new(NoopSink)).expect("registry");
        let ws = registry
            .create(NewWorkspace {
                name: "A".into(),
                ..Default::default()
            })
            .expect("workspace");
        let runs = Arc::new(IngestRuns::default());
        let reservation = runs.reserve(&ws).expect("reservation");
        assert!(runs.reserve(&ws).is_none());
        assert!(!runs.cancel(ws.id(), "wrong"));
        assert!(runs.cancel(ws.id(), &reservation.id));
        assert!(reservation.cancel.is_cancelled());
        assert!(runs.reserve(&ws).is_none());
        drop(reservation);
        assert!(runs.reserve(&ws).is_some());
        let run = Run {
            id: "run_1".into(),
            started_at: 1,
            finished_at: Some(2),
            report: Report {
                sessions: 1,
                new_cards: 3,
                ..Default::default()
            },
        };
        finish(ws.root(), &run).expect("persist");
        assert_eq!(history(ws.root()).expect("history"), [run]);
    }
}
