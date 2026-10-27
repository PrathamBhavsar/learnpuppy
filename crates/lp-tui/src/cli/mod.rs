//! `learnpuppy` command line: the TUI by default, headless subcommands otherwise (PLAN 5.2).

mod auth;
mod chat;
mod config;
mod graph;
mod ingest;
mod review;

use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand};
use lp_cli_driver::accounts::{discover, AccountPool, DiscoverRoots, Strategy};

use crate::backend::{Backend, DEFAULT_PIN};
use crate::engine::CliPoolEngine;
use crate::state::{now_ms, Settings, Stores};

#[derive(Parser)]
#[command(name = "learnpuppy", version)]
pub struct Cli {
    /// State directory (default `$LEARNPUPPY_HOME` or `~/.learnpuppy`).
    #[arg(long, global = true)]
    root: Option<PathBuf>,
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    #[command(subcommand)]
    Auth(auth::Cmd),
    #[command(subcommand)]
    Chat(chat::Cmd),
    #[command(subcommand)]
    Graph(graph::Cmd),
    #[command(subcommand)]
    Config(config::Cmd),
    Ingest(ingest::Args),
    Review(review::Args),
}

fn pool(home: &std::path::Path, settings: &Settings) -> Arc<AccountPool> {
    let roots = DiscoverRoots {
        home: home.to_owned(),
        claude_config_dir: std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from),
        extra_claude_dirs: settings.claude_dirs.clone(),
        now_ms: u64::try_from(now_ms()).unwrap_or(0),
    };
    Arc::new(AccountPool::new(discover(&roots), Strategy::Priority))
}

fn home() -> anyhow::Result<PathBuf> {
    Ok(PathBuf::from(
        std::env::var_os("HOME").context("HOME is not set")?,
    ))
}

impl Cli {
    /// The state directory: `--root`, else `$LEARNPUPPY_HOME`, else `~/.learnpuppy`.
    pub fn state_root(&self) -> anyhow::Result<PathBuf> {
        match self
            .root
            .clone()
            .or_else(|| std::env::var_os("LEARNPUPPY_HOME").map(PathBuf::from))
        {
            Some(r) => Ok(r),
            None => Ok(home()?.join(".learnpuppy")),
        }
    }
}

pub async fn run(cli: Cli) -> anyhow::Result<()> {
    let home = home()?;
    let root = cli.state_root()?;
    let stores = Stores::open(&root)?;
    let mut settings = Settings::load(&root)?;
    let device = settings.ensure_device(&root)?.to_owned();
    crate::review::sync(&stores.tracker, &device)?;
    let pool = pool(&home, &settings);
    let mut out = std::io::stdout();
    let engine_for = {
        let (pool, settings) = (pool.clone(), settings.clone());
        move |role: &str| {
            let pin = settings.pins.get(role).map_or(DEFAULT_PIN, String::as_str);
            Arc::new(CliPoolEngine::for_role(pool.clone(), &settings, role, pin))
        }
    };
    match cli.cmd {
        None => {
            let (p, cwd, st) = (pool.clone(), std::env::current_dir()?, settings.clone());
            let backend = Backend::new(root, cwd, stores, settings, pool, move |role, pin| {
                Arc::new(CliPoolEngine::for_role(p.clone(), &st, role, pin))
            });
            crate::app::run(Arc::new(backend)).await?;
        }
        Some(Cmd::Auth(auth::Cmd::Status { json })) => {
            let found: Vec<_> = discover(&DiscoverRoots {
                home,
                claude_config_dir: std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from),
                extra_claude_dirs: settings.claude_dirs.clone(),
                now_ms: u64::try_from(now_ms()).unwrap_or(0),
            });
            auth::status(&found, json, &mut out)?;
        }
        Some(Cmd::Auth(auth::Cmd::AddClaude { path })) => {
            auth::add_claude(&root, &home, &mut settings, &path, &mut out)?;
        }
        Some(Cmd::Config(cmd)) => config::run(&root, &mut settings, cmd, &mut out)?,
        Some(Cmd::Chat(cmd)) => {
            let cwd = std::env::current_dir()?;
            chat::run(stores.chats.as_ref(), &engine_for, &cwd, cmd, &mut out).await?;
        }
        Some(Cmd::Graph(cmd)) => {
            graph::run(stores.graph, engine_for("extractor"), cmd, &mut out).await?;
        }
        Some(Cmd::Ingest(args)) => {
            ingest::run(
                &stores,
                engine_for("distiller"),
                &root,
                &home,
                &mut settings,
                args,
                &mut out,
            )
            .await?;
            crate::review::export_cards(stores.tracker.as_ref(), &device)?;
        }
        Some(Cmd::Review(args)) => {
            let stdin = std::io::stdin();
            review::run(
                stores.tracker.as_ref(),
                &device,
                args,
                &mut stdin.lock(),
                &mut out,
            )?;
        }
    }
    out.flush()?;
    Ok(())
}
