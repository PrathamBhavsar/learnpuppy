//! CLI accounts: discovery into the process-wide `AccountPool`, the account rows the UI lists
//! (never tokens), and registering extra Claude profiles.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Context};
use lp_cli_driver::accounts::{
    discover, AccountHealth, AccountPool, AccountState, DiscoverRoots, DiscoveredAccount,
    ProviderKind, Strategy,
};

use crate::settings::Settings;

pub fn provider_name(p: ProviderKind) -> &'static str {
    match p {
        ProviderKind::ClaudeCli => "claude",
        ProviderKind::CodexCli => "codex",
        ProviderKind::AgyCli => "agy",
    }
}

/// Account health as plain data; `Cooldown` carries the seconds left at snapshot time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Ready,
    AuthExpired,
    Cooldown { secs_left: u64, reason: String },
}

/// One account as the accounts screen shows it. Holds no credentials.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Account {
    pub id: String,
    pub provider: &'static str,
    pub email: Option<String>,
    pub tier: Option<String>,
    pub config_dir: Option<PathBuf>,
    pub status: Status,
    /// Requests in the current rate-limit window.
    pub requests: u32,
}

impl Account {
    pub fn from_state(a: AccountState, now: Instant) -> Self {
        let status = match a.health {
            AccountHealth::Healthy => Status::Ready,
            AccountHealth::AuthExpired => Status::AuthExpired,
            AccountHealth::Cooldown { until, reason } => Status::Cooldown {
                secs_left: until.saturating_duration_since(now).as_secs(),
                reason,
            },
        };
        Self {
            id: a.id.0,
            provider: provider_name(a.provider),
            email: a.email,
            tier: a.rate_limit_tier,
            config_dir: a.config_dir,
            status,
            requests: a.window_requests,
        }
    }
}

/// Every account in the pool, in priority order.
pub fn list(pool: &AccountPool) -> Vec<Account> {
    let now = Instant::now();
    pool.snapshot()
        .into_iter()
        .map(|a| Account::from_state(a, now))
        .collect()
}

/// Discovery roots: `home`, `$CLAUDE_CONFIG_DIR` and the profiles saved in settings.
pub fn roots(
    home: &Path,
    claude_config_dir: Option<PathBuf>,
    settings: &Settings,
) -> DiscoverRoots {
    DiscoverRoots {
        home: home.to_owned(),
        claude_config_dir,
        extra_claude_dirs: settings.claude_dirs.clone(),
        now_ms: u64::try_from(crate::now_ms()).unwrap_or(0),
    }
}

/// Discovers every logged-in CLI account into a fresh pool. Blocking (reads config files).
pub fn discover_pool(roots: &DiscoverRoots) -> Arc<AccountPool> {
    Arc::new(AccountPool::new(discover(roots), Strategy::Priority))
}

/// Validates that `path` holds a Claude login, then saves it to `settings.json` under `root`.
/// Returns the account found; adding the same profile twice saves it once.
pub fn add_claude(
    root: &Path,
    home: &Path,
    settings: &mut Settings,
    path: &Path,
) -> anyhow::Result<DiscoveredAccount> {
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
    Ok(acct)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_cli_driver::accounts::AccountId;

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
        let a = add_claude(&root, &home, &mut s, &prof).unwrap();
        assert_eq!(a.id.0, "claude:me@x.com");
        add_claude(&root, &home, &mut s, &prof).unwrap();
        assert_eq!(s.claude_dirs.len(), 1);
        assert_eq!(Settings::load(&root).unwrap(), s);

        let empty = d.path().join("empty");
        std::fs::create_dir_all(&empty).unwrap();
        assert!(add_claude(&root, &home, &mut s, &empty).is_err());
    }

    #[test]
    fn discovered_accounts_list_with_health() {
        let d = tempfile::tempdir().unwrap();
        profile(&d.path().join(".claude"), "a@x.com");
        let extra = d.path().join("work");
        profile(&extra, "b@x.com");
        let mut s = Settings::default();
        s.claude_dirs.push(extra);
        let pool = discover_pool(&roots(d.path(), None, &s));
        let rows = list(&pool);
        let ids: Vec<_> = rows.iter().map(|a| a.id.as_str()).collect();
        assert!(ids.contains(&"claude:a@x.com") && ids.contains(&"claude:b@x.com"));
        assert!(rows.iter().all(|a| a.provider == "claude"));
        assert!(rows.iter().all(|a| a.status == Status::Ready));

        pool.report_auth_expired(&AccountId("claude:a@x.com".into()));
        pool.report_rate_limit(
            &AccountId("claude:b@x.com".into()),
            "usage limit reached",
            Instant::now(),
            0,
        );
        let rows = list(&pool);
        let by = |id: &str| rows.iter().find(|a| a.id == id).unwrap().status.clone();
        // A runtime auth failure may be a false positive, so the pool retries it after a cooldown.
        assert!(
            matches!(by("claude:a@x.com"), Status::Cooldown { secs_left, reason } if secs_left > 0 && reason == "auth expired")
        );
        assert!(
            matches!(by("claude:b@x.com"), Status::Cooldown { reason, .. } if reason == "usage limit reached")
        );
    }
}
