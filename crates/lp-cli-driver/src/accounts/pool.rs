use std::path::PathBuf;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use uuid::Uuid;

use super::{AccountId, DiscoveredAccount, ModelInfo, ProviderKind};
use crate::engine::{self, FailureKind};

const BASE_COOLDOWN: Duration = Duration::from_secs(60);
const MAX_COOLDOWN: Duration = Duration::from_secs(3600);
/// Upper bound for a cooldown parsed from CLI output (a hostile message can claim anything).
const MAX_PARSED_COOLDOWN: Duration = engine::MAX_PARSED_WAIT;
/// While offline, retry no sooner than this, doubling per consecutive failure.
const OFFLINE_BASE: Duration = Duration::from_secs(5);
const OFFLINE_MAX: Duration = Duration::from_secs(120);
/// A model the CLI says does not exist stays benched this long (the list may be refreshed sooner).
const NO_MODEL_BENCH: Duration = Duration::from_secs(3600);
const AUTH_EXPIRED_REASON: &str = "auth expired";

/// What an account (or one of its models) can do right now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccountStatus {
    Ready,
    /// Rate limited until this unix epoch millisecond.
    CoolingDown {
        until: u64,
    },
    /// Installed but logged out / token expired.
    NeedsLogin,
    /// No network. Global: says nothing about the account itself.
    Offline,
    /// No usable model (unknown id, not entitled, empty list).
    NoModel,
    /// The CLI is not installed.
    Missing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ModelStatusRow {
    pub id: String,
    /// The user switched this model off; it is never picked (`status` reads `NoModel`).
    pub disabled: bool,
    pub free: bool,
    pub credit_multiplier: Option<f64>,
    pub status: AccountStatus,
    /// Unix epoch ms when a rate limit on this model lifts.
    pub resets_at: Option<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AccountStatusRow {
    pub id: AccountId,
    pub provider: ProviderKind,
    pub status: AccountStatus,
    pub resets_at: Option<u64>,
    pub models: Vec<ModelStatusRow>,
}

/// One model of an account with its failure state.
#[derive(Debug, Clone)]
pub struct ModelState {
    pub info: ModelInfo,
    pub until: Option<Instant>,
    pub resets_at_ms: Option<u64>,
    pub unavailable: bool,
    /// Switched off by the user.
    pub disabled: bool,
}

/// An account and, for multi-model CLIs, the model to run on it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pick {
    pub account: AccountId,
    pub model: Option<String>,
}

#[derive(Debug, Default)]
struct Offline {
    until: Option<Instant>,
    strikes: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub enum AccountHealth {
    Healthy,
    Cooldown { until: Instant, reason: String },
    AuthExpired,
}

#[derive(Debug, Clone)]
pub struct AccountState {
    pub id: AccountId,
    pub provider: ProviderKind,
    pub email: Option<String>,
    pub account_uuid: Option<Uuid>,
    pub config_dir: Option<PathBuf>,
    pub rate_limit_tier: Option<String>,
    pub health: AccountHealth,
    pub window_requests: u32,
    pub window_input_tokens: u64,
    pub window_output_tokens: u64,
    pub last_used_at: Option<Instant>,
    /// Excluded from `acquire` by the user until re-enabled.
    pub disabled: bool,
    /// Consecutive rate-limit hits; drives exponential cooldown.
    strikes: u32,
    pub models: Vec<ModelState>,
    /// Unix epoch ms when the account-wide cooldown lifts, when the CLI said.
    pub resets_at_ms: Option<u64>,
    /// The CLI binary disappeared.
    pub missing: bool,
    /// The account's model list came back empty or every model is unavailable; not picked before
    /// this instant (a refresh or a success clears it sooner).
    no_model_until: Option<Instant>,
}

impl AccountState {
    fn from_discovered(d: DiscoveredAccount) -> Self {
        Self {
            health: if d.auth_expired {
                AccountHealth::AuthExpired
            } else {
                AccountHealth::Healthy
            },
            id: d.id,
            provider: d.provider,
            email: d.email,
            account_uuid: d.account_uuid,
            config_dir: d.config_dir,
            rate_limit_tier: d.rate_limit_tier,
            window_requests: 0,
            window_input_tokens: 0,
            window_output_tokens: 0,
            last_used_at: None,
            disabled: false,
            strikes: 0,
            models: d.models.into_iter().map(ModelState::new).collect(),
            resets_at_ms: None,
            missing: false,
            no_model_until: None,
        }
    }

    /// The CLI said this account has no usable model and the bench has not lapsed by `now`.
    pub fn no_model_benched(&self, now: Instant) -> bool {
        self.no_model_until.is_some_and(|u| u > now)
    }

    /// Cooldowns that elapsed by `now` are cleared.
    fn expire(&mut self, now: Instant) {
        if matches!(&self.health, AccountHealth::Cooldown { until, .. } if *until <= now) {
            self.health = AccountHealth::Healthy;
            self.resets_at_ms = None;
            self.window_requests = 0;
            self.window_input_tokens = 0;
            self.window_output_tokens = 0;
        }
        if self.no_model_until.is_some_and(|u| u <= now) {
            self.no_model_until = None;
        }
        for m in &mut self.models {
            if m.until.is_some_and(|u| u <= now) {
                m.until = None;
                m.resets_at_ms = None;
            }
        }
    }

    fn model_usable(m: &ModelState) -> bool {
        m.until.is_none() && !m.unavailable && !m.disabled
    }

