//! `state.json`: process-global state shared by all workspaces (`last_workspace`, `device_id`).

use std::path::Path;

use serde::{Deserialize, Serialize};

use super::{fsx, Result};
use crate::paths;
use crate::settings::Settings;

pub const STATE_SCHEMA: u32 = 1;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct State {
    /// Where `/` redirects when it should skip the picker.
    #[serde(default)]
    pub last_workspace: Option<String>,
    /// This install's FSRS replication id (`sync-<device>.jsonl`).
    #[serde(default)]
    pub device_id: String,
    #[serde(default = "schema")]
    pub schema: u32,
}

fn schema() -> u32 {
    STATE_SCHEMA
}

impl State {
    /// Read `state.json`, creating it on first run. A missing device id is taken from the legacy
    /// `settings.json` (so FSRS replication keeps its identity), else minted.
    pub fn load_or_init(root: &Path) -> Result<Self> {
        let path = paths::state_json(root);
        let existing: Option<Self> = fsx::read_json(&path)?;
        let mut state = existing.clone().unwrap_or(Self {
            last_workspace: None,
            device_id: String::new(),
            schema: STATE_SCHEMA,
        });
        if state.device_id.is_empty() {
            let legacy = Settings::load(root)
                .map(|s| s.device_id)
                .unwrap_or_default();
            state.device_id = if legacy.is_empty() {
                format!("dev{:x}{:08x}", crate::now_ms(), rand::random::<u32>())
            } else {
                legacy
            };
        }
        if existing.as_ref() != Some(&state) {
            state.save(root)?;
        }
        Ok(state)
    }

    /// Atomic write of `state.json`.
    pub fn save(&self, root: &Path) -> Result<()> {
        fsx::create_dir(root)?;
        fsx::write_json(&paths::state_json(root), self)
    }

    /// Read-modify-write of `state.json`.
    pub fn update(root: &Path, f: impl FnOnce(&mut Self)) -> Result<Self> {
        let mut state = Self::load_or_init(root)?;
        let before = state.clone();
        f(&mut state);
        if state != before {
            state.save(root)?;
        }
        Ok(state)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_run_mints_a_device_and_later_runs_keep_it() {
        let d = tempfile::tempdir().unwrap();
        let s = State::load_or_init(d.path()).unwrap();
        assert!(s.device_id.starts_with("dev") && s.last_workspace.is_none());
        assert!(paths::state_json(d.path()).is_file());
        let s2 = State::update(d.path(), |s| s.last_workspace = Some("ws_1".into())).unwrap();
        assert_eq!(s2.device_id, s.device_id);
        assert_eq!(
            State::load_or_init(d.path())
                .unwrap()
                .last_workspace
                .as_deref(),
            Some("ws_1")
        );
    }

    #[test]
    fn legacy_settings_device_id_is_adopted() {
        let d = tempfile::tempdir().unwrap();
        let settings = Settings {
            device_id: "devlegacy".into(),
            ..Settings::default()
        };
        settings.save(d.path()).unwrap();
        assert_eq!(
            State::load_or_init(d.path()).unwrap().device_id,
            "devlegacy"
        );
    }
}
