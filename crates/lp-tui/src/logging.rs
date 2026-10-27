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

    #[tokio::test]
    async fn a_cli_call_logs_provider_model_effort_and_no_prompt() {
        use lp_cli_driver::engine::{CliKind, LlmEngine, SubprocessEngine, TurnRequest};
        use std::os::unix::fs::PermissionsExt;

        let d = tempfile::tempdir().unwrap();
        let mock = d.path().join("mock-cli");
        std::fs::write(
            &mock,
            "#!/bin/sh\ncat >/dev/null\necho '{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"ok\"}]}}'\n",
        )
        .unwrap();
        std::fs::set_permissions(&mock, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut e = SubprocessEngine::new(CliKind::Claude).unwrap();
        e.program = mock;
        e.role = "teacher".into();
        e.account = "claude:main".into();
        e.model = Some("sonnet-x".into());
        e.effort = Some("high".into());

        let (sub, guard) = subscriber(d.path()).unwrap();
        let _g = tracing::subscriber::set_default(sub);
        let (tx, mut rx) = tokio::sync::mpsc::channel(8);
        let req = TurnRequest {
            prompt: "SECRET-PROMPT-TEXT".into(),
            ..Default::default()
        };
        e.stream_turn(req, tx, tokio_util::sync::CancellationToken::new())
            .await
            .unwrap();
        while rx.try_recv().is_ok() {}
        drop(guard);

        let log = std::fs::read_dir(d.path().join("logs"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap();
        let text = std::fs::read_to_string(log.path()).unwrap();
        for want in [
            "role=teacher",
            "provider=Claude",
            "account=claude:main",
            "model=sonnet-x",
            "effort=high",
            "cli turn end",
            "status=ok",
        ] {
            assert!(text.contains(want), "{want} missing: {text}");
        }
        assert!(!text.contains("SECRET-PROMPT-TEXT"), "{text}");
    }
}
