//! What `/bootstrap` needs to know about workspaces, behind a trait so the workspace registry
//! (`lp_service::workspace`) can plug in. [`DiskWorkspaces`] reads `workspace.json` files and
//! `state.json` directly (docs/web/02 disk layout).

use std::path::{Path, PathBuf};

use lp_service::paths;
use serde::Deserialize;

/// One workspace as the switcher lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceSummary {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub icon: String,
    pub hue: String,
    pub archived: bool,
    pub last_opened_at: Option<i64>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerLevel {
    Info,
    Warning,
}

/// A one-time notice shown at app start (e.g. "your data moved into the Default workspace").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Banner {
    pub id: String,
    pub level: BannerLevel,
    pub message: String,
}

/// Workspace listing for bootstrap. Methods block (filesystem); call from `spawn_blocking`.
pub trait WorkspaceDirectory: Send + Sync {
    /// Every workspace, archived included, most recently opened first.
    fn list(&self) -> anyhow::Result<Vec<WorkspaceSummary>>;
    /// The workspace the user had open last, if it still exists.
    fn last_workspace(&self) -> anyhow::Result<Option<String>>;
    fn banners(&self) -> anyhow::Result<Vec<Banner>> {
        Ok(Vec::new())
    }
}

/// Reads the data root as laid out in docs/web/02.
pub struct DiskWorkspaces {
    root: PathBuf,
}

impl DiskWorkspaces {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }
}

#[derive(Deserialize)]
#[serde(default)]
struct MetaFile {
    id: String,
    name: String,
    slug: String,
    icon: String,
    hue: String,
    archived: bool,
    last_opened_at: Option<i64>,
}

impl Default for MetaFile {
    fn default() -> Self {
        Self {
            id: String::new(),
            name: String::new(),
            slug: String::new(),
            icon: "book-open".into(),
            hue: "violet".into(),
            archived: false,
            last_opened_at: None,
        }
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct StateFile {
    last_workspace: Option<String>,
}

fn read_meta(dir: &Path) -> anyhow::Result<WorkspaceSummary> {
    let bytes = std::fs::read(dir.join("workspace.json"))?;
    let m: MetaFile = serde_json::from_slice(&bytes)?;
    // The directory name is the id; a mismatching or empty `id` field does not win.
    let id = dir
        .file_name()
        .and_then(|n| n.to_str())
        .map_or(m.id, str::to_owned);
    Ok(WorkspaceSummary {
        name: if m.name.is_empty() {
            id.clone()
        } else {
            m.name
        },
        slug: m.slug,
        icon: m.icon,
        hue: m.hue,
        archived: m.archived,
        last_opened_at: m.last_opened_at,
        id,
    })
}

impl WorkspaceDirectory for DiskWorkspaces {
    fn list(&self) -> anyhow::Result<Vec<WorkspaceSummary>> {
        let dir = paths::workspaces_dir(&self.root);
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e.into()),
        };
        let mut out = Vec::new();
        for entry in entries {
            let path = entry?.path();
            if !path.join("workspace.json").is_file() {
                continue;
            }
            match read_meta(&path) {
                Ok(ws) => out.push(ws),
                Err(e) => {
                    tracing::warn!(dir = %path.display(), error = %e, "unreadable workspace.json")
                }
            }
        }
        out.sort_by(|a, b| {
            b.last_opened_at
                .cmp(&a.last_opened_at)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Ok(out)
    }

    fn last_workspace(&self) -> anyhow::Result<Option<String>> {
        let state: StateFile = match std::fs::read(paths::state_json(&self.root)) {
            Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => StateFile::default(),
            Err(e) => return Err(e.into()),
        };
        Ok(state.last_workspace.filter(|id| {
            paths::workspace_dir(&self.root, id)
                .join("workspace.json")
                .is_file()
        }))
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Writes a minimal `workspace.json` under `root`.
    pub(crate) fn write_ws(root: &Path, id: &str, name: &str, opened: i64, archived: bool) {
        let dir = paths::workspace_dir(root, id);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("workspace.json"),
            serde_json::json!({
                "id": id, "name": name, "slug": name.to_lowercase(), "icon": "brain",
                "hue": "teal", "archived": archived, "last_opened_at": opened,
                "created_at": 1, "description": "", "schema": 1
            })
            .to_string(),
        )
        .unwrap();
    }

    #[test]
    fn lists_workspaces_newest_first_and_skips_broken_ones() {
        let d = tempfile::tempdir().unwrap();
        let dir = DiskWorkspaces::new(d.path());
        assert!(dir.list().unwrap().is_empty());
        assert_eq!(dir.last_workspace().unwrap(), None);

        write_ws(d.path(), "ws_old", "Old", 10, true);
        write_ws(d.path(), "ws_new", "New", 20, false);
        let broken = paths::workspace_dir(d.path(), "ws_bad");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(broken.join("workspace.json"), "{").unwrap();
        std::fs::create_dir_all(paths::workspace_dir(d.path(), "ws_empty")).unwrap();

        let list = dir.list().unwrap();
        let ids: Vec<_> = list.iter().map(|w| w.id.as_str()).collect();
        assert_eq!(ids, ["ws_new", "ws_old"]);
        assert_eq!(
            list[1],
            WorkspaceSummary {
                id: "ws_old".into(),
                name: "Old".into(),
                slug: "old".into(),
                icon: "brain".into(),
                hue: "teal".into(),
                archived: true,
                last_opened_at: Some(10),
            }
        );
    }

    #[test]
    fn last_workspace_must_still_exist() {
        let d = tempfile::tempdir().unwrap();
        let dir = DiskWorkspaces::new(d.path());
        std::fs::write(
            paths::state_json(d.path()),
            r#"{"last_workspace":"ws_a","device_id":"x","schema":1}"#,
        )
        .unwrap();
        assert_eq!(dir.last_workspace().unwrap(), None);
        write_ws(d.path(), "ws_a", "A", 1, false);
        assert_eq!(dir.last_workspace().unwrap().as_deref(), Some("ws_a"));
    }
}
