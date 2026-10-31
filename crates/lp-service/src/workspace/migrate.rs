//! Legacy single-store → `workspaces/<id>` "Default" migration (docs/web/02 §Migration).
//!
//! Entries are **renamed, not copied** (same filesystem: atomic, no extra disk). `.migrated-v1` at
//! the data root records the plan and every finished move: `in-progress` while running, so a rerun
//! after a crash resumes; `done` afterwards, so a rerun is a no-op; and it is what
//! [`revert`] walks backwards.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::{Deserialize, Serialize};

use super::lock::ServerLock;
use super::meta::{is_valid_id, WorkspaceMeta};
use super::{fsx, state::State, Result, WorkspaceError};
use crate::paths;

/// The marker file at the data root.
pub const MARKER: &str = ".migrated-v1";

/// What moves, in order. `logs/`, `settings.json` and `state.json` stay global.
pub const LEGACY_ENTRIES: [&str; 8] = [
    "graph",
    "chats",
    "jobs",
    "artifacts",
    "tracker",
    "index.db",
    "index.db-wal",
    "index.db-shm",
];

/// The name the migrated workspace gets.
pub const DEFAULT_NAME: &str = "Default";

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MarkerState {
    InProgress,
    Done,
}

/// One planned move, paths relative to the data root.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Move {
    pub from: String,
    pub to: String,
}

/// The contents of `.migrated-v1`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MigrationMarker {
    pub state: MarkerState,
    pub workspace_id: String,
    /// Every planned move, old → new.
    pub moves: Vec<Move>,
    /// `from` of each finished move, in order.
    pub done: Vec<String>,
    pub started_at: i64,
    #[serde(default)]
    pub finished_at: Option<i64>,
}

/// Result of [`migrate`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MigrationOutcome {
    /// No legacy data at the root (fresh install or already workspace-shaped).
    NotNeeded,
    /// The marker says it already ran.
    AlreadyMigrated { workspace_id: String },
    /// Moved `moved` into the new workspace; `resumed` when an interrupted run was finished.
    Migrated {
        workspace_id: String,
        moved: Vec<String>,
        resumed: bool,
    },
}

/// Result of [`revert`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RevertReport {
    pub workspace_id: String,
    /// Entries moved back to the data root.
    pub restored: Vec<String>,
    /// The workspace directory, if it still held files created after the migration.
    pub left_behind: Option<PathBuf>,
}

fn marker_path(root: &Path) -> PathBuf {
    root.join(MARKER)
}

/// The marker, if any.
pub fn marker(root: &Path) -> Result<Option<MigrationMarker>> {
    fsx::read_json(&marker_path(root))
}

fn save_marker(root: &Path, m: &MigrationMarker) -> Result<()> {
    fsx::write_json(&marker_path(root), m)
}

