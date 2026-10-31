//! Small crash-safe filesystem helpers shared by the workspace modules.

use std::fs::{self, DirBuilder, File, OpenOptions};
use std::io::Write;
use std::os::unix::fs::{DirBuilderExt, OpenOptionsExt};
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::Serialize;

use super::Result;

/// Create `dir` and missing parents owner-only, like `lp-store` does for its trees.
pub(crate) fn create_dir(dir: &Path) -> Result<()> {
    DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    Ok(())
}

/// fsync a directory so a rename or create inside it survives a crash.
pub(crate) fn sync_dir(dir: &Path) -> Result<()> {
    File::open(dir)?.sync_all()?;
    Ok(())
}

/// Replace `path` atomically: owner-only temp file in the same dir, fsync, rename, fsync dir.
/// A failure (disk full included) leaves the previous file untouched.
pub(crate) fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().ok_or_else(|| {
        super::WorkspaceError::Internal(format!("{} has no parent", path.display()))
    })?;
    let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("file");
    let tmp = dir.join(format!(".{name}.tmp.{}", std::process::id()));
    let res = (|| -> Result<()> {
        let mut f = OpenOptions::new()
            .create(true)
            .truncate(true)
            .write(true)
            .mode(0o600)
            .open(&tmp)?;
        f.write_all(bytes)?;
        f.sync_all()?;
        fs::rename(&tmp, path)?;
        sync_dir(dir)
    })();
    if res.is_err() {
        let _ = fs::remove_file(&tmp);
    }
    res
}

/// Pretty JSON, atomically.
pub(crate) fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    write_atomic(path, &bytes)
}

/// `Ok(None)` when the file does not exist.
pub(crate) fn read_json<T: DeserializeOwned>(path: &Path) -> Result<Option<T>> {
    match fs::read(path) {
        Ok(b) => Ok(Some(serde_json::from_slice(&b)?)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.into()),
    }
}

/// Rename `from` to `to` (same filesystem: atomic, no copy) and fsync both parents.
pub(crate) fn rename(from: &Path, to: &Path) -> Result<()> {
    fs::rename(from, to)?;
    for dir in [from.parent(), to.parent()].into_iter().flatten() {
        sync_dir(dir)?;
    }
    Ok(())
}

/// Whether `path` exists without following a final symlink.
pub(crate) fn exists(path: &Path) -> bool {
    fs::symlink_metadata(path).is_ok()
}
