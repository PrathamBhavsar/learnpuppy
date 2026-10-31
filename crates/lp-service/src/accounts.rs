//! CLI accounts: discovery into the process-wide `AccountPool`, the account rows the UI lists
//! (never tokens), and registering extra Claude profiles.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Context};
use lp_cli_driver::accounts::{
    discover, AccountHealth, AccountId, AccountPool, AccountState, DiscoverRoots,
    DiscoveredAccount, ProviderKind, Strategy,
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
    Cooldown {
        secs_left: u64,
        reason: String,
    },
    /// Excluded from the pool by the user.
    Disabled,
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
    /// Tokens in the current rate-limit window.
    pub tokens_in: u64,
    pub tokens_out: u64,
}

impl Account {
    pub fn from_state(a: AccountState, now: Instant) -> Self {
        let status = match a.health {
            _ if a.disabled => Status::Disabled,
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
            tokens_in: a.window_input_tokens,
            tokens_out: a.window_output_tokens,
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

/// Excludes or re-includes `id` and remembers the choice in `settings.json` under `root`.
/// `Ok(false)` when no such account is in the pool.
pub fn set_disabled(
    pool: &AccountPool,
    root: &Path,
    settings: &mut Settings,
    id: &str,
    disabled: bool,
) -> anyhow::Result<bool> {
    if !pool.set_disabled(&AccountId(id.to_owned()), disabled) {
        return Ok(false);
    }
    settings.disabled_accounts.retain(|d| d != id);
    if disabled {
        settings.disabled_accounts.push(id.to_owned());
    }
    settings.save(root)?;
    Ok(true)
}

/// Re-applies the saved exclusions to a freshly discovered pool.
pub fn apply_disabled(pool: &AccountPool, settings: &Settings) {
    for id in &settings.disabled_accounts {
        pool.set_disabled(&AccountId(id.clone()), true);
    }
}

/// Re-runs discovery into the live pool. Returns the ids of accounts that appeared.
pub fn rescan(pool: &AccountPool, roots: &DiscoverRoots, settings: &Settings) -> Vec<String> {
    let added = pool.merge_discovered(discover(roots));
    apply_disabled(pool, settings);
    added.into_iter().map(|id| id.0).collect()
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

    #[test]
    fn disabling_persists_and_rescan_finds_new_profiles() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("lp");
        profile(&d.path().join(".claude"), "a@x.com");
        let mut s = Settings::default();
        let pool = discover_pool(&roots(d.path(), None, &s));

        assert!(set_disabled(&pool, &root, &mut s, "claude:a@x.com", true).unwrap());
        assert!(!set_disabled(&pool, &root, &mut s, "claude:nobody", true).unwrap());
        assert_eq!(list(&pool)[0].status, Status::Disabled);
        assert_eq!(
            pool.acquire(&[ProviderKind::ClaudeCli], Instant::now()),
            None
        );
        assert_eq!(
            Settings::load(&root).unwrap().disabled_accounts,
            ["claude:a@x.com"]
        );
        assert!(set_disabled(&pool, &root, &mut s, "claude:a@x.com", false).unwrap());
        assert_eq!(list(&pool)[0].status, Status::Ready);
        assert!(Settings::load(&root).unwrap().disabled_accounts.is_empty());

        // A profile saved after start shows up on rescan; the disabled one stays disabled.
        let extra = d.path().join("work");
        profile(&extra, "b@x.com");
        s.claude_dirs.push(extra);
        s.disabled_accounts.push("claude:b@x.com".into());
        let added = rescan(&pool, &roots(d.path(), None, &s), &s);
        assert_eq!(added, ["claude:b@x.com"]);
        assert!(rescan(&pool, &roots(d.path(), None, &s), &s).is_empty());
        let rows = list(&pool);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].status, Status::Disabled);
    }
}