    /// Next model to run: the preferred one when usable, else free ones first, then cheapest.
    fn pick_model(&self, preferred: Option<&str>) -> Option<String> {
        if let Some(p) = preferred {
            if let Some(m) = self
                .models
                .iter()
                .find(|m| m.info.id == p && Self::model_usable(m))
            {
                return Some(m.info.id.clone());
            }
        }
        self.models
            .iter()
            .filter(|m| Self::model_usable(m))
            .min_by(|a, b| {
                (!a.info.free).cmp(&!b.info.free).then(
                    a.info
                        .credit_multiplier
                        .unwrap_or(0.0)
                        .total_cmp(&b.info.credit_multiplier.unwrap_or(0.0)),
                )
            })
            .map(|m| m.info.id.clone())
    }

    /// Multi-model accounts are only usable while some model is.
    fn has_usable_model(&self) -> bool {
        self.models.is_empty() || self.models.iter().any(Self::model_usable)
    }
}

impl ModelState {
    /// `info` from a fresh model list, carrying over the failure state of the same model in `old`.
    fn refreshed(old: &[ModelState], info: ModelInfo) -> Self {
        match old.iter().find(|m| m.info.id == info.id) {
            Some(m) => Self { info, ..m.clone() },
            None => Self::new(info),
        }
    }

    fn new(info: ModelInfo) -> Self {
        Self {
            info,
            until: None,
            resets_at_ms: None,
            unavailable: false,
            disabled: false,
        }
    }
}

fn ms_until(until: Instant, now: Instant, now_ms: u64) -> u64 {
    now_ms.saturating_add(
        u64::try_from(until.saturating_duration_since(now).as_millis()).unwrap_or(0),
    )
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    /// First usable account in discovery order.
    Priority,
    /// Least recently used account of the provider.
    RoundRobin,
}

/// Picks a usable account per request and cools down ones that hit a limit.
#[derive(Debug)]
pub struct AccountPool {
    strategy: Strategy,
    accounts: Mutex<Vec<AccountState>>,
    offline: Mutex<Offline>,
}

impl AccountPool {
    pub fn new(discovered: Vec<DiscoveredAccount>, strategy: Strategy) -> Self {
        let accounts = discovered
            .into_iter()
            .map(AccountState::from_discovered)
            .collect();
        Self {
            strategy,
            accounts: Mutex::new(accounts),
            offline: Mutex::new(Offline::default()),
        }
    }

    /// Pick an account for the first provider in `chain` that has a usable one.
    /// Cooldowns that have elapsed by `now` are cleared and start a fresh window.
    pub fn acquire(&self, chain: &[ProviderKind], now: Instant) -> Option<AccountId> {
        self.acquire_model(chain, None, now).map(|p| p.account)
    }

    /// Like `acquire`, and also names the model for accounts that expose several (kiro, opencode):
    /// `preferred` when it is usable, else the next free (then cheapest) model, so a limited or
    /// unavailable model fails over to its siblings before the account is given up on.
    pub fn acquire_model(
        &self,
        chain: &[ProviderKind],
        preferred: Option<&str>,
        now: Instant,
    ) -> Option<Pick> {
        let mut accounts = self.accounts.lock();
        for a in accounts.iter_mut() {
            a.expire(now);
        }
        for provider in chain {
            let mut usable = accounts.iter_mut().filter(|a| {
                a.provider == *provider
                    && !a.disabled
                    && !a.missing
                    && a.health == AccountHealth::Healthy
                    && !a.no_model_benched(now)
                    && a.has_usable_model()
            });
            let picked = match self.strategy {
                Strategy::Priority => usable.next(),
                Strategy::RoundRobin => usable.min_by_key(|a| a.last_used_at),
            };
            if let Some(a) = picked {
                a.last_used_at = Some(now);
                a.window_requests += 1;
                let model = a.pick_model(preferred);
                return Some(Pick {
                    account: a.id.clone(),
                    model,
                });
            }
        }
        None
    }

    /// Cool `id` down after a rate-limit failure. The reset time comes from the
    /// CLI's message (`now_epoch_secs` anchors absolute timestamps), else an
    /// exponential backoff. Returns the cooldown applied.
    pub fn report_rate_limit(
        &self,
        id: &AccountId,
        message: &str,
        now: Instant,
        now_epoch_secs: u64,
    ) -> Duration {
        let mut accounts = self.accounts.lock();
        let Some(a) = accounts.iter_mut().find(|a| a.id == *id) else {
            return Duration::ZERO;
        };
        let cooldown = parse_cooldown(message, now_epoch_secs).unwrap_or_else(|| {
            BASE_COOLDOWN
                .saturating_mul(1 << a.strikes.min(16))
                .min(MAX_COOLDOWN)
        });
        a.strikes += 1;
        a.health = AccountHealth::Cooldown {
            until: now.checked_add(cooldown).unwrap_or(now),
            reason: message.trim().chars().take(200).collect(),
        };
        cooldown
    }

    pub fn report_auth_expired(&self, id: &AccountId) {
        // A runtime auth failure may be a false positive, so retry after MAX_COOLDOWN
        // instead of dropping the account until restart.
        self.set_health(
            id,
            AccountHealth::Cooldown {
                until: Instant::now() + MAX_COOLDOWN,
                reason: AUTH_EXPIRED_REASON.into(),
            },
        );
    }

