//! Where LearnPuppy keeps its data (docs/web/02 disk layout).
//!
//! ```text
//! <data root>/                 $LEARNPUPPY_HOME, else ~/.learnpuppy
//!   state.json  settings.json  server.lock  logs/  .trash/
//!   workspaces/<ws_id>/        one isolated store root per workspace
//! ```

use std::ffi::OsString;
use std::path::{Path, PathBuf};

/// Overrides the data root; also honoured by `lp-cli-driver` for its raw debug logs.
pub const HOME_ENV: &str = "LEARNPUPPY_HOME";

/// The data root from explicit env values: `$LEARNPUPPY_HOME`, else `$HOME/.learnpuppy`.
pub fn resolve_data_root(
    learnpuppy_home: Option<OsString>,
    home: Option<OsString>,
) -> anyhow::Result<PathBuf> {
    match (learnpuppy_home.filter(|v| !v.is_empty()), home) {
        (Some(root), _) => Ok(PathBuf::from(root)),
        (None, Some(home)) if !home.is_empty() => Ok(PathBuf::from(home).join(".learnpuppy")),
        _ => anyhow::bail!("neither {HOME_ENV} nor HOME is set"),
    }
}

/// The data root of this process.
pub fn data_root() -> anyhow::Result<PathBuf> {
    resolve_data_root(std::env::var_os(HOME_ENV), std::env::var_os("HOME"))
}

/// The directory holding one subdirectory per workspace.
pub fn workspaces_dir(root: &Path) -> PathBuf {
    root.join("workspaces")
}

/// One workspace's store root.
pub fn workspace_dir(root: &Path, id: &str) -> PathBuf {
    workspaces_dir(root).join(id)
}

/// Process-global state: last workspace, device id, schema.
pub fn state_json(root: &Path) -> PathBuf {
    root.join("state.json")
}

/// Global profiles and pins (`settings::Settings`).
pub fn settings_json(root: &Path) -> PathBuf {
    root.join("settings.json")
}

/// Pid, port and token hash of the running server.
pub fn server_lock(root: &Path) -> PathBuf {
    root.join("server.lock")
}

/// The running server's session token (0600), so a second `serve` can build a working URL.
pub fn server_token(root: &Path) -> PathBuf {
    root.join("server.token")
}

/// One workspace's learning directory (bouts, check-ins, register, profile, schedule).
pub fn learning_dir(ws_root: &Path) -> PathBuf {
    ws_root.join("learning")
}

pub fn logs_dir(root: &Path) -> PathBuf {
    root.join("logs")
}

/// Deleted workspaces wait here until purged.
pub fn trash_dir(root: &Path) -> PathBuf {
    root.join(".trash")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn env_override_wins_then_home_then_error() {
        let root = resolve_data_root(Some("/data/lp".into()), Some("/home/me".into())).unwrap();
        assert_eq!(root, Path::new("/data/lp"));
        let root = resolve_data_root(Some("".into()), Some("/home/me".into())).unwrap();
        assert_eq!(root, Path::new("/home/me/.learnpuppy"));
        assert!(resolve_data_root(None, None).is_err());
        assert!(resolve_data_root(None, Some("".into())).is_err());
    }

    #[test]
    fn layout_matches_the_workspace_spec() {
        let r = Path::new("/r");
        assert_eq!(state_json(r), Path::new("/r/state.json"));
        assert_eq!(settings_json(r), Path::new("/r/settings.json"));
        assert_eq!(server_lock(r), Path::new("/r/server.lock"));
        assert_eq!(
            learning_dir(Path::new("/r/workspaces/w")),
            Path::new("/r/workspaces/w/learning")
        );
        assert_eq!(logs_dir(r), Path::new("/r/logs"));
        assert_eq!(trash_dir(r), Path::new("/r/.trash"));
        assert_eq!(workspaces_dir(r), Path::new("/r/workspaces"));
        assert_eq!(
            workspace_dir(r, "ws_a1b2c3d4"),
            Path::new("/r/workspaces/ws_a1b2c3d4")
        );
    }
}
