use std::path::PathBuf;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use uuid::Uuid;

use super::{AccountId, DiscoveredAccount, ProviderKind};

const BASE_COOLDOWN: Duration = Duration::from_secs(60);
const MAX_COOLDOWN: Duration = Duration::from_secs(3600);
/// Upper bound for a cooldown parsed from CLI output (a hostile message can claim anything).
const MAX_PARSED_COOLDOWN: Duration = Duration::from_secs(24 * 3600);

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
    /// Consecutive rate-limit hits; drives exponential cooldown.
    strikes: u32,
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
}

impl AccountPool {
    pub fn new(discovered: Vec<DiscoveredAccount>, strategy: Strategy) -> Self {
        let accounts = discovered
            .into_iter()
            .map(|d| AccountState {
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
                strikes: 0,
            })
            .collect();
        Self {
            strategy,
            accounts: Mutex::new(accounts),
        }
    }

    /// Pick an account for the first provider in `chain` that has a usable one.
    /// Cooldowns that have elapsed by `now` are cleared and start a fresh window.
    pub fn acquire(&self, chain: &[ProviderKind], now: Instant) -> Option<AccountId> {
        let mut accounts = self.accounts.lock();
        for a in accounts.iter_mut() {
            if matches!(&a.health, AccountHealth::Cooldown { until, .. } if *until <= now) {
                a.health = AccountHealth::Healthy;
                a.window_requests = 0;
                a.window_input_tokens = 0;
                a.window_output_tokens = 0;
            }
        }
        for provider in chain {
            let mut usable = accounts
                .iter_mut()
                .filter(|a| a.provider == *provider && a.health == AccountHealth::Healthy);
            let picked = match self.strategy {
                Strategy::Priority => usable.next(),
                Strategy::RoundRobin => usable.min_by_key(|a| a.last_used_at),
            };
            if let Some(a) = picked {
                a.last_used_at = Some(now);
                a.window_requests += 1;
                return Some(a.id.clone());
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
                reason: "auth expired".into(),
            },
        );
    }

    pub fn report_success(&self, id: &AccountId, input_tokens: u64, output_tokens: u64) {
        let mut accounts = self.accounts.lock();
        if let Some(a) = accounts.iter_mut().find(|a| a.id == *id) {
            a.strikes = 0;
            a.window_input_tokens += input_tokens;
            a.window_output_tokens += output_tokens;
        }
    }

    pub fn snapshot(&self) -> Vec<AccountState> {
        self.accounts.lock().clone()
    }

    fn set_health(&self, id: &AccountId, health: AccountHealth) {
        if let Some(a) = self.accounts.lock().iter_mut().find(|a| a.id == *id) {
            a.health = health;
        }
    }
}

/// Extract a wait from a CLI rate-limit message: `...limit reached|<unix secs>`,
/// `retry after 30`, or `try again in 5 minutes` / `2h` / `45s`.
pub fn parse_cooldown(message: &str, now_epoch_secs: u64) -> Option<Duration> {
    if let Some((_, tail)) = message.split_once('|') {
        let digits: String = tail
            .trim()
            .chars()
            .take_while(char::is_ascii_digit)
            .collect();
        if let Ok(at) = digits.parse::<u64>() {
            return Some(capped(at.saturating_sub(now_epoch_secs)));
        }
    }
    let lower = message.to_ascii_lowercase();
    let tokens: Vec<&str> = lower
        .split(|c: char| !c.is_ascii_alphanumeric())
        .filter(|t| !t.is_empty())
        .collect();
    for (i, tok) in tokens.iter().enumerate() {
        let digits_end = tok.find(|c: char| !c.is_ascii_digit()).unwrap_or(tok.len());
        let Ok(n) = tok[..digits_end].parse::<u64>() else {
            continue;
        };
        let unit = match &tok[digits_end..] {
            "" => tokens.get(i + 1).copied().unwrap_or(""),
            u => u,
        };
        let secs = match unit {
            "s" | "sec" | "secs" | "second" | "seconds" => n,
            "m" | "min" | "mins" | "minute" | "minutes" => n.saturating_mul(60),
            "h" | "hr" | "hrs" | "hour" | "hours" => n.saturating_mul(3600),
            // `retry after 30` / `retry-after: 30` is seconds.
            _ if i > 0 && tokens[i - 1] == "after" => n,
            _ => continue,
        };
        return Some(capped(secs));
    }
    None
}

fn capped(secs: u64) -> Duration {
    Duration::from_secs(secs).min(MAX_PARSED_COOLDOWN)
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
}
