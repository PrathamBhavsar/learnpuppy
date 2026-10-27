//! `learnpuppy auth status | add-claude`.

use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use clap::Subcommand;
use lp_cli_driver::accounts::{discover, DiscoverRoots, DiscoveredAccount, ProviderKind};
use serde_json::json;

use crate::state::Settings;

#[derive(Subcommand)]
pub enum Cmd {
    /// List every discovered CLI login and whether its token is still valid.
    Status {
        #[arg(long)]
        json: bool,
    },
    /// Register an additional `CLAUDE_CONFIG_DIR` profile.
    AddClaude { path: PathBuf },
}

fn provider(p: ProviderKind) -> &'static str {
    match p {
        ProviderKind::ClaudeCli => "claude",
        ProviderKind::CodexCli => "codex",
        ProviderKind::AgyCli => "agy",
    }
}

pub fn status(
    accounts: &[DiscoveredAccount],
    json: bool,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    if json {
        let rows: Vec<_> = accounts
            .iter()
            .map(|a| {
                json!({
                    "id": a.id.0,
                    "provider": provider(a.provider),
                    "email": a.email,
                    "config_dir": a.config_dir,
                    "tier": a.rate_limit_tier,
                    "auth_expired": a.auth_expired,
                })
            })
            .collect();
        writeln!(out, "{}", serde_json::to_string_pretty(&rows)?)?;
        return Ok(());
    }
    if accounts.is_empty() {
        writeln!(
            out,
            "no CLI logins found (log in with `claude`, `codex` or `agy`)"
        )?;
    }
    for a in accounts {
        let health = if a.auth_expired { "auth expired" } else { "ok" };
        writeln!(out, "{:<32} {:<7} {health}", a.id.0, provider(a.provider))?;
    }
    Ok(())
}

/// Validates that `path` holds a Claude login, then saves it to `settings.json`.
pub fn add_claude(
    root: &Path,
    home: &Path,
    settings: &mut Settings,
    path: &Path,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let dir = path
        .canonicalize()
        .with_context(|| path.display().to_string())?;
    let roots = DiscoverRoots {
        home: home.to_owned(),
        extra_claude_dirs: vec![dir.clone()],
        ..DiscoverRoots::default()
    };
    let Some(acct) = discover(&roots)
        .into_iter()
        .find(|a| a.config_dir.as_deref() == Some(dir.as_path()))
    else {
        bail!("no Claude login found in {}", dir.display());
    };
    if !settings.claude_dirs.contains(&dir) {
        settings.claude_dirs.push(dir);
        settings.save(root)?;
    }
    writeln!(out, "added {}", acct.id.0)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn profile(dir: &Path, email: &str) {
        std::fs::create_dir_all(dir).unwrap();
        std::fs::write(
            dir.join(".claude.json"),
            format!(r#"{{"oauthAccount":{{"emailAddress":"{email}"}}}}"#),
        )
        .unwrap();
    }

    #[test]
    fn add_claude_registers_profile_once_and_rejects_empty_dir() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("lp");
        let home = d.path().join("home");
        let prof = d.path().join("p");
        std::fs::create_dir_all(&home).unwrap();
        profile(&prof, "me@x.com");
        let mut s = Settings::default();
        let mut out = Vec::new();
        add_claude(&root, &home, &mut s, &prof, &mut out).unwrap();
        add_claude(&root, &home, &mut s, &prof, &mut out).unwrap();
        assert_eq!(s.claude_dirs.len(), 1);
        assert_eq!(Settings::load(&root).unwrap(), s);
        assert!(String::from_utf8(out)
            .unwrap()
            .contains("added claude:me@x.com"));

        let empty = d.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(add_claude(&root, &home, &mut s, &empty, &mut Vec::new()).is_err());
    }

    #[test]
    fn status_prints_json_and_text() {
        let d = tempfile::tempdir().unwrap();
        profile(&d.path().join(".claude"), "a@x.com");
        let accts = discover(&DiscoverRoots {
            home: d.path().to_owned(),
            ..DiscoverRoots::default()
        });
        let mut j = Vec::new();
        status(&accts, true, &mut j).unwrap();
        let v: serde_json::Value = serde_json::from_slice(&j).unwrap();
        assert_eq!(v[0]["id"], "claude:a@x.com");
        let mut t = Vec::new();
        status(&accts, false, &mut t).unwrap();
        assert!(String::from_utf8(t).unwrap().contains("claude:a@x.com"));
    }
}