/// Whether `workspaces/` already holds at least one workspace.
fn has_workspaces(root: &Path) -> Result<bool> {
    match std::fs::read_dir(paths::workspaces_dir(root)) {
        Ok(entries) => {
            for e in entries {
                if e?.file_name().to_str().is_some_and(is_valid_id) {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e.into()),
    }
}

/// Whether [`migrate`] has work to do: an interrupted run, or legacy `graph/` or `chats/` at the
/// root with no workspaces yet.
pub fn needed(root: &Path) -> Result<bool> {
    if let Some(m) = marker(root)? {
        return Ok(m.state == MarkerState::InProgress);
    }
    let legacy = ["graph", "chats"]
        .iter()
        .any(|e| fsx::exists(&root.join(e)));
    Ok(legacy && !has_workspaces(root)?)
}

/// Pids other than ours holding a legacy store file open (a running legacy TUI). Uses `lsof`;
/// when it is unavailable the check is skipped with a warning.
fn legacy_holders(root: &Path) -> Vec<u32> {
    let mut files = vec![];
    for db in ["graph/graph.db", "tracker/tracker.db", "index.db"] {
        for suffix in ["", "-wal"] {
            let p = root.join(format!("{db}{suffix}"));
            if p.is_file() {
                files.push(p);
            }
        }
    }
    for ledger in ["nodes", "chains", "coverage", "sessions"] {
        let p = root.join(format!("graph/{ledger}.jsonl"));
        if p.is_file() {
            files.push(p);
        }
    }
    if files.is_empty() {
        return vec![];
    }
    let out = match Command::new("lsof")
        .arg("-t")
        .arg("-w")
        .arg("--")
        .args(&files)
        .output()
    {
        Ok(o) => o,
        Err(e) => {
            tracing::warn!("lsof unavailable, cannot check for a running legacy LearnPuppy: {e}");
            return vec![];
        }
    };
    let me = std::process::id();
    let mut pids: Vec<u32> = String::from_utf8_lossy(&out.stdout)
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .filter(|p| *p != me)
        .collect();
    pids.sort_unstable();
    pids.dedup();
    pids
}

/// Run (or resume) the migration. The caller proves it holds `server.lock`; another process with
/// the legacy files open is [`WorkspaceError::Conflict`].
pub fn migrate(root: &Path, _lock: &ServerLock) -> Result<MigrationOutcome> {
    migrate_with(root, &mut |_| Ok(()))
}

/// [`migrate`] with a hook run before each move (tests inject crashes here).
pub(crate) fn migrate_with(
    root: &Path,
    before_move: &mut dyn FnMut(&str) -> Result<()>,
) -> Result<MigrationOutcome> {
    let existing = marker(root)?;
    if let Some(m) = &existing {
        if m.state == MarkerState::Done {
            return Ok(MigrationOutcome::AlreadyMigrated {
                workspace_id: m.workspace_id.clone(),
            });
        }
    } else if !needed(root)? {
        return Ok(MigrationOutcome::NotNeeded);
    }
    let holders = legacy_holders(root);
    if !holders.is_empty() {
        return Err(WorkspaceError::Conflict(format!(
            "another LearnPuppy process (pid {}) has the legacy store open; quit it and retry",
            holders
                .iter()
                .map(u32::to_string)
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }

    let resumed = existing.is_some();
    let mut m = match existing {
        Some(m) => m,
        None => {
            let id = super::meta::new_id();
            let ws = format!("workspaces/{id}");
            let m = MigrationMarker {
                state: MarkerState::InProgress,
                moves: LEGACY_ENTRIES
                    .iter()
                    .filter(|e| fsx::exists(&root.join(e)))
                    .map(|e| Move {
                        from: (*e).to_owned(),
                        to: format!("{ws}/{e}"),
                    })
                    .collect(),
                workspace_id: id,
                done: vec![],
                started_at: crate::now_ms(),
                finished_at: None,
            };
            // Persist the plan before touching anything, so a crash at any point can resume.
            save_marker(root, &m)?;
            m
        }
    };
    if !is_valid_id(&m.workspace_id) {
        return Err(WorkspaceError::Internal(format!(
            "{MARKER} names an invalid workspace id {:?}",
            m.workspace_id
        )));
    }

    let ws_dir = paths::workspace_dir(root, &m.workspace_id);
    fsx::create_dir(&ws_dir)?;
    if WorkspaceMeta::load(&ws_dir).is_err() {
        let mut meta =
            WorkspaceMeta::new(m.workspace_id.clone(), DEFAULT_NAME.into(), m.started_at);
        meta.slug = "default".into();
        meta.save(&ws_dir)?;
    }

    for mv in m.moves.clone() {
        if m.done.contains(&mv.from) {
            continue;
        }
        let (from, to) = (root.join(&mv.from), root.join(&mv.to));
        match (fsx::exists(&from), fsx::exists(&to)) {
            // Crashed after the rename but before the marker update.
            (false, true) => {}
            (true, false) => {
                before_move(&mv.from)?;
                fsx::rename(&from, &to)?;
            }
            (false, false) => {
                return Err(WorkspaceError::Internal(format!(
                    "migration lost {}: neither {} nor {} exists",
                    mv.from,
                    from.display(),
                    to.display()
                )))
            }
            (true, true) => {
                return Err(WorkspaceError::Conflict(format!(
                    "both {} and {} exist; move one aside and rerun",
                    from.display(),
                    to.display()
                )))
            }
        }
        m.done.push(mv.from.clone());
        save_marker(root, &m)?;
    }

    State::update(root, |s| s.last_workspace = Some(m.workspace_id.clone()))?;
    m.state = MarkerState::Done;
    m.finished_at = Some(crate::now_ms());
    save_marker(root, &m)?;
    tracing::info!(workspace = %m.workspace_id, moved = ?m.done, "migrated legacy store");
    Ok(MigrationOutcome::Migrated {
        workspace_id: m.workspace_id,
        moved: m.done,
        resumed,
    })
}

/// `learnpuppy migrate --revert`: move every finished move back, drop the Default workspace's
/// metadata and directory (kept if it holds newer files) and the marker.
pub fn revert(root: &Path, _lock: &ServerLock) -> Result<RevertReport> {
    let Some(m) = marker(root)? else {
        return Err(WorkspaceError::NotFound(format!(
            "no {MARKER} marker under {}",
            root.display()
        )));
    };
    let mut restored = vec![];
    for from in m.done.iter().rev() {
        let Some(mv) = m.moves.iter().find(|mv| &mv.from == from) else {
            continue;
        };
        let (old, new) = (root.join(&mv.from), root.join(&mv.to));
        if fsx::exists(&old) {
            return Err(WorkspaceError::Conflict(format!(
                "{} exists again; move it aside before reverting",
                old.display()
            )));
        }
        if fsx::exists(&new) {
            fsx::rename(&new, &old)?;
            restored.push(mv.from.clone());
        }
    }
    let ws_dir = paths::workspace_dir(root, &m.workspace_id);
    let left_behind = if fsx::exists(&ws_dir) {
        // Store dirs `open` created after the migration are empty; drop them.
        if let Ok(entries) = std::fs::read_dir(&ws_dir) {
            for e in entries.flatten() {
                if e.file_type().is_ok_and(|t| t.is_dir()) {
                    remove_empty_dirs(&e.path());
                }
            }
        }
        let only_meta = std::fs::read_dir(&ws_dir)?
            .flatten()
            .all(|e| e.file_name() == super::meta::META_FILE);
        if only_meta {
            std::fs::remove_dir_all(&ws_dir)?;
            None
        } else {
            // Newer files live there; keep it a valid workspace rather than lose them.
            Some(ws_dir)
        }
    } else {
        None
    };
    // An empty `workspaces/` would not block a later migration, but leave no trace of this one.
    let _ = std::fs::remove_dir(paths::workspaces_dir(root));
    std::fs::remove_file(marker_path(root))?;
    State::update(root, |s| {
        if s.last_workspace.as_deref() == Some(m.workspace_id.as_str()) {
            s.last_workspace = None;
        }
    })?;
    restored.reverse();
    Ok(RevertReport {
        workspace_id: m.workspace_id,
        restored,
        left_behind,
    })
}

/// Remove `dir` and its subdirectories bottom-up wherever they are empty.
fn remove_empty_dirs(dir: &Path) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for e in entries.flatten() {
            if e.file_type().is_ok_and(|t| t.is_dir()) {
                remove_empty_dirs(&e.path());
            }
        }
    }
    let _ = std::fs::remove_dir(dir);
}