    /// Record a classified failure of `model` on `id` (see `FailureKind`):
    /// - `RateLimited`: cool down until the reset time the CLI gave (default backoff if none). A quota
    ///   message (free tier, credits, subscription) or a call without a model cools the whole account;
    ///   otherwise only `model`, so siblings keep serving.
    /// - `Offline`: global backoff only; no account is touched.
    /// - `NotLoggedIn`: `NeedsLogin`. `NoModel`: that model is benched. `NotInstalled`: `Missing`.
    ///
    /// Returns the wait applied (zero when nothing was cooled).
    pub fn report_failure(
        &self,
        id: &AccountId,
        model: Option<&str>,
        kind: &FailureKind,
        message: &str,
        now: Instant,
        now_ms: u64,
    ) -> Duration {
        match kind {
            FailureKind::Offline => {
                let mut o = self.offline.lock();
                let wait = OFFLINE_BASE
                    .saturating_mul(1 << o.strikes.min(16))
                    .min(OFFLINE_MAX);
                o.strikes += 1;
                o.until = now.checked_add(wait);
                return wait;
            }
            FailureKind::Other(_) | FailureKind::TimedOut => return Duration::ZERO,
            _ => {}
        }
        let mut accounts = self.accounts.lock();
        let Some(a) = accounts.iter_mut().find(|a| a.id == *id) else {
            return Duration::ZERO;
        };
        match kind {
            FailureKind::NotLoggedIn => {
                a.health = AccountHealth::Cooldown {
                    until: now.checked_add(MAX_COOLDOWN).unwrap_or(now),
                    reason: AUTH_EXPIRED_REASON.into(),
                };
                MAX_COOLDOWN
            }
            FailureKind::NotInstalled => {
                a.missing = true;
                Duration::ZERO
            }
            FailureKind::NoModel => {
                if let Some(m) = model.and_then(|m| a.models.iter_mut().find(|s| s.info.id == m)) {
                    m.unavailable = true;
                    m.until = None;
                }
                // Nothing left to run on this account: bench the account itself, so the next
                // acquire fails over instead of spawning the same broken CLI again.
                let gone = a.models.is_empty() || a.models.iter().all(|m| m.unavailable);
                a.no_model_until = gone.then(|| now.checked_add(NO_MODEL_BENCH)).flatten();
                NO_MODEL_BENCH
            }
            FailureKind::RateLimited { resets_at } => {
                let wait = resets_at
                    .map(|r| Duration::from_millis(r.saturating_sub(now_ms)))
                    .map(|d| d.min(MAX_PARSED_COOLDOWN))
                    .unwrap_or_else(|| {
                        BASE_COOLDOWN
                            .saturating_mul(1 << a.strikes.min(16))
                            .min(MAX_COOLDOWN)
                    });
                a.strikes += 1;
                let until = now.checked_add(wait).unwrap_or(now);
                let lower = message.to_ascii_lowercase();
                let account_wide = ["free usage", "credit", "quota", "subscription", "monthly"]
                    .iter()
                    .any(|w| lower.contains(w));
                match model.and_then(|m| a.models.iter_mut().find(|s| s.info.id == m)) {
                    Some(m) if !account_wide => {
                        m.until = Some(until);
                        m.resets_at_ms = resets_at.or(Some(ms_until(until, now, now_ms)));
                    }
                    _ => {
                        a.health = AccountHealth::Cooldown {
                            until,
                            reason: message.trim().chars().take(200).collect(),
                        };
                        a.resets_at_ms = resets_at.or(Some(ms_until(until, now, now_ms)));
                    }
                }
                wait
            }
            _ => Duration::ZERO,
        }
    }

    /// How long callers should wait before the next attempt while the network is down.
    pub fn offline_for(&self, now: Instant) -> Option<Duration> {
        let o = self.offline.lock();
        o.until
            .filter(|u| *u > now)
            .map(|u| u.saturating_duration_since(now))
    }

    pub fn report_success(&self, id: &AccountId, input_tokens: u64, output_tokens: u64) {
        *self.offline.lock() = Offline::default();
        let mut accounts = self.accounts.lock();
        if let Some(a) = accounts.iter_mut().find(|a| a.id == *id) {
            a.strikes = 0;
            a.no_model_until = None;
            a.window_input_tokens += input_tokens;
            a.window_output_tokens += output_tokens;
        }
    }

    /// Exclude (or re-include) an account from `acquire`. False when `id` is unknown.
    pub fn set_disabled(&self, id: &AccountId, disabled: bool) -> bool {
        match self.accounts.lock().iter_mut().find(|a| a.id == *id) {
            Some(a) => {
                a.disabled = disabled;
                true
            }
            None => false,
        }
    }

    /// Fold a fresh discovery into the pool: unknown accounts are appended (their ids are
    /// returned); known ones keep their health, but a login that expired or was renewed on disk
    /// is picked up when the pool has no runtime cooldown on it.
    pub fn merge_discovered(&self, discovered: Vec<DiscoveredAccount>) -> Vec<AccountId> {
        let mut accounts = self.accounts.lock();
        let mut added = Vec::new();
        for d in discovered {
            match accounts.iter_mut().find(|a| a.id == d.id) {
                Some(a) => {
                    a.email = d.email;
                    a.rate_limit_tier = d.rate_limit_tier;
                    a.config_dir = d.config_dir;
                    a.missing = false;
                    // A refreshed model list keeps the failure state of models that are still there.
                    let old = std::mem::take(&mut a.models);
                    a.models = d
                        .models
                        .into_iter()
                        .map(|info| ModelState::refreshed(&old, info))
                        .collect();
                    a.no_model_until = None;
                    match (&a.health, d.auth_expired) {
                        (AccountHealth::Healthy, true) => a.health = AccountHealth::AuthExpired,
                        (AccountHealth::AuthExpired, false) => a.health = AccountHealth::Healthy,
                        _ => {}
                    }
                }
                None => {
                    added.push(d.id.clone());
                    accounts.push(AccountState::from_discovered(d));
                }
            }
        }
        added
    }

    pub fn snapshot(&self) -> Vec<AccountState> {
        self.accounts.lock().clone()
    }

