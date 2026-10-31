//! `learnpuppy`: the local LearnPuppy server binary.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use clap::Parser;
use lp_server::app::{EngineMode, HEARTBEAT};
use lp_server::assets::Assets;
use lp_server::cli::{Cli, Cmd, Engine};
use lp_server::{doctor, openapi, serve};
use lp_service::paths;
use tokio_util::sync::CancellationToken;

/// Exit code of a subcommand that exists but is not built yet.
const NOT_IMPLEMENTED: u8 = 2;

fn home() -> anyhow::Result<PathBuf> {
    std::env::var_os("HOME")
        .filter(|h| !h.is_empty())
        .map(PathBuf::from)
        .ok_or_else(|| anyhow::anyhow!("HOME is not set"))
}

/// JSON logs to `<root>/logs/server.<date>.log`; the guard flushes them on drop.
fn init_logs(root: &Path) -> Option<tracing_appender::non_blocking::WorkerGuard> {
    use tracing_appender::rolling::{Builder, Rotation};
    let dir = paths::logs_dir(root);
    std::fs::create_dir_all(&dir).ok()?;
    let appender = Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix("server")
        .filename_suffix("log")
        .max_log_files(14)
        .build(dir)
        .ok()?;
    let (writer, guard) = tracing_appender::non_blocking(appender);
    let filter = tracing_subscriber::EnvFilter::try_from_env("LP_LOG")
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .json()
        .with_writer(writer)
        .with_env_filter(filter)
        .try_init()
        .ok()?;
    Some(guard)
}

fn serve_cmd(
    port: u16,
    dev: bool,
    real: bool,
    fixture: Option<PathBuf>,
    engine: Option<Engine>,
    no_open: bool,
) -> anyhow::Result<ExitCode> {
    let engine = match Engine::resolve(engine, dev, real) {
        Engine::Real => EngineMode::Real,
        Engine::Faux => EngineMode::Faux,
    };
    // A fixture root is self-contained: accounts are discovered inside it, not in $HOME.
    let (data_root, home, claude_config_dir) = match fixture {
        Some(dir) => (dir.clone(), dir, None),
        None => (
            paths::data_root()?,
            home()?,
            std::env::var_os("CLAUDE_CONFIG_DIR")
                .filter(|v| !v.is_empty())
                .map(PathBuf::from),
        ),
    };
    std::fs::create_dir_all(&data_root)?;
    let _logs = init_logs(&data_root);
    if dev {
        eprintln!(
            "learnpuppy: --dev: no launch token; any local process can use the API on 127.0.0.1"
        );
    }
    let opts = serve::ServeOptions {
        port,
        dev,
        engine,
        data_root,
        home,
        claude_config_dir,
        assets: Assets::Embedded,
        heartbeat: HEARTBEAT,
    };
    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let stop = CancellationToken::new();
        serve::cancel_on_signals(stop.clone());
        serve::run(opts, stop, |launch| {
            let mut out = std::io::stdout().lock();
            if let Ok(line) = serde_json::to_string(launch) {
                let _ = writeln!(out, "{line}");
                let _ = out.flush();
            }
            if launch.existing {
                eprintln!("learnpuppy: already running at {}", launch.url);
            } else {
                eprintln!("learnpuppy: serving at {} (Ctrl+C to stop)", launch.url);
            }
            if !no_open {
                serve::open_browser(&launch.url);
            }
        })
        .await
    })?;
    Ok(ExitCode::SUCCESS)
}

fn doctor_cmd(out: &mut dyn Write) -> anyhow::Result<ExitCode> {
    let input = doctor::DoctorInput {
        data_root: paths::data_root()?,
        home: home()?,
        claude_config_dir: std::env::var_os("CLAUDE_CONFIG_DIR")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from),
        path_dirs: std::env::var_os("PATH")
            .map(|p| std::env::split_paths(&p).collect())
            .unwrap_or_default(),
        assets: Assets::Embedded,
        port: doctor::DEFAULT_PORT,
    };
    let ok = doctor::report(&doctor::checks(&input), out)?;
    Ok(if ok {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    })
}

fn run(cmd: Cmd, out: &mut dyn Write, err: &mut dyn Write) -> anyhow::Result<ExitCode> {
    match cmd {
        Cmd::Openapi { out: Some(path) } => {
            std::fs::write(&path, openapi::document_json()?)?;
            writeln!(out, "wrote {}", path.display())?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Openapi { out: None } => {
            out.write_all(openapi::document_json()?.as_bytes())?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Serve {
            port,
            dev,
            real,
            fixture,
            engine,
            no_open,
        } => serve_cmd(port, dev, real, fixture, engine, no_open),
        Cmd::Doctor => doctor_cmd(out),
        other @ (Cmd::Migrate { .. } | Cmd::Seed { .. }) => {
            writeln!(
                err,
                "learnpuppy {}: not implemented in this wave (web UI rollout, see docs/web/08-task-dag.md)",
                other.name()
            )?;
            Ok(ExitCode::from(NOT_IMPLEMENTED))
        }
    }
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    match run(cli.cmd, &mut std::io::stdout(), &mut std::io::stderr()) {
        Ok(code) => code,
        Err(e) => {
            eprintln!("learnpuppy: {e:#}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn go(cmd: Cmd) -> (ExitCode, String, String) {
        let (mut out, mut err) = (Vec::new(), Vec::new());
        let code = run(cmd, &mut out, &mut err).unwrap();
        (
            code,
            String::from_utf8(out).unwrap(),
            String::from_utf8(err).unwrap(),
        )
    }

    #[test]
    fn unbuilt_subcommands_say_so_and_fail() {
        for cmd in [
            Cmd::Migrate { revert: false },
            Cmd::Seed { dir: "x".into() },
        ] {
            let name = cmd.name();
            let (code, out, err) = go(cmd);
            assert_eq!(code, ExitCode::from(NOT_IMPLEMENTED));
            assert!(out.is_empty());
            assert!(
                err.starts_with(&format!("learnpuppy {name}: not implemented in this wave")),
                "{err}"
            );
        }
    }

    #[test]
    fn openapi_prints_or_writes_the_document() {
        let (code, out, _) = go(Cmd::Openapi { out: None });
        assert_eq!(code, ExitCode::SUCCESS);
        let doc: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert!(doc["paths"]["/health"].is_object());
        assert_eq!(doc["info"]["version"], env!("CARGO_PKG_VERSION"));

        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("openapi.json");
        let (code, out, _) = go(Cmd::Openapi {
            out: Some(path.clone()),
        });
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(out.starts_with("wrote "));
        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            openapi::document_json().unwrap()
        );
    }
}
