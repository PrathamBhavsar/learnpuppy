//! CLI accounts: discovery into the process-wide `AccountPool`, the account rows the UI lists
//! (never tokens), and registering extra Claude profiles.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use anyhow::{bail, Context};
use lp_cli_driver::accounts::{
    discover, AccountHealth, AccountId, AccountPool, AccountState, AccountStatus, DiscoverRoots,
    DiscoveredAccount, ProviderKind, Strategy,
};

use crate::settings::Settings;

pub fn provider_name(p: ProviderKind) -> &'static str {
    match p {
        ProviderKind::ClaudeCli => "claude",
        ProviderKind::CodexCli => "codex",
        ProviderKind::AgyCli => "agy",
        ProviderKind::KiroCli => "kiro",
        ProviderKind::OpencodeCli => "opencode",
    }
}

/// The exact command the user runs themselves to connect `provider`. LearnPuppy never runs it.
pub fn how_to_connect(provider: &str) -> &'static str {
    match provider {
        "claude" => "claude login",
        "codex" => "codex login",
        "agy" => "agy auth login",
        "kiro" => "kiro-cli login",
        "opencode" => "opencode auth login",
        _ => "",
    }
}

/// Account health as plain data; `Cooldown` carries the seconds left at snapshot time.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Status {
    Ready,
    /// Logged out or the token expired: the user runs the connect command.
    AuthExpired,
    Cooldown {
        secs_left: u64,
        reason: String,
    },
    /// Excluded from the pool by the user.
    Disabled,
    /// No network. Says nothing about the account itself.
    Offline,
    /// Installed and logged in but no usable model.
    NoModel,
    /// The CLI disappeared from this machine.
    Missing,
}

/// One model of a multi-model account (kiro, opencode) with its own limit state.
#[derive(Clone, Debug, PartialEq)]
pub struct ModelRow {
    pub id: String,
    pub free: bool,
    pub credit_multiplier: Option<f64>,
    pub status: Status,
    /// Switched off by the user (`status` is then `Disabled`).
    pub disabled: bool,
    /// Unix epoch ms when this model's limit lifts.
    pub resets_at: Option<u64>,
}

/// One account as the accounts screen shows it. Holds no credentials.
#[derive(Clone, Debug, PartialEq)]
pub struct Account {
    pub id: String,
    pub provider: &'static str,
    pub email: Option<String>,
    pub tier: Option<String>,
    pub config_dir: Option<PathBuf>,
    pub status: Status,
    /// Unix epoch ms when the account's limit lifts (`Cooldown`).
    pub resets_at: Option<u64>,
    /// Requests in the current rate-limit window.
    pub requests: u32,
    /// Tokens in the current rate-limit window.
    pub tokens_in: u64,
    pub tokens_out: u64,
    /// Models the CLI lists for this account (empty for CLIs that pick their own).
    pub models: Vec<ModelRow>,
}

fn status_of(s: &AccountStatus, now_ms: u64, reason: &str) -> Status {
    match s {
        AccountStatus::Ready => Status::Ready,
        AccountStatus::CoolingDown { until } => Status::Cooldown {
            secs_left: until.saturating_sub(now_ms).div_ceil(1000),
            reason: reason.to_owned(),
        },
        AccountStatus::NeedsLogin => Status::AuthExpired,
        AccountStatus::Offline => Status::Offline,
        AccountStatus::NoModel => Status::NoModel,
        AccountStatus::Missing => Status::Missing,
    }
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
            resets_at: None,
            requests: a.window_requests,
            tokens_in: a.window_input_tokens,
            tokens_out: a.window_output_tokens,
            models: Vec::new(),
        }
    }
}

/// Every account in the pool, in priority order, with the pool's live status per account and model.
pub fn list(pool: &AccountPool) -> Vec<Account> {
    let now = Instant::now();
    let now_ms = u64::try_from(crate::now_ms()).unwrap_or(0);
    let rows = pool.statuses(now, now_ms);
    pool.snapshot()
        .into_iter()
        .map(|a| {
            let reason = match &a.health {
                AccountHealth::Cooldown { reason, .. } => reason.clone(),
                _ => "usage limit reached".to_owned(),
            };
            let mut acct = Account::from_state(a, now);
            if let Some(row) = rows.iter().find(|r| r.id.0 == acct.id) {
                if acct.status != Status::Disabled {
                    acct.status = status_of(&row.status, now_ms, &reason);
                }
                acct.resets_at = row.resets_at;
                acct.models = row
                    .models
                    .iter()
                    .map(|m| ModelRow {
                        id: m.id.clone(),
                        free: m.free,
                        credit_multiplier: m.credit_multiplier,
                        status: if m.disabled {
                            Status::Disabled
                        } else {
                            status_of(&m.status, now_ms, &reason)
                        },
                        disabled: m.disabled,
                        resets_at: m.resets_at,
                    })
                    .collect();
            }
            acct
        })
        .collect()
}