    /// Status of every account and model at `now` (`now_ms` = the same instant as unix ms).
    /// While offline every account reads `Offline`: the network is down, not the account.
    pub fn statuses(&self, now: Instant, now_ms: u64) -> Vec<AccountStatusRow> {
        let offline = self.offline_for(now).is_some();
        self.accounts
            .lock()
            .iter()
            .map(|a| {
                let model_rows: Vec<ModelStatusRow> = a
                    .models
                    .iter()
                    .map(|m| ModelStatusRow {
                        id: m.info.id.clone(),
                        disabled: m.disabled,
                        free: m.info.free,
                        credit_multiplier: m.info.credit_multiplier,
                        status: if m.unavailable || m.disabled {
                            AccountStatus::NoModel
                        } else if let Some(u) = m.until.filter(|u| *u > now) {
                            AccountStatus::CoolingDown {
                                until: m.resets_at_ms.unwrap_or_else(|| ms_until(u, now, now_ms)),
                            }
                        } else {
                            AccountStatus::Ready
                        },
                        resets_at: m.resets_at_ms.filter(|_| m.until.is_some_and(|u| u > now)),
                    })
                    .collect();
                let mut status = match &a.health {
                    _ if a.missing => AccountStatus::Missing,
                    AccountHealth::AuthExpired => AccountStatus::NeedsLogin,
                    AccountHealth::Cooldown { reason, .. } if reason == AUTH_EXPIRED_REASON => {
                        AccountStatus::NeedsLogin
                    }
                    AccountHealth::Cooldown { until, .. } if *until > now => {
                        AccountStatus::CoolingDown {
                            until: a
                                .resets_at_ms
                                .unwrap_or_else(|| ms_until(*until, now, now_ms)),
                        }
                    }
                    _ if a.no_model_benched(now) => AccountStatus::NoModel,
                    _ if !a.models.is_empty()
                        && model_rows
                            .iter()
                            .all(|m| m.status == AccountStatus::NoModel) =>
                    {
                        AccountStatus::NoModel
                    }
                    _ if !a.models.is_empty()
                        && model_rows.iter().all(|m| m.status != AccountStatus::Ready) =>
                    {
                        // Every model is limited or gone: wait for the earliest reset.
                        model_rows
                            .iter()
                            .filter_map(|m| match m.status {
                                AccountStatus::CoolingDown { until } => Some(until),
                                _ => None,
                            })
                            .min()
                            .map_or(AccountStatus::NoModel, |until| AccountStatus::CoolingDown {
                                until,
                            })
                    }
                    _ => AccountStatus::Ready,
                };
                if offline
                    && matches!(
                        status,
                        AccountStatus::Ready | AccountStatus::CoolingDown { .. }
                    )
                {
                    status = AccountStatus::Offline;
                }
                AccountStatusRow {
                    id: a.id.clone(),
                    provider: a.provider,
                    resets_at: match status {
                        AccountStatus::CoolingDown { until } => Some(until),
                        _ => None,
                    },
                    status,
                    models: model_rows,
                }
            })
            .collect()
    }

    /// One status per provider; a provider with no account at all is `Missing`.
    /// The best account wins (`Ready` over cooling over logged out).
    pub fn provider_statuses(
        &self,
        now: Instant,
        now_ms: u64,
    ) -> Vec<(ProviderKind, AccountStatus)> {
        let rows = self.statuses(now, now_ms);
        let rank = |s: &AccountStatus| match s {
            AccountStatus::Ready => 0,
            AccountStatus::Offline => 1,
            AccountStatus::CoolingDown { .. } => 2,
            AccountStatus::NoModel => 3,
            AccountStatus::NeedsLogin => 4,
            AccountStatus::Missing => 5,
        };
        ProviderKind::ALL
            .iter()
            .map(|p| {
                let best = rows
                    .iter()
                    .filter(|r| r.provider == *p)
                    .map(|r| r.status.clone())
                    .min_by_key(rank)
                    .unwrap_or(AccountStatus::Missing);
                (*p, best)
            })
            .collect()
    }

    /// Switch one model of `id` off (or back on). False when the account or model is unknown.
    pub fn set_model_disabled(&self, id: &AccountId, model: &str, disabled: bool) -> bool {
        let mut accounts = self.accounts.lock();
        let Some(m) = accounts
            .iter_mut()
            .find(|a| a.id == *id)
            .and_then(|a| a.models.iter_mut().find(|m| m.info.id == model))
        else {
            return false;
        };
        m.disabled = disabled;
        true
    }

    /// Replace an account's model list (e.g. after the TTL cache refreshed), keeping failure state.
    pub fn set_models(&self, id: &AccountId, models: Vec<ModelInfo>) {
        if let Some(a) = self.accounts.lock().iter_mut().find(|a| a.id == *id) {
            let old = std::mem::take(&mut a.models);
            a.models = models
                .into_iter()
                .map(|info| ModelState::refreshed(&old, info))
                .collect();
            a.no_model_until = None;
        }
    }

    fn set_health(&self, id: &AccountId, health: AccountHealth) {
        if let Some(a) = self.accounts.lock().iter_mut().find(|a| a.id == *id) {
            a.health = health;
        }
    }
}

/// Extract a wait from a CLI rate-limit message: `...limit reached|<unix secs>`, `retry after 30`,
/// `try again in 5 minutes`, `Resets in 50h7m56s`, `retryAfterMs`, or a calendar date.
pub fn parse_cooldown(message: &str, now_epoch_secs: u64) -> Option<Duration> {
    engine::parse_wait(message, now_epoch_secs)
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::Path;

    use super::super::{discover, DiscoverRoots};
    use super::*;

    const NOW_MS: u64 = 1_800_000_000_000;

    fn write(path: &Path, body: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, body).unwrap();
    }

    fn claude_json(email: &str, uuid: &str) -> String {
        format!(
            r#"{{"oauthAccount":{{"accountUuid":"{uuid}","emailAddress":"{email}","userRateLimitTier":"default_claude_max_5x"}}}}"#
        )
    }

