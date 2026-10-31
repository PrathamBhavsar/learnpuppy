//! Delete into `.trash/<ts>-<id>/`, list, restore and purge (docs/web/02 §Registry, delete).

use std::path::PathBuf;

use serde_json::json;

use super::meta::{self, is_valid_id, unique_name, unique_slug, WorkspaceMeta};
use super::registry::WorkspaceRegistry;
use super::{fsx, state::State, Result, WorkspaceError};
use crate::paths;

/// One deleted workspace waiting in `.trash/`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TrashEntry {
    /// The trash directory name, `<deleted_at>-<id>`; the key for restore and purge.
    pub key: String,
    /// The workspace id it had when deleted.
    pub id: String,
    /// From its `workspace.json`; the id when that file is unreadable.
    pub name: String,
    /// Epoch ms.
    pub deleted_at: i64,
    pub path: PathBuf,
}

/// Split `<ts>-<id>` into its parts, rejecting anything else (and so any path tricks).
fn parse_key(key: &str) -> Option<(i64, &str)> {
    let (ts, id) = key.split_once('-')?;
    if ts.is_empty() || !ts.bytes().all(|b| b.is_ascii_digit()) || !is_valid_id(id) {
        return None;
    }
    Some((ts.parse().ok()?, id))
}

impl WorkspaceRegistry {
    /// Delete a workspace: `confirm_name` must equal its name (the id for a damaged one). Running
    /// turns and jobs are cancelled first, then the directory moves to `.trash/<ts>-<id>/`.
    /// Emits `workspace.deleted`.
    pub fn delete(&self, id: &str, confirm_name: &str) -> Result<TrashEntry> {
        let _ops = self.ops_lock();
        let dir = self.dir(id)?;
        let name = match WorkspaceMeta::load(&dir) {
            Ok(m) => m.name,
            Err(WorkspaceError::Damaged { .. }) => id.to_owned(),
            Err(e) => return Err(e),
        };
        if confirm_name.trim() != name {
            return Err(WorkspaceError::ConfirmMismatch);
        }
        self.evict(id);
        let trash = paths::trash_dir(self.root());
        fsx::create_dir(&trash)?;
        let mut deleted_at = crate::now_ms();
        let mut dest = trash.join(format!("{deleted_at}-{id}"));
        while fsx::exists(&dest) {
            deleted_at += 1;
            dest = trash.join(format!("{deleted_at}-{id}"));
        }
        fsx::rename(&dir, &dest)?;
        State::update(self.root(), |s| {
            if s.last_workspace.as_deref() == Some(id) {
                s.last_workspace = None;
            }
        })?;
        self.emit_global("workspace.deleted", json!({ "id": id }));
        Ok(TrashEntry {
            key: format!("{deleted_at}-{id}"),
            id: id.to_owned(),
            name,
            deleted_at,
            path: dest,
        })
    }

    /// Everything in the trash, most recently deleted first.
    pub fn trash_list(&self) -> Result<Vec<TrashEntry>> {
        let trash = paths::trash_dir(self.root());
        let entries = match std::fs::read_dir(&trash) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(e.into()),
        };
        let mut out = vec![];
        for entry in entries {
            let entry = entry?;
            let Some(key) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            let Some((deleted_at, id)) = parse_key(&key) else {
                continue;
            };
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = fsx::read_json::<WorkspaceMeta>(&entry.path().join(meta::META_FILE))
                .ok()
                .flatten()
                .map_or_else(|| id.to_owned(), |m| m.name);
            out.push(TrashEntry {
                id: id.to_owned(),
                key,
                name,
                deleted_at,
                path: entry.path(),
            });
        }
        out.sort_by(|a, b| b.deleted_at.cmp(&a.deleted_at).then(a.key.cmp(&b.key)));
        Ok(out)
    }

    fn trash_path(&self, key: &str) -> Result<(PathBuf, String)> {
        let (_, id) = parse_key(key).ok_or_else(|| WorkspaceError::NotFound(key.to_owned()))?;
        let path = paths::trash_dir(self.root()).join(key);
        if !path.is_dir() {
            return Err(WorkspaceError::NotFound(key.to_owned()));
        }
        Ok((path, id.to_owned()))
    }

    /// Move a trashed workspace back. It keeps its id unless that id was reused meanwhile, and
    /// gets a ` (2)` suffix if a live workspace took its name. Damaged metadata is rewritten
    /// minimally. Emits `workspace.created`.
    pub fn trash_restore(&self, key: &str) -> Result<WorkspaceMeta> {
        let _ops = self.ops_lock();
        let (path, old_id) = self.trash_path(key)?;
        let id = if fsx::exists(&self.dir(&old_id)?) {
            self.fresh_id()?
        } else {
            old_id
        };
        let now = crate::now_ms();
        let mut m = fsx::read_json::<WorkspaceMeta>(&path.join(meta::META_FILE))
            .ok()
            .flatten()
            .unwrap_or_else(|| WorkspaceMeta::new(id.clone(), id.clone(), now));
        m.id = id.clone();
        m.name = unique_name(&m.name, &self.live_names(None)?);
        m.slug = unique_slug(&meta::slugify(&m.name), &self.slugs(None)?);
        m.save(&path)?;
        fsx::rename(&path, &self.dir(&id)?)?;
        self.emit_global("workspace.created", json!({ "id": id }));
        Ok(m)
    }

    /// Permanently delete one trashed workspace.
    pub fn trash_purge(&self, key: &str) -> Result<()> {
        let _ops = self.ops_lock();
        let (path, _) = self.trash_path(key)?;
        std::fs::remove_dir_all(&path)?;
        Ok(())
    }

    /// Permanently delete everything in the trash; returns how many workspaces were purged.
    pub fn trash_empty(&self) -> Result<usize> {
        let entries = self.trash_list()?;
        for e in &entries {
            self.trash_purge(&e.key)?;
        }
        Ok(entries.len())
    }
}

#[cfg(test)]
mod tests {
    use super::parse_key;

    #[test]
    fn trash_keys_are_strict() {
        assert_eq!(
            parse_key("1761700000000-ws_ab12"),
            Some((1761700000000, "ws_ab12"))
        );
        for bad in [
            "-ws_ab",
            "12-",
            "12-../x",
            "1a-ws_ab",
            "12-ws_AB",
            "12-ws_ab/../x",
        ] {
            assert_eq!(parse_key(bad), None, "{bad}");
        }
    }
}
