//! File logging under `<root>/logs`; the TUI owns the terminal, so nothing goes to stderr.

use std::os::unix::fs::DirBuilderExt;
use std::path::Path;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::EnvFilter;

/// A daily-rolling file subscriber plus the guard that flushes it on drop.
fn subscriber(
    root: &Path,
) -> anyhow::Result<(impl tracing::Subscriber + Send + Sync, WorkerGuard)> {
    let dir = root.join("logs");
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(&dir)?;
    let (writer, guard) =
        tracing_appender::non_blocking(tracing_appender::rolling::daily(dir, "learnpuppy.log"));
    let sub = tracing_subscriber::fmt()
        .with_writer(writer)
        .with_ansi(false)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()))
        .finish();
    Ok((sub, guard))
}

/// Installs the global subscriber; keep the guard alive for the process lifetime.
pub fn init(root: &Path) -> anyhow::Result<WorkerGuard> {
    let (sub, guard) = subscriber(root)?;
    tracing::subscriber::set_global_default(sub)?;
    Ok(guard)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn events_land_in_a_log_file_under_root() {
        let d = tempfile::tempdir().unwrap();
        let (sub, guard) = subscriber(d.path()).unwrap();
        tracing::subscriber::with_default(sub, || tracing::warn!("marker-event"));
        drop(guard);
        let logs: Vec<_> = std::fs::read_dir(d.path().join("logs")).unwrap().collect();
        assert_eq!(logs.len(), 1);
        let text = std::fs::read_to_string(logs[0].as_ref().unwrap().path()).unwrap();
        assert!(text.contains("marker-event"), "{text}");
    }
}
