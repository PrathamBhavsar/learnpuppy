//! `learnpuppy ingest [--since 7d] [--repo <path>]`.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use clap::Args as ClapArgs;
use lp_core::ports::TrackerStore;
use lp_ingest::{ingest, Roots};

use crate::state::{now_ms, Settings};

#[derive(ClapArgs)]
pub struct Args {
    /// Only sessions started within this window: `<n>h`, `<n>d` or `<n>w`.
    #[arg(long)]
    since: Option<String>,
    /// Repo to allowlist (saved to settings); repeatable.
    #[arg(long)]
    repo: Vec<PathBuf>,
}

fn since_ms(spec: &str, now: i64) -> anyhow::Result<i64> {
    lp_ingest::since_ms(spec, now).map_err(|e| anyhow::anyhow!(e))
}

pub fn run(
    tracker: &dyn TrackerStore,
    root: &Path,
    home: &Path,
    settings: &mut Settings,
    args: Args,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let mut changed = false;
    for r in args.repo {
        let r = r.canonicalize().with_context(|| r.display().to_string())?;
        if !settings.repos.contains(&r) {
            settings.repos.push(r);
            changed = true;
        }
    }
    if changed {
        settings.save(root)?;
    }
    if settings.repos.is_empty() {
        bail!("no repos allowlisted; pass --repo <path>");
    }
    let since = args.since.map_or(Ok(0), |s| since_ms(&s, now_ms()))?;
    let mut roots = Roots::from_home(home);
    roots
        .claude
        .extend(settings.claude_dirs.iter().map(|d| d.join("projects")));
    let rep = ingest(tracker, &roots, &settings.repos, since);
    writeln!(out, "files {}  sessions {}", rep.files, rep.sessions)?;
    for e in &rep.errors {
        writeln!(out, "error: {e}")?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_store::SqliteTrackerStore;

    #[test]
    fn parses_since_windows() {
        assert_eq!(since_ms("7d", 10 * 86_400_000).unwrap(), 3 * 86_400_000);
        assert_eq!(since_ms("2h", 3_600_000 * 3).unwrap(), 3_600_000);
        assert!(since_ms("7x", 0).is_err());
        assert!(since_ms("", 0).is_err());
    }

    #[test]
    fn requires_an_allowlisted_repo_then_scans() {
        let d = tempfile::tempdir().unwrap();
        let t = SqliteTrackerStore::open(d.path().join("t")).unwrap();
        let (root, home) = (d.path().join("lp"), d.path().join("home"));
        std::fs::create_dir_all(&home).unwrap();
        let mut s = Settings::default();
        let none = Args {
            since: None,
            repo: vec![],
        };
        assert!(run(&t, &root, &home, &mut s, none, &mut Vec::new()).is_err());

        let args = Args {
            since: Some("7d".into()),
            repo: vec![d.path().to_owned()],
        };
        let mut out = Vec::new();
        run(&t, &root, &home, &mut s, args, &mut out).unwrap();
        assert_eq!(s.repos.len(), 1);
        assert!(String::from_utf8(out).unwrap().contains("sessions 0"));
    }
}
