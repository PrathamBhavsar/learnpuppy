//! The `learnpuppy` command line.

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

#[derive(Debug, Parser)]
#[command(
    name = "learnpuppy",
    version,
    about = "LearnPuppy: local web app for learning with a knowledge graph and spaced repetition"
)]
pub struct Cli {
    #[command(subcommand)]
    pub cmd: Cmd,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Engine {
    /// Local CLI account pool (`claude`, `codex`, `agy`).
    Real,
    /// Deterministic scripted engine for development and tests.
    Faux,
}

/// `$LP_ENGINE` selects the engine when `--engine` is not given.
pub const ENGINE_ENV: &str = "LP_ENGINE";

#[derive(Debug, Subcommand)]
pub enum Cmd {
    /// Start the local server on 127.0.0.1 and open the browser.
    Serve {
        /// Port to bind; 0 picks a free one. Busy ports auto-increment.
        #[arg(long, default_value_t = 7878)]
        port: u16,
        /// Development mode: no launch token, faux engine unless `--real`.
        #[arg(long)]
        dev: bool,
        /// With `--dev`, use the real CLI pool instead of the faux engine.
        #[arg(long, requires = "dev")]
        real: bool,
        /// Serve a fixture data root instead of `~/.learnpuppy`.
        #[arg(long)]
        fixture: Option<PathBuf>,
        /// LLM engine; defaults to `faux` with `--dev` (unless `--real`), else `real`.
        #[arg(long, value_enum, env = ENGINE_ENV)]
        engine: Option<Engine>,
        /// Do not open the browser.
        #[arg(long)]
        no_open: bool,
    },
    /// Move a single-store `~/.learnpuppy` into a "Default" workspace.
    Migrate {
        /// Undo a previous migration using its marker.
        #[arg(long)]
        revert: bool,
    },
    /// Write a deterministic fixture data root for tests and demos.
    Seed {
        /// Directory to create the fixture in.
        dir: PathBuf,
    },
    /// Check CLI logins, data root, disk and ports.
    Doctor,
    /// Print the OpenAPI document of the HTTP API.
    Openapi {
        /// Write to this file instead of stdout.
        #[arg(long)]
        out: Option<PathBuf>,
    },
}

impl Engine {
    /// The engine `serve` runs with: explicit choice, else faux in dev mode unless `--real`.
    pub fn resolve(engine: Option<Engine>, dev: bool, real: bool) -> Engine {
        match engine {
            Some(e) => e,
            None if dev && !real => Engine::Faux,
            None => Engine::Real,
        }
    }
}

impl Cmd {
    pub fn name(&self) -> &'static str {
        match self {
            Cmd::Serve { .. } => "serve",
            Cmd::Migrate { .. } => "migrate",
            Cmd::Seed { .. } => "seed",
            Cmd::Doctor => "doctor",
            Cmd::Openapi { .. } => "openapi",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    fn parse(args: &[&str]) -> Result<Cli, clap::Error> {
        Cli::try_parse_from(std::iter::once("learnpuppy").chain(args.iter().copied()))
    }

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn parses_every_subcommand() {
        match parse(&[
            "serve",
            "--dev",
            "--real",
            "--port",
            "0",
            "--engine",
            "faux",
            "--no-open",
        ])
        .unwrap()
        .cmd
        {
            Cmd::Serve {
                port,
                dev,
                real,
                fixture,
                engine,
                no_open,
            } => {
                assert_eq!((port, dev, real, no_open), (0, true, true, true));
                assert_eq!((fixture, engine), (None, Some(Engine::Faux)));
            }
            other => panic!("parsed {other:?}"),
        }
        assert!(matches!(
            parse(&["serve"]).unwrap().cmd,
            Cmd::Serve {
                port: 7878,
                dev: false,
                ..
            }
        ));
        assert!(matches!(
            parse(&["migrate", "--revert"]).unwrap().cmd,
            Cmd::Migrate { revert: true }
        ));
        assert!(matches!(
            parse(&["seed", "/tmp/x"]).unwrap().cmd,
            Cmd::Seed { .. }
        ));
        assert!(matches!(parse(&["doctor"]).unwrap().cmd, Cmd::Doctor));
        assert!(matches!(
            parse(&["openapi"]).unwrap().cmd,
            Cmd::Openapi { out: None }
        ));
    }

    #[test]
    fn engine_defaults_to_faux_only_in_dev() {
        assert_eq!(Engine::resolve(None, false, false), Engine::Real);
        assert_eq!(Engine::resolve(None, true, false), Engine::Faux);
        assert_eq!(Engine::resolve(None, true, true), Engine::Real);
        assert_eq!(
            Engine::resolve(Some(Engine::Faux), false, false),
            Engine::Faux
        );
        assert_eq!(
            Engine::resolve(Some(Engine::Real), true, false),
            Engine::Real
        );
    }

    #[test]
    fn rejects_bad_input() {
        assert!(parse(&[]).is_err());
        assert!(parse(&["serve", "--real"]).is_err(), "--real needs --dev");
        assert!(parse(&["serve", "--engine", "gpt"]).is_err());
        assert!(parse(&["serve", "--engine", "cli"]).is_err());
        assert!(parse(&["seed"]).is_err());
        let v = parse(&["--version"]).unwrap_err();
        assert_eq!(v.kind(), clap::error::ErrorKind::DisplayVersion);
    }
}