/// Model ids the CLIs listed for `provider`, across its accounts. Sorted, no duplicates.
pub fn discovered_models(pool: &AccountPool, provider: &str) -> Vec<String> {
    let mut out: Vec<String> = pool
        .snapshot()
        .into_iter()
        .filter(|a| provider_name(a.provider) == provider)
        .flat_map(|a| a.models.into_iter().map(|m| m.info.id))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// Free opencode models the CLI listed (the input of the "Use free models" preset).
pub fn free_opencode_models(pool: &AccountPool) -> Vec<String> {
    let mut out: Vec<String> = pool
        .snapshot()
        .into_iter()
        .filter(|a| a.provider == ProviderKind::OpencodeCli)
        .flat_map(|a| {
            a.models
                .into_iter()
                .filter(|m| m.info.free)
                .map(|m| m.info.id)
        })
        .collect();
    out.sort();
    out.dedup();
    out
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
        probe_clis: true,
        ..Default::default()
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

/// Re-applies the saved exclusions (accounts and models) to a freshly discovered pool.
pub fn apply_disabled(pool: &AccountPool, settings: &Settings) {
    for id in &settings.disabled_accounts {
        pool.set_disabled(&AccountId(id.clone()), true);
    }
    for key in &settings.disabled_models {
        if let Some((acct, model)) = key.split_once('|') {
            pool.set_model_disabled(&AccountId(acct.to_owned()), model, true);
        }
    }
}

/// Switches one model of `id` off (or on) and remembers it in `settings.json` under `root`.
/// `Ok(false)` when no such account or model is in the pool.
pub fn set_model_disabled(
    pool: &AccountPool,
    root: &Path,
    settings: &mut Settings,
    id: &str,
    model: &str,
    disabled: bool,
) -> anyhow::Result<bool> {
    if !pool.set_model_disabled(&AccountId(id.to_owned()), model, disabled) {
        return Ok(false);
    }
    let key = format!("{id}|{model}");
    settings.disabled_models.retain(|d| *d != key);
    if disabled {
        settings.disabled_models.push(key);
    }
    settings.save(root)?;
    Ok(true)
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

    /// `roots` without probing the real kiro/opencode on this machine.
    fn offline_roots(home: &Path, s: &Settings) -> DiscoverRoots {
        DiscoverRoots {
            probe_clis: false,
            ..roots(home, None, s)
        }
    }

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
        let pool = discover_pool(&offline_roots(d.path(), &s));
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
        // A runtime auth failure reads as signed out (and is retried after the pool's cooldown).
        assert_eq!(by("claude:a@x.com"), Status::AuthExpired);
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
        let pool = discover_pool(&offline_roots(d.path(), &s));

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
        let added = rescan(&pool, &offline_roots(d.path(), &s), &s);
        assert_eq!(added, ["claude:b@x.com"]);
        assert!(rescan(&pool, &offline_roots(d.path(), &s), &s).is_empty());
        let rows = list(&pool);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].status, Status::Disabled);
    }

    #[test]
    fn multi_model_accounts_list_models_status_and_reset_times() {
        use lp_cli_driver::accounts::{DiscoveredAccount, ModelInfo};
        use lp_cli_driver::engine::FailureKind;
        let acct = |id: &str, provider, ids: &[(&str, bool)]| DiscoveredAccount {
            id: AccountId(id.into()),
            provider,
            email: Some("k@x.com".into()),
            account_uuid: None,
            config_dir: None,
            rate_limit_tier: None,
            auth_expired: false,
            models: ids
                .iter()
                .map(|(m, free)| ModelInfo {
                    id: (*m).into(),
                    free: *free,
                    credit_multiplier: (!free).then_some(1.5),
                })
                .collect(),
        };
        let pool = AccountPool::new(
            vec![
                acct(
                    "opencode:opencode",
                    ProviderKind::OpencodeCli,
                    &[
                        ("opencode/big-pickle", true),
                        ("opencode/x-free", true),
                        ("opencode/paid", false),
                    ],
                ),
                acct("kiro:k@x.com", ProviderKind::KiroCli, &[("auto", false)]),
            ],
            Strategy::Priority,
        );
        let now_ms = u64::try_from(crate::now_ms()).unwrap();
        pool.report_failure(
            &AccountId("opencode:opencode".into()),
            None,
            &FailureKind::RateLimited {
                resets_at: Some(now_ms + 5 * 3_600_000),
            },
            "Free usage limit reached",
            Instant::now(),
            now_ms,
        );
        let rows = list(&pool);
        let oc = &rows[0];
        assert!(matches!(oc.status, Status::Cooldown { secs_left, .. } if secs_left > 4 * 3600));
        assert_eq!(oc.resets_at, Some(now_ms + 5 * 3_600_000));
        assert_eq!(oc.models.len(), 3);
        assert!(oc.models[0].free && !oc.models[2].free);
        assert_eq!(rows[1].models[0].credit_multiplier, Some(1.5));
        assert_eq!(rows[1].status, Status::Ready);
        assert_eq!(discovered_models(&pool, "opencode").len(), 3);
        assert_eq!(
            free_opencode_models(&pool),
            ["opencode/big-pickle", "opencode/x-free"]
        );
        // The user can disable the limited account; it stays Disabled, not Cooldown.
        pool.set_disabled(&AccountId("opencode:opencode".into()), true);
        assert_eq!(list(&pool)[0].status, Status::Disabled);
        pool.set_disabled(&AccountId("opencode:opencode".into()), false);
        let d = tempfile::tempdir().unwrap();
        let mut st = Settings::default();
        assert!(set_model_disabled(
            &pool,
            d.path(),
            &mut st,
            "opencode:opencode",
            "opencode/x-free",
            true
        )
        .unwrap());
        assert!(
            !set_model_disabled(&pool, d.path(), &mut st, "opencode:opencode", "nope", true)
                .unwrap()
        );
        let m = &list(&pool)[0].models[1];
        assert!(m.disabled && m.status == Status::Disabled);
        assert_eq!(
            Settings::load(d.path()).unwrap().disabled_models,
            ["opencode:opencode|opencode/x-free"]
        );
        assert_eq!(how_to_connect("kiro"), "kiro-cli login");
        assert_eq!(how_to_connect("opencode"), "opencode auth login");
    }
}
