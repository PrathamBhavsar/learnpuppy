//! Export file ownership and short-lived one-shot download capabilities.
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const MAX_IMPORT_BYTES: u64 = 2 << 30;
const DOWNLOAD_TTL: Duration = Duration::from_secs(15 * 60);

#[derive(Default)]
pub struct ExportJobs {
    downloads: Mutex<HashMap<String, (Instant, PathBuf)>>,
}
impl ExportJobs {
    pub fn ready(&self, path: PathBuf) -> String {
        let mut downloads = self.downloads.lock();
        downloads.retain(|_, (created, path)| {
            if created.elapsed() < DOWNLOAD_TTL {
                true
            } else {
                let _ = std::fs::remove_file(path);
                false
            }
        });
        let token = format!("{:032x}", rand::random::<u128>());
        downloads.insert(token.clone(), (Instant::now(), path));
        token
    }
    pub fn take(&self, token: &str) -> Option<Download> {
        let (at, path) = self.downloads.lock().remove(token)?;
        let download = Download { path };
        (at.elapsed() < DOWNLOAD_TTL).then_some(download)
    }
}
impl Drop for ExportJobs {
    fn drop(&mut self) {
        for (_, (_, path)) in self.downloads.get_mut().drain() {
            cleanup_file(path);
        }
    }
}
/// Deletes the backing file when the response stream finishes or the client disconnects.
pub struct Download {
    pub path: PathBuf,
}
impl Drop for Download {
    fn drop(&mut self) {
        cleanup_file(self.path.clone());
    }
}

fn cleanup_file(path: PathBuf) {
    if let Ok(runtime) = tokio::runtime::Handle::try_current() {
        std::mem::drop(runtime.spawn_blocking(move || {
            let _ = std::fs::remove_file(path);
        }));
    } else {
        let _ = std::fs::remove_file(path);
    }
}

pub fn job_path(data_root: &Path, job: &str) -> anyhow::Result<PathBuf> {
    let dir = data_root.join("exports");
    std::fs::create_dir_all(&dir)?;
    Ok(dir.join(format!("{job}.zip")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn download_capabilities_are_one_shot_expire_and_cleanup() {
        let root = tempfile::tempdir().expect("tempdir");
        let file = root.path().join("archive.zip");
        std::fs::write(&file, b"zip").expect("archive");
        let jobs = ExportJobs::default();
        let token = jobs.ready(file.clone());
        let download = jobs.take(&token).expect("first download");
        assert!(jobs.take(&token).is_none());
        assert!(file.exists());
        drop(download);
        assert!(!file.exists());
        std::fs::write(&file, b"zip").expect("archive");
        let token = jobs.ready(file.clone());
        jobs.downloads.lock().get_mut(&token).expect("entry").0 = Instant::now() - DOWNLOAD_TTL;
        assert!(jobs.take(&token).is_none());
        assert!(!file.exists());
        std::fs::write(&file, b"zip").expect("archive");
        jobs.ready(file.clone());
        drop(jobs);
        assert!(!file.exists());
    }
}
