//! `~/.learnpuppy` on disk: `settings.json` and the opened stores.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::Context;
use lp_store::{FsChatStore, FsGraphStore, SqliteTrackerStore};
use serde::{Deserialize, Serialize};

/// Model slot pins, extra Claude profiles and allowlisted repos.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// Role -> `claude:auto` | account id.
    pub pins: BTreeMap<String, String>,
    pub claude_dirs: Vec<PathBuf>,
    pub repos: Vec<PathBuf>,
}

impl Settings {
    fn path(root: &Path) -> PathBuf {
        root.join("settings.json")
    }

    pub fn load(root: &Path) -> anyhow::Result<Self> {
        match std::fs::read(Self::path(root)) {
            Ok(b) => serde_json::from_slice(&b).context("settings.json"),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e.into()),
        }
    }

    /// Atomic write: temp file, fsync, rename.
    pub fn save(&self, root: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(root)?;
        let path = Self::path(root);
        let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
        let mut f = std::fs::File::create(&tmp)?;
        f.write_all(&serde_json::to_vec_pretty(self)?)?;
        f.sync_all()?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }
}

pub struct Stores {
    pub graph: Arc<FsGraphStore>,
    pub chats: Arc<FsChatStore>,
    pub tracker: Arc<SqliteTrackerStore>,
}

impl Stores {
    pub fn open(root: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            graph: Arc::new(FsGraphStore::open(root.join("graph"))?),
            chats: Arc::new(FsChatStore::open(root.join("chats"))?),
            tracker: Arc::new(SqliteTrackerStore::open(root.join("tracker"))?),
        })
    }
}

pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_round_trip_and_default_when_missing() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(Settings::load(d.path()).unwrap(), Settings::default());
        let mut s = Settings::default();
        s.pins.insert("teacher".into(), "agy:default".into());
        s.save(d.path()).unwrap();
        assert_eq!(Settings::load(d.path()).unwrap(), s);
    }
}
