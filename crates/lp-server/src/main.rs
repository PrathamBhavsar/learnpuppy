//! `learnpuppy`: the local LearnPuppy server binary.

mod cli;

use std::io::Write;
use std::process::ExitCode;

use clap::Parser;

use crate::cli::{Cli, Cmd};

/// Exit code of a subcommand that exists but is not built yet.
const NOT_IMPLEMENTED: u8 = 2;

/// The OpenAPI document of the HTTP API. No routes exist yet, so `paths` is empty.
fn openapi_document() -> serde_json::Value {
    serde_json::json!({
        "openapi": "3.1.0",
        "info": {
            "title": "LearnPuppy",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "servers": [{ "url": "/api/v1" }],
        "paths": {},
    })
}

fn run(cmd: Cmd, out: &mut dyn Write, err: &mut dyn Write) -> std::io::Result<ExitCode> {
    match cmd {
        Cmd::Openapi { out: Some(path) } => {
            let mut doc = serde_json::to_string_pretty(&openapi_document())?;
            doc.push('\n');
            std::fs::write(&path, doc)?;
            writeln!(out, "wrote {}", path.display())?;
            Ok(ExitCode::SUCCESS)
        }
        Cmd::Openapi { out: None } => {
            serde_json::to_writer_pretty(&mut *out, &openapi_document())?;
            writeln!(out)?;
            Ok(ExitCode::SUCCESS)
        }
        other => {
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
            eprintln!("learnpuppy: {e}");
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
            Cmd::Serve {
                port: 7878,
                dev: false,
                real: false,
                fixture: None,
                engine: None,
            },
            Cmd::Migrate { revert: false },
            Cmd::Seed { dir: "x".into() },
            Cmd::Doctor,
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
        assert_eq!(doc["openapi"], "3.1.0");
        assert_eq!(doc["info"]["version"], env!("CARGO_PKG_VERSION"));

        let d = tempfile::tempdir().unwrap();
        let path = d.path().join("openapi.json");
        let (code, out, _) = go(Cmd::Openapi {
            out: Some(path.clone()),
        });
        assert_eq!(code, ExitCode::SUCCESS);
        assert!(out.starts_with("wrote "));
        let written: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        assert_eq!(written, doc);
    }
}
