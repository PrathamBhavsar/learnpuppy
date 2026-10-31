//! `server.lock`: one LearnPuppy server per data root (docs/web/01).
//!
//! The running server holds an exclusive `flock` on the file for its whole lifetime and writes its
//! pid/port into it. The OS drops the lock when the process dies, so a file whose lock can be taken
//! is stale no matter what pid it names (no pid-reuse false positives).

use std::fs::{File, OpenOptions, TryLockError};
use std::io::{Read, Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use super::{fsx, Result, WorkspaceError};
use crate::paths;

/// What the lock file says about its holder.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ServerLockInfo {
    pub pid: u32,
    /// `None` for a non-serving holder such as `learnpuppy migrate`.
    pub port: Option<u16>,
    /// Hash of the server token; never the token itself.
    #[serde(default)]
    pub token_hash: Option<String>,
    pub started_at: i64,
}

/// Result of [`ServerLock::status`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LockStatus {
    /// No lock file.
    Free,
    /// A live process holds the lock; `info` is `None` while it is still writing it.
    Live(Option<ServerLockInfo>),
    /// A lock file left by a process that is gone; acquiring replaces it.
    Stale(Option<ServerLockInfo>),
}

/// The held lock; released (and the file removed) on drop.
#[derive(Debug)]
pub struct ServerLock {
    file: File,
    path: PathBuf,
    info: ServerLockInfo,
}

fn read_info(f: &mut File) -> Option<ServerLockInfo> {
    let mut s = String::new();
    f.seek(SeekFrom::Start(0)).ok()?;
    f.read_to_string(&mut s).ok()?;
    serde_json::from_str(&s).ok()
}

impl ServerLock {
    /// Who, if anyone, holds `server.lock` under `root`.
    pub fn status(root: &Path) -> Result<LockStatus> {
        let path = paths::server_lock(root);
        let mut f = match File::open(&path) {
            Ok(f) => f,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(LockStatus::Free),
            Err(e) => return Err(e.into()),
        };
        let info = read_info(&mut f);
        match f.try_lock() {
            Ok(()) => {
                f.unlock()?;
                Ok(LockStatus::Stale(info))
            }
            Err(TryLockError::WouldBlock) => Ok(LockStatus::Live(info)),
            Err(TryLockError::Error(e)) => Err(e.into()),
        }
    }

    /// Take the lock for this process, replacing a stale file. A live holder is
    /// [`WorkspaceError::Conflict`] naming its pid and port.
    pub fn acquire(root: &Path, port: Option<u16>, token_hash: Option<String>) -> Result<Self> {
        fsx::create_dir(root)?;
        let path = paths::server_lock(root);
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .mode(0o600)
            .open(&path)?;
        match file.try_lock() {
            Ok(()) => {}
            Err(TryLockError::WouldBlock) => {
                let held = read_info(&mut file);
                return Err(WorkspaceError::Conflict(match held {
                    Some(i) => format!(
                        "LearnPuppy is already running (pid {}{})",
                        i.pid,
                        i.port.map(|p| format!(", port {p}")).unwrap_or_default()
                    ),
                    None => "LearnPuppy is already running".into(),
                }));
            }
            Err(TryLockError::Error(e)) => return Err(e.into()),
        }
        let info = ServerLockInfo {
            pid: std::process::id(),
            port,
            token_hash,
            started_at: crate::now_ms(),
        };
        let mut lock = Self { file, path, info };
        lock.write()?;
        Ok(lock)
    }

    fn write(&mut self) -> Result<()> {
        let bytes = serde_json::to_vec(&self.info)?;
        self.file.set_len(0)?;
        self.file.seek(SeekFrom::Start(0))?;
        self.file.write_all(&bytes)?;
        self.file.sync_all()?;
        Ok(())
    }

    /// Record the port once the listener is bound.
    pub fn set_port(&mut self, port: u16) -> Result<()> {
        self.info.port = Some(port);
        self.write()
    }

    pub fn info(&self) -> &ServerLockInfo {
        &self.info
    }
}

impl Drop for ServerLock {
    fn drop(&mut self) {
        // Remove while still holding the lock so nobody sees an unlocked file with our pid.
        let _ = std::fs::remove_file(&self.path);
        let _ = self.file.unlock();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_lock_blocks_a_second_holder_and_drop_frees_it() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(ServerLock::status(d.path()).unwrap(), LockStatus::Free);
        let mut held = ServerLock::acquire(d.path(), None, Some("h".into())).unwrap();
        held.set_port(4317).unwrap();
        match ServerLock::status(d.path()).unwrap() {
            LockStatus::Live(Some(i)) => {
                assert_eq!((i.pid, i.port), (std::process::id(), Some(4317)));
            }
            other => panic!("{other:?}"),
        }
        let err = ServerLock::acquire(d.path(), Some(1), None).unwrap_err();
        assert_eq!(err.code(), "conflict");
        assert!(err.to_string().contains("port 4317"), "{err}");
        drop(held);
        assert_eq!(ServerLock::status(d.path()).unwrap(), LockStatus::Free);
    }

    #[test]
    fn a_dead_holders_file_is_stale_and_gets_replaced() {
        let d = tempfile::tempdir().unwrap();
        // What a crashed server leaves behind: the file, but no flock (pid 1 is never us).
        let dead = ServerLockInfo {
            pid: 1,
            port: Some(9),
            token_hash: None,
            started_at: 0,
        };
        std::fs::write(
            paths::server_lock(d.path()),
            serde_json::to_vec(&dead).unwrap(),
        )
        .unwrap();
        assert_eq!(
            ServerLock::status(d.path()).unwrap(),
            LockStatus::Stale(Some(dead))
        );
        let held = ServerLock::acquire(d.path(), Some(10), None).unwrap();
        assert_eq!(held.info().pid, std::process::id());
        assert!(matches!(
            ServerLock::status(d.path()).unwrap(),
            LockStatus::Live(Some(ServerLockInfo { port: Some(10), .. }))
        ));
    }
}
