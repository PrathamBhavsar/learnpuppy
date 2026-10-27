use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::Value;
use uuid::Uuid;

use super::{AccountId, DiscoveredAccount, ProviderKind};

/// Where to look; injectable so tests can point at fixture directories.
#[derive(Debug, Clone, Default)]
pub struct DiscoverRoots {
    pub home: PathBuf,
    /// `$CLAUDE_CONFIG_DIR`.
    pub claude_config_dir: Option<PathBuf>,
    /// Extra profiles from `~/.learnpuppy/settings.json`.
    pub extra_claude_dirs: Vec<PathBuf>,
    /// Wall clock in unix milliseconds, for token-expiry checks.
    pub now_ms: u64,
}

/// Find every logged-in Claude, Codex and Agy account, in priority order.
pub fn discover(roots: &DiscoverRoots) -> Vec<DiscoveredAccount> {
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    for dir in claude_dirs(roots) {
        if let Some(acct) = read_claude(&dir, roots.now_ms) {
            if seen.insert(acct.id.clone()) {
                out.push(acct);
            }
        }
    }
    out.extend(read_codex(&roots.home));
    out.extend(read_agy(&roots.home, roots.now_ms));
    out
}

fn claude_dirs(roots: &DiscoverRoots) -> Vec<PathBuf> {
    let mut dirs = vec![roots.home.join(".claude")];
    if let Ok(rd) = std::fs::read_dir(roots.home.join(".claude-accounts")) {
        let mut subs: Vec<_> = rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.is_dir())
            .collect();
        subs.sort();
        dirs.extend(subs);
    }
    dirs.push(roots.home.join(".claude-work"));
    dirs.extend(roots.claude_config_dir.clone());
    dirs.extend(roots.extra_claude_dirs.iter().cloned());
    dirs
}

fn read_json(path: &Path) -> Option<Value> {
    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

fn str_at(v: &Value, key: &str) -> Option<String> {
    v.get(key)?.as_str().map(str::to_owned)
}

/// A profile's `.claude.json` lives inside it, except the default `~/.claude`,
/// whose file sits next to it as `~/.claude.json`.
fn read_claude(dir: &Path, now_ms: u64) -> Option<DiscoveredAccount> {
    let inner = dir.join(".claude.json");
    let sibling = dir.parent()?.join(".claude.json");
    let cfg = read_json(&inner).or_else(|| {
        (dir.file_name()? == ".claude")
            .then(|| read_json(&sibling))
            .flatten()
    })?;
    let acct = cfg.get("oauthAccount")?;
    let email = str_at(acct, "emailAddress")?;
    let expires_ms = read_json(&dir.join(".credentials.json"))
        .and_then(|c| c.get("claudeAiOauth")?.get("expiresAt")?.as_u64());
    Some(DiscoveredAccount {
        id: AccountId(format!("claude:{email}")),
        provider: ProviderKind::ClaudeCli,
        account_uuid: str_at(acct, "accountUuid").and_then(|s| Uuid::parse_str(&s).ok()),
        rate_limit_tier: str_at(acct, "userRateLimitTier").or_else(|| str_at(acct, "seatTier")),
        email: Some(email),
        config_dir: Some(dir.to_owned()),
        auth_expired: expires_ms.is_some_and(|e| e <= now_ms),
    })
}

fn read_codex(home: &Path) -> Option<DiscoveredAccount> {
    let auth = read_json(&home.join(".codex/auth.json"))?;
    let claims = auth
        .get("tokens")
        .and_then(|t| str_at(t, "id_token"))
        .and_then(|jwt| jwt_claims(&jwt));
    let email = claims.as_ref().and_then(|c| str_at(c, "email"));
    let plan = claims
        .as_ref()
        .and_then(|c| str_at(c.get("https://api.openai.com/auth")?, "chatgpt_plan_type"));
    Some(DiscoveredAccount {
        id: AccountId(format!("codex:{}", email.as_deref().unwrap_or("default"))),
        provider: ProviderKind::CodexCli,
        email,
        account_uuid: None,
        config_dir: None,
        rate_limit_tier: plan,
        auth_expired: false,
    })
}

fn read_agy(home: &Path, now_ms: u64) -> Option<DiscoveredAccount> {
    let creds = read_json(&home.join(".gemini/oauth_creds.json"))?;
    let email =
        read_json(&home.join(".gemini/google_accounts.json")).and_then(|a| str_at(&a, "active"));
    // An expired access token is routine while a refresh token exists.
    let expired = creds.get("expiry_date").and_then(Value::as_u64) <= Some(now_ms)
        && str_at(&creds, "refresh_token").is_none();
    Some(DiscoveredAccount {
        id: AccountId("agy:default".into()),
        provider: ProviderKind::AgyCli,
        email,
        account_uuid: None,
        config_dir: None,
        rate_limit_tier: None,
        auth_expired: expired,
    })
}

/// Decode the (unverified) payload of a JWT.
fn jwt_claims(jwt: &str) -> Option<Value> {
    let bytes = base64url_decode(jwt.split('.').nth(1)?)?;
    serde_json::from_slice(&bytes).ok()
}

fn base64url_decode(s: &str) -> Option<Vec<u8>> {
    let (mut out, mut acc, mut bits) = (Vec::new(), 0u32, 0u32);
    for c in s.trim_end_matches('=').bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'-' => 62,
            b'_' => 63,
            _ => return None,
        };
        acc = (acc << 6) | u32::from(v);
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}