    /// 3 Claude accounts (healthy, rate-limited in the pool, expired) + Codex + Agy.
    fn fixture() -> tempfile::TempDir {
        let home = tempfile::tempdir().unwrap();
        let h = home.path();
        write(
            &h.join(".claude.json"),
            &claude_json("a@x.com", "11111111-1111-1111-1111-111111111111"),
        );
        write(&h.join(".claude/settings.json"), "{}");
        write(
            &h.join(".claude-accounts/account2/.claude.json"),
            &claude_json("b@x.com", "22222222-2222-2222-2222-222222222222"),
        );
        write(
            &h.join(".claude-work/.claude.json"),
            &claude_json("c@x.com", "33333333-3333-3333-3333-333333333333"),
        );
        write(
            &h.join(".claude-work/.credentials.json"),
            &format!(r#"{{"claudeAiOauth":{{"expiresAt":{}}}}}"#, NOW_MS - 1),
        );
        // {"email":"d@x.com","https://api.openai.com/auth":{"chatgpt_plan_type":"plus"}}
        let jwt = "e30.eyJlbWFpbCI6ImRAeC5jb20iLCJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9wbGFuX3R5cGUiOiJwbHVzIn19.sig";
        write(
            &h.join(".codex/auth.json"),
            &format!(r#"{{"auth_mode":"chatgpt","tokens":{{"id_token":"{jwt}"}}}}"#),
        );
        write(
            &h.join(".gemini/oauth_creds.json"),
            &format!(r#"{{"refresh_token":"r","expiry_date":{}}}"#, NOW_MS - 1),
        );
        write(
            &h.join(".gemini/google_accounts.json"),
            r#"{"active":"e@x.com"}"#,
        );
        home
    }

    fn roots(home: &Path) -> DiscoverRoots {
        DiscoverRoots {
            home: home.to_owned(),
            now_ms: NOW_MS,
            ..Default::default()
        }
    }

    #[test]
    fn discovers_all_accounts_with_identity() {
        let home = fixture();
        let found = discover(&roots(home.path()));
        let ids: Vec<_> = found.iter().map(|a| a.id.0.as_str()).collect();
        assert_eq!(
            ids,
            [
                "claude:a@x.com",
                "claude:b@x.com",
                "claude:c@x.com",
                "codex:d@x.com",
                "agy:default"
            ]
        );
        assert_eq!(
            found[0].account_uuid.unwrap().to_string(),
            "11111111-1111-1111-1111-111111111111"
        );
        assert_eq!(
            found[0].rate_limit_tier.as_deref(),
            Some("default_claude_max_5x")
        );
        assert!(found[2].auth_expired);
        assert_eq!(found[3].rate_limit_tier.as_deref(), Some("plus"));
        // Agy has a refresh token, so an old access token is not "expired".
        assert_eq!(found[4].email.as_deref(), Some("e@x.com"));
        assert!(!found[4].auth_expired);
    }

    #[test]
    fn dedupes_claude_config_dir_pointing_at_known_profile() {
        let home = fixture();
        let mut r = roots(home.path());
        r.claude_config_dir = Some(home.path().join(".claude-accounts/account2"));
        let claude = discover(&r)
            .into_iter()
            .filter(|a| a.provider == ProviderKind::ClaudeCli)
            .count();
        assert_eq!(claude, 3);
    }

    #[test]
    fn fails_over_across_accounts_then_providers_and_recovers() {
        let home = fixture();
        let pool = AccountPool::new(discover(&roots(home.path())), Strategy::Priority);
        let chain = [
            ProviderKind::ClaudeCli,
            ProviderKind::CodexCli,
            ProviderKind::AgyCli,
        ];
        let t0 = Instant::now();

        let a = pool.acquire(&chain, t0).unwrap();
        assert_eq!(a.0, "claude:a@x.com");
        let cd = pool.report_rate_limit(&a, "usage limit reached, try again in 2 hours", t0, 0);
        assert_eq!(cd, Duration::from_secs(7200));

        // Account 2 takes over; the expired third account is never picked.
        let b = pool.acquire(&chain, t0).unwrap();
        assert_eq!(b.0, "claude:b@x.com");
        pool.report_rate_limit(&b, "429", t0, 0);
        assert_eq!(pool.acquire(&chain, t0).unwrap().0, "codex:d@x.com");

        // b's 60s backoff elapses before a's 2h window.
        let later = t0 + Duration::from_secs(61);
        assert_eq!(pool.acquire(&chain, later).unwrap().0, "claude:b@x.com");
        let much_later = t0 + Duration::from_secs(7201);
        assert_eq!(
            pool.acquire(&chain, much_later).unwrap().0,
            "claude:a@x.com"
        );
    }

    #[test]
    fn exhausted_pool_returns_none_and_backoff_doubles() {
        let home = fixture();
        let pool = AccountPool::new(discover(&roots(home.path())), Strategy::Priority);
        let t0 = Instant::now();
        let a = pool.acquire(&[ProviderKind::AgyCli], t0).unwrap();
        assert_eq!(
            pool.report_rate_limit(&a, "429", t0, 0),
            Duration::from_secs(60)
        );
        assert!(pool.acquire(&[ProviderKind::AgyCli], t0).is_none());
        let t1 = t0 + Duration::from_secs(60);
        let a = pool.acquire(&[ProviderKind::AgyCli], t1).unwrap();
        assert_eq!(
            pool.report_rate_limit(&a, "429", t1, 0),
            Duration::from_secs(120)
        );
        pool.report_success(&a, 10, 5);
        let s = pool.snapshot().into_iter().find(|s| s.id == a).unwrap();
        assert_eq!((s.window_input_tokens, s.window_output_tokens), (10, 5));
    }

    #[test]
    fn round_robin_rotates_least_recently_used() {
        let home = fixture();
        let pool = AccountPool::new(discover(&roots(home.path())), Strategy::RoundRobin);
        let t0 = Instant::now();
        let picks: Vec<_> = (0..3)
            .map(|i| {
                pool.acquire(&[ProviderKind::ClaudeCli], t0 + Duration::from_secs(i))
                    .unwrap()
                    .0
            })
            .collect();
        assert_eq!(
            picks,
            ["claude:a@x.com", "claude:b@x.com", "claude:a@x.com"]
        );
    }

    #[test]
    fn expired_auth_is_skipped() {
        let home = fixture();
        let pool = AccountPool::new(discover(&roots(home.path())), Strategy::Priority);
        pool.report_auth_expired(&AccountId("claude:a@x.com".into()));
        let t0 = Instant::now();
        assert_eq!(
            pool.acquire(&[ProviderKind::ClaudeCli], t0).unwrap().0,
            "claude:b@x.com"
        );
    }

    #[test]
    fn parses_cooldown_formats() {
        assert_eq!(
            parse_cooldown("Claude AI usage limit reached|1800000500", 1_800_000_000),
            Some(Duration::from_secs(500))
        );
        assert_eq!(
            parse_cooldown("retry-after: 30", 0),
            Some(Duration::from_secs(30))
        );
        assert_eq!(
            parse_cooldown("try again in 5 minutes", 0),
            Some(Duration::from_secs(300))
        );
        assert_eq!(
            parse_cooldown("resets in 45s", 0),
            Some(Duration::from_secs(45))
        );
        assert_eq!(parse_cooldown("HTTP 429", 0), None);
    }

    #[test]
    fn huge_cooldowns_are_capped_not_panicking() {
        assert_eq!(
            parse_cooldown("limit reached|99999999999999999", 0),
            Some(MAX_PARSED_COOLDOWN)
        );
        assert_eq!(
            parse_cooldown("try again in 99999999999999999999 hours", 0),
            None
        );
        assert_eq!(
            parse_cooldown("try again in 18446744073709551615 hours", 0),
            Some(MAX_PARSED_COOLDOWN)
        );
        let home = fixture();
        let pool = AccountPool::new(discover(&roots(home.path())), Strategy::Priority);
        let a = AccountId("claude:a@x.com".into());
        let cd = pool.report_rate_limit(&a, "limit reached|99999999999999999", Instant::now(), 0);
        assert_eq!(cd, MAX_PARSED_COOLDOWN);
    }

    #[test]
    fn runtime_auth_expiry_recovers_after_cooldown() {
        let home = fixture();
        let pool = AccountPool::new(discover(&roots(home.path())), Strategy::Priority);
        pool.report_auth_expired(&AccountId("claude:a@x.com".into()));
        let later = Instant::now() + MAX_COOLDOWN + Duration::from_secs(1);
        assert_eq!(
            pool.acquire(&[ProviderKind::ClaudeCli], later).unwrap().0,
            "claude:a@x.com"
        );
    }

    // ---- multi-model failover, taxonomy-driven status ---------------------------------------

    use super::super::ModelInfo;

    fn model(id: &str, free: bool, mult: Option<f64>) -> ModelInfo {
        ModelInfo {
            id: id.into(),
            free,
            credit_multiplier: mult,
        }
    }

    fn acct(id: &str, provider: ProviderKind, models: Vec<ModelInfo>) -> DiscoveredAccount {
        DiscoveredAccount {
            id: AccountId(id.into()),
            provider,
            email: None,
            account_uuid: None,
            config_dir: None,
            rate_limit_tier: None,
            auth_expired: false,
            models,
        }
    }

    fn opencode_pool() -> AccountPool {
        AccountPool::new(
            vec![acct(
                "opencode:opencode",
                ProviderKind::OpencodeCli,
                vec![
                    model("opencode/paid", false, None),
                    model("opencode/a-free", true, None),
                    model("opencode/b-free", true, None),
                    model("opencode/c-free", true, None),
                ],
            )],
            Strategy::Priority,
        )
    }

    const CHAIN: [ProviderKind; 1] = [ProviderKind::OpencodeCli];

    #[test]
    fn a_limited_model_fails_over_to_the_next_free_model_then_the_account_cools() {
        let pool = opencode_pool();
        let (t0, id) = (Instant::now(), AccountId("opencode:opencode".into()));
        let pick = |t| {
            pool.acquire_model(&CHAIN, None, t)
                .map(|p| p.model.unwrap())
        };
        // Free models first, in list order.
        assert_eq!(pick(t0).as_deref(), Some("opencode/a-free"));
        let limited = FailureKind::RateLimited {
            resets_at: Some(NOW_MS + 600_000),
        };
        pool.report_failure(
            &id,
            Some("opencode/a-free"),
            &limited,
            "429 slow down",
            t0,
            NOW_MS,
        );
        assert_eq!(pick(t0).as_deref(), Some("opencode/b-free"));
        // Unavailable model (bad id) is benched too.
        pool.report_failure(
            &id,
            Some("opencode/b-free"),
            &FailureKind::NoModel,
            "gone",
            t0,
            NOW_MS,
        );
        assert_eq!(pick(t0).as_deref(), Some("opencode/c-free"));
        // The preferred model wins when usable.
        let p = pool
            .acquire_model(&CHAIN, Some("opencode/paid"), t0)
            .unwrap();
        assert_eq!(p.model.as_deref(), Some("opencode/paid"));
        pool.report_failure(&id, Some("opencode/c-free"), &limited, "429", t0, NOW_MS);
        pool.report_failure(&id, Some("opencode/paid"), &limited, "429", t0, NOW_MS);
        assert!(pool.acquire_model(&CHAIN, None, t0).is_none());
        let rows = pool.statuses(t0, NOW_MS);
        assert_eq!(
            rows[0].status,
            AccountStatus::CoolingDown {
                until: NOW_MS + 600_000
            }
        );
        let m = &rows[0].models;
        assert_eq!(
            m[1].status,
            AccountStatus::CoolingDown {
                until: NOW_MS + 600_000
            }
        );
        assert_eq!(m[2].status, AccountStatus::NoModel);
        // After the reset the model serves again; the benched one stays out.
        let later = t0 + Duration::from_secs(601);
        assert_eq!(pick(later).as_deref(), Some("opencode/a-free"));
    }

    #[test]
    fn quota_text_cools_the_whole_account_until_the_reported_reset() {
        let pool = opencode_pool();
        let (t0, id) = (Instant::now(), AccountId("opencode:opencode".into()));
        let resets = NOW_MS + 19_898_000;
        let k = FailureKind::RateLimited {
            resets_at: Some(resets),
        };
        let wait = pool.report_failure(
            &id,
            Some("opencode/a-free"),
            &k,
            "Free usage limit reached",
            t0,
            NOW_MS,
        );
        assert_eq!(wait, Duration::from_millis(19_898_000));
        assert!(pool.acquire_model(&CHAIN, None, t0).is_none());
        let row = &pool.statuses(t0, NOW_MS)[0];
        assert_eq!(row.status, AccountStatus::CoolingDown { until: resets });
        assert_eq!(row.resets_at, Some(resets));
        let after = t0 + Duration::from_millis(19_898_001);
        assert!(pool.acquire_model(&CHAIN, None, after).is_some());
    }

    #[test]
    fn reset_time_beats_backoff_and_unknown_uses_exponential_backoff() {
        let pool = opencode_pool();
        let (t0, id) = (Instant::now(), AccountId("opencode:opencode".into()));
        let none = FailureKind::RateLimited { resets_at: None };
        assert_eq!(
            pool.report_failure(&id, None, &none, "429", t0, NOW_MS),
            Duration::from_secs(60)
        );
        let t1 = t0 + Duration::from_secs(61);
        assert_eq!(
            pool.report_failure(&id, None, &none, "429", t1, NOW_MS),
            Duration::from_secs(120)
        );
        // 50h reset parsed from "Resets in 50h7m56s" is honoured, not capped to an hour.
        let k = crate::engine::classify("Individual quota reached. Resets in 50h7m56s.", NOW_MS);
        let w = pool.report_failure(&id, None, &k, "quota", t1, NOW_MS);
        assert_eq!(w, Duration::from_secs(50 * 3600 + 7 * 60 + 56));
    }

    #[test]
    fn offline_never_cools_an_account_and_backs_off_globally() {
        let home = fixture();
        let pool = AccountPool::new(discover(&roots(home.path())), Strategy::Priority);
        let (t0, id) = (Instant::now(), AccountId("claude:a@x.com".into()));
        assert!(pool.offline_for(t0).is_none());
        let w1 = pool.report_failure(&id, None, &FailureKind::Offline, "ECONNREFUSED", t0, NOW_MS);
        let w2 = pool.report_failure(&id, None, &FailureKind::Offline, "ECONNREFUSED", t0, NOW_MS);
        assert_eq!((w1, w2), (Duration::from_secs(5), Duration::from_secs(10)));
        assert_eq!(pool.offline_for(t0), Some(Duration::from_secs(10)));
        // No account is limited: the same one is still handed out, and every row reads Offline.
        assert_eq!(pool.acquire(&[ProviderKind::ClaudeCli], t0).unwrap(), id);
        let rows = pool.statuses(t0, NOW_MS);
        assert!(rows
            .iter()
            .filter(|r| r.status != AccountStatus::NeedsLogin)
            .all(|r| r.status == AccountStatus::Offline));
        assert!(pool
            .snapshot()
            .iter()
            .all(|a| !matches!(a.health, AccountHealth::Cooldown { .. })));
        // The pause lapses by itself; a success clears the strikes.
        assert!(pool.offline_for(t0 + Duration::from_secs(11)).is_none());
        pool.report_success(&id, 1, 1);
        assert!(pool.offline_for(t0).is_none());
        assert_eq!(
            pool.report_failure(&id, None, &FailureKind::Offline, "x", t0, NOW_MS),
            Duration::from_secs(5)
        );
    }

    #[test]
    fn login_missing_and_no_model_statuses() {
        let home = fixture();
        let pool = AccountPool::new(discover(&roots(home.path())), Strategy::Priority);
        let t0 = Instant::now();
        let a = AccountId("claude:a@x.com".into());
        pool.report_failure(
            &a,
            None,
            &FailureKind::NotLoggedIn,
            "Not logged in",
            t0,
            NOW_MS,
        );
        let status = |id: &str| {
            pool.statuses(t0, NOW_MS)
                .into_iter()
                .find(|r| r.id.0 == id)
                .unwrap()
                .status
        };
        assert_eq!(status("claude:a@x.com"), AccountStatus::NeedsLogin);
        assert_eq!(status("claude:c@x.com"), AccountStatus::NeedsLogin);
        assert_eq!(status("codex:d@x.com"), AccountStatus::Ready);
        pool.report_failure(
            &AccountId("codex:d@x.com".into()),
            None,
            &FailureKind::NotInstalled,
            "",
            t0,
            NOW_MS,
        );
        assert_eq!(status("codex:d@x.com"), AccountStatus::Missing);
        assert!(pool.acquire(&[ProviderKind::CodexCli], t0).is_none());
        pool.report_failure(
            &AccountId("agy:default".into()),
            None,
            &FailureKind::NoModel,
            "no",
            t0,
            NOW_MS,
        );
        assert_eq!(status("agy:default"), AccountStatus::NoModel);
        // Providers without any account are Missing.
        let p = pool.provider_statuses(t0, NOW_MS);
        let of = |k| p.iter().find(|(q, _)| *q == k).unwrap().1.clone();
        assert_eq!(of(ProviderKind::KiroCli), AccountStatus::Missing);
        assert_eq!(of(ProviderKind::ClaudeCli), AccountStatus::Ready);
    }

    #[test]
    fn kiro_picks_the_cheapest_model_and_refresh_keeps_failure_state() {
        let pool = AccountPool::new(
            vec![acct(
                "kiro:k@x.com",
                ProviderKind::KiroCli,
                vec![
                    model("auto", false, Some(1.0)),
                    model("qwen3-coder-next", false, Some(0.05)),
                    model("glm-5", false, Some(0.5)),
                ],
            )],
            Strategy::Priority,
        );
        let (t0, id) = (Instant::now(), AccountId("kiro:k@x.com".into()));
        let chain = [ProviderKind::KiroCli];
        let m = |t| pool.acquire_model(&chain, None, t).unwrap().model.unwrap();
        assert_eq!(m(t0), "qwen3-coder-next");
        pool.report_failure(
            &id,
            Some("qwen3-coder-next"),
            &FailureKind::NoModel,
            "",
            t0,
            NOW_MS,
        );
        assert_eq!(m(t0), "glm-5");
        pool.set_models(
            &id,
            vec![
                model("glm-5", false, Some(0.5)),
                model("qwen3-coder-next", false, Some(0.05)),
            ],
        );
        assert_eq!(m(t0), "glm-5");
        // Credits exhausted for the month: whole account, reset date from the text.
        let k = crate::engine::classify(
            "Monthly credits exhausted. Resets on 2027-02-01",
            1_800_000_000_000,
        );
        pool.report_failure(
            &id,
            Some("glm-5"),
            &k,
            "Monthly credits exhausted",
            t0,
            1_800_000_000_000,
        );
        assert!(pool.acquire_model(&chain, None, t0).is_none());
        let FailureKind::RateLimited { resets_at: Some(r) } = k else {
            panic!()
        };
        assert_eq!(pool.statuses(t0, 1_800_000_000_000)[0].resets_at, Some(r));
    }

    #[test]
    fn a_no_model_account_is_not_picked_again_and_recovers() {
        let home = fixture();
        let pool = AccountPool::new(discover(&roots(home.path())), Strategy::Priority);
        let (t0, a, b) = (
            Instant::now(),
            AccountId("claude:a@x.com".into()),
            AccountId("claude:b@x.com".into()),
        );
        assert_eq!(pool.acquire(&[ProviderKind::ClaudeCli], t0).unwrap(), a);
        // Claude exposes no model list: "no model" benches the account, not one model of it.
        pool.report_failure(
            &a,
            None,
            &FailureKind::NoModel,
            "no models available",
            t0,
            NOW_MS,
        );
        assert_eq!(pool.acquire(&[ProviderKind::ClaudeCli], t0).unwrap(), b);
        pool.report_failure(
            &b,
            None,
            &FailureKind::NoModel,
            "no models available",
            t0,
            NOW_MS,
        );
        // claude:c@x.com is signed out, so the provider is exhausted rather than handed out again.
        assert!(pool.acquire(&[ProviderKind::ClaudeCli], t0).is_none());
        assert_eq!(pool.statuses(t0, NOW_MS)[0].status, AccountStatus::NoModel);
        // A success clears the bench.
        pool.report_success(&a, 0, 0);
        assert_eq!(pool.acquire(&[ProviderKind::ClaudeCli], t0).unwrap(), a);
        // So does its expiry.
        pool.report_failure(&a, None, &FailureKind::NoModel, "", t0, NOW_MS);
        assert!(pool.acquire(&[ProviderKind::ClaudeCli], t0).is_none());
        let later = t0 + NO_MODEL_BENCH + Duration::from_secs(1);
        assert_eq!(pool.acquire(&[ProviderKind::ClaudeCli], later).unwrap(), a);
    }

    #[test]
    fn a_disabled_model_is_never_picked_and_reads_no_model() {
        let acct = DiscoveredAccount {
            id: AccountId("opencode:opencode".into()),
            provider: ProviderKind::OpencodeCli,
            email: None,
            account_uuid: None,
            config_dir: None,
            rate_limit_tier: None,
            auth_expired: false,
            models: ["opencode/a-free", "opencode/b-free"]
                .map(|id| ModelInfo {
                    id: id.into(),
                    free: true,
                    credit_multiplier: None,
                })
                .to_vec(),
        };
        let pool = AccountPool::new(vec![acct], Strategy::Priority);
        let id = AccountId("opencode:opencode".into());
        let chain = [ProviderKind::OpencodeCli];
        assert!(pool.set_model_disabled(&id, "opencode/a-free", true));
        assert!(!pool.set_model_disabled(&id, "nope", true));
        let pick = pool.acquire_model(&chain, Some("opencode/a-free"), Instant::now());
        assert_eq!(pick.unwrap().model.as_deref(), Some("opencode/b-free"));
        let rows = pool.statuses(Instant::now(), NOW_MS);
        assert!(rows[0].models[0].disabled);
        assert_eq!(rows[0].models[0].status, AccountStatus::NoModel);
        assert_eq!(rows[0].status, AccountStatus::Ready);
        pool.set_model_disabled(&id, "opencode/b-free", true);
        assert!(pool.acquire_model(&chain, None, Instant::now()).is_none());
        pool.set_model_disabled(&id, "opencode/a-free", false);
        assert!(pool.acquire_model(&chain, None, Instant::now()).is_some());
    }
}
