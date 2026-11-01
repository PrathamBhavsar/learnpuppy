use std::collections::HashSet;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use serde_json::Value;
use uuid::Uuid;

use super::{AccountId, DiscoveredAccount, ModelInfo, ProviderKind};

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
    /// Also look for `kiro-cli` and `opencode` (runs `whoami`, `auth list`, `models`: short, local, no
    /// network, no keychain). Off by default so file-only discovery stays hermetic.
    pub probe_clis: bool,
    /// Override the binaries to probe (tests); `None` searches `PATH` and common install dirs.
    pub kiro_program: Option<PathBuf>,
    pub opencode_program: Option<PathBuf>,
}

/// Probe commands must answer within this long.
const PROBE_TIMEOUT: Duration = Duration::from_secs(8);
/// How long a model list stays cached before the CLI is asked again.
pub const MODEL_TTL: Duration = Duration::from_secs(600);

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
    if roots.probe_clis {
        out.extend(read_kiro(roots));
        out.extend(read_opencode(roots));
    }
    out
}

/// `name` on `PATH`, else in the usual install dirs (a GUI-launched server has a short `PATH`).
fn find_program(name: &str, home: &Path) -> Option<PathBuf> {
    let path = std::env::var_os("PATH").unwrap_or_default();
    std::env::split_paths(&path)
        .chain([
            home.join(".local/bin"),
            PathBuf::from("/opt/homebrew/bin"),
            PathBuf::from("/usr/local/bin"),
            home.join(".opencode/bin"),
        ])
        .map(|d| d.join(name))
        .find(|p| p.is_file())
}

/// Runs a short read-only CLI command; `None` on spawn failure, non-zero exit or timeout.
fn probe(program: &Path, args: &[&str], home: &Path) -> Option<String> {
    let mut child = Command::new(program)
        .args(args)
        .env_clear()
        .env("HOME", home)
        .env("PATH", std::env::var_os("PATH").unwrap_or_default())
        .env("USER", std::env::var_os("USER").unwrap_or_default())
        .env("LOGNAME", std::env::var_os("LOGNAME").unwrap_or_default())
        .current_dir(std::env::temp_dir())
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let mut out = child.stdout.take()?;
    let reader = std::thread::spawn(move || {
        let mut s = String::new();
        let _ = out.read_to_string(&mut s);
        s
    });
    let deadline = Instant::now() + PROBE_TIMEOUT;
    let ok = loop {
        match child.try_wait() {
            Ok(Some(st)) => break st.success(),
            Ok(None) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(20));
            }
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                break false;
            }
        }
    };
    let text = reader.join().ok()?;
    ok.then_some(text)
}

type ModelCache = Mutex<Vec<(String, Instant, Vec<ModelInfo>)>>;
static MODELS: Mutex<Vec<(String, Instant, Vec<ModelInfo>)>> = Mutex::new(Vec::new());

/// Model list from `fetch`, cached per `key` for `MODEL_TTL`.
fn cached_models(key: String, fetch: impl FnOnce() -> Vec<ModelInfo>) -> Vec<ModelInfo> {
    let cache: &ModelCache = &MODELS;
    if let Ok(c) = cache.lock() {
        if let Some((_, at, m)) = c.iter().find(|(k, ..)| *k == key) {
            if at.elapsed() < MODEL_TTL {
                return m.clone();
            }
        }
    }
    let fresh = fetch();
    if let Ok(mut c) = cache.lock() {
        c.retain(|(k, ..)| *k != key);
        c.push((key, Instant::now(), fresh.clone()));
    }
    fresh
}

/// `kiro-cli whoami -f json` -> `{"accountType":..,"email":..}`; models from `chat --list-models`.
fn read_kiro(roots: &DiscoverRoots) -> Option<DiscoveredAccount> {
    let program = roots
        .kiro_program
        .clone()
        .or_else(|| find_program("kiro-cli", &roots.home))?;
    let who = probe(&program, &["whoami", "-f", "json"], &roots.home)
        .and_then(|t| serde_json::from_str::<Value>(t.trim()).ok());
    let email = who.as_ref().and_then(|w| str_at(w, "email"));
    let models = if email.is_some() {
        cached_models(format!("kiro:{}", program.display()), || {
            probe(
                &program,
                &["chat", "--list-models", "--format", "json"],
                &roots.home,
            )
            .map(|t| parse_kiro_models(&t))
            .unwrap_or_default()
        })
    } else {
        Vec::new()
    };
    Some(DiscoveredAccount {
        id: AccountId(format!("kiro:{}", email.as_deref().unwrap_or("default"))),
        provider: ProviderKind::KiroCli,
        account_uuid: None,
        config_dir: None,
        rate_limit_tier: who.as_ref().and_then(|w| str_at(w, "accountType")),
        auth_expired: email.is_none(),
        email,
        models,
    })
}

pub(crate) fn parse_kiro_models(json: &str) -> Vec<ModelInfo> {
    let Ok(v) = serde_json::from_str::<Value>(json.trim()) else {
        return Vec::new();
    };
    v.get("models")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|m| {
            Some(ModelInfo {
                id: str_at(m, "model_id")?,
                free: false,
                credit_multiplier: m.get("rate_multiplier").and_then(Value::as_f64),
            })
        })
        .collect()
}

/// Opencode accounts: `opencode:opencode` (free models, no login) plus one per logged-in provider that
/// has models. Google is skipped: agy covers it (and its antigravity models are not ours to offer).
fn read_opencode(roots: &DiscoverRoots) -> Vec<DiscoveredAccount> {
    let Some(program) = roots
        .opencode_program
        .clone()
        .or_else(|| find_program("opencode", &roots.home))
    else {
        return Vec::new();
    };
    let all = cached_models(format!("opencode:{}", program.display()), || {
        probe(&program, &["models"], &roots.home)
            .map(|t| parse_opencode_models(&t))
            .unwrap_or_default()
    });
    let logged_in: Vec<String> = probe(&program, &["auth", "list"], &roots.home)
        .map(|t| parse_opencode_auth(&t))
        .unwrap_or_default();
    let mut providers = vec!["opencode".to_owned()];
    for p in logged_in {
        if !providers.contains(&p) && all.iter().any(|m| model_provider(&m.id) == p) {
            providers.push(p);
        }
    }
    providers
        .into_iter()
        .filter_map(|p| {
            let models: Vec<ModelInfo> = all
                .iter()
                .filter(|m| model_provider(&m.id) == p)
                .cloned()
                .collect();
            // A provider with no usable model is not an account (reported as NoModel only once seen failing).
            (!models.is_empty()).then(|| DiscoveredAccount {
                id: AccountId(format!("opencode:{p}")),
                provider: ProviderKind::OpencodeCli,
                email: None,
                account_uuid: None,
                config_dir: None,
                rate_limit_tier: None,
                auth_expired: false,
                models,
            })
        })
        .collect()
}

fn model_provider(model: &str) -> &str {
    model.split_once('/').map_or(model, |(p, _)| p)
}

/// `opencode models`: one `provider/model` per line. Antigravity models and Google are not offered.
pub(crate) fn parse_opencode_models(text: &str) -> Vec<ModelInfo> {
    text.lines()
        .map(str::trim)
        .filter(|l| l.contains('/') && !l.contains(char::is_whitespace))
        .filter(|l| !l.contains("antigravity") && !l.starts_with("google/"))
        .map(|id| ModelInfo {
            id: id.to_owned(),
            free: id.ends_with("-free") || id == "opencode/big-pickle",
            credit_multiplier: None,
        })
        .collect()
}

/// `opencode auth list`: `<Provider>  <detail> stored` rows (ANSI colours stripped), lowercased.
pub(crate) fn parse_opencode_auth(text: &str) -> Vec<String> {
    let mut plain = String::new();
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\u{1b}' {
            for n in chars.by_ref() {
                if n.is_ascii_alphabetic() {
                    break;
                }
            }
        } else {
            plain.push(c);
        }
    }
    plain
        .lines()
        .filter(|l| l.contains("stored") || l.contains("oauth") || l.contains("api"))
        .filter_map(|l| l.split_whitespace().next())
        .map(str::to_ascii_lowercase)
        .filter(|p| p.chars().all(|c| c.is_ascii_alphanumeric() || c == '-'))
        .collect()
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
        models: Vec::new(),
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
        models: Vec::new(),
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
        models: Vec::new(),
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

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn script(dir: &Path, name: &str, body: &str) -> PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, format!("#!/bin/sh\n{body}\n")).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755)).unwrap();
        p
    }

    fn roots(dir: &Path) -> DiscoverRoots {
        DiscoverRoots {
            home: dir.join("home"),
            probe_clis: true,
            ..Default::default()
        }
    }

    const KIRO: &str = r#"case "$1" in
whoami) echo '{"accountType":"SocialGoogle","email":"k@x.com"}';;
chat) echo '{"models":[{"model_id":"auto","rate_multiplier":1.0},{"model_id":"qwen3-coder-next","rate_multiplier":0.05}],"default_model":"auto"}';;
esac"#;

    const OPENCODE: &str = r#"case "$1" in
models) printf '%s\n' opencode/big-pickle opencode/glm-5.3-flash-free opencode/paid-model anthropic/claude-x google/antigravity-gemini-3-pro google/gemini-2;;
auth) printf '\033[0mGoogle  me@x.com stored\nAnthropic  api stored\n';;
esac"#;

    #[test]
    fn kiro_and_opencode_accounts_with_models_are_discovered_without_network() {
        let d = tempfile::tempdir().unwrap();
        let mut r = roots(d.path());
        r.kiro_program = Some(script(d.path(), "kiro", KIRO));
        r.opencode_program = Some(script(d.path(), "opencode", OPENCODE));
        let found = discover(&r);
        let ids: Vec<_> = found.iter().map(|a| a.id.0.as_str()).collect();
        assert_eq!(
            ids,
            ["kiro:k@x.com", "opencode:opencode", "opencode:anthropic"]
        );
        let kiro = &found[0];
        assert!(!kiro.auth_expired && kiro.provider == ProviderKind::KiroCli);
        assert_eq!(kiro.models[1].credit_multiplier, Some(0.05));
        let oc = &found[1];
        let free: Vec<_> = oc.models.iter().map(|m| (m.id.as_str(), m.free)).collect();
        assert_eq!(
            free,
            [
                ("opencode/big-pickle", true),
                ("opencode/glm-5.3-flash-free", true),
                ("opencode/paid-model", false)
            ]
        );
        // Google (agy's job) and antigravity models are never offered.
        assert!(found
            .iter()
            .all(|a| a.models.iter().all(|m| !m.id.starts_with("google/"))));
    }

    #[test]
    fn logged_out_kiro_needs_login_and_missing_clis_are_skipped() {
        let d = tempfile::tempdir().unwrap();
        let mut r = roots(d.path());
        r.kiro_program = Some(script(d.path(), "kiro", "echo 'Not logged in'"));
        r.opencode_program = Some(d.path().join("no-such-opencode"));
        let found = discover(&r);
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].auth_expired && found[0].id.0 == "kiro:default");
        assert!(found[0].models.is_empty());
        // Opt-in: without probe_clis nothing is run.
        r.probe_clis = false;
        assert!(discover(&r).is_empty());
    }

    #[test]
    fn hanging_probe_is_killed_and_model_list_is_cached() {
        let d = tempfile::tempdir().unwrap();
        let count = d.path().join("count");
        let body = format!("echo x >> {}\n{KIRO}", count.display());
        let mut r = roots(d.path());
        r.kiro_program = Some(script(d.path(), "kiro", &body));
        discover(&r);
        discover(&r);
        // whoami runs each time (login can change), the model list once per TTL.
        let calls = std::fs::read_to_string(&count).unwrap().lines().count();
        assert_eq!(calls, 3, "2 whoami + 1 list-models");
        assert!(MODEL_TTL.as_secs() >= 60);
    }

    #[test]
    fn real_cli_output_formats_parse() {
        let models = "opencode/beam-free\nopencode/big-pickle\nopencode/flint-40b-free\n";
        let m = parse_opencode_models(models);
        assert_eq!(m.len(), 3);
        assert!(m.iter().all(|m| m.free));
        let auth = "\u{1b}[90m┌\u{1b}[0m  Credentials\nGoogle  pratham@x.com\u{1b}[0mstored\n";
        assert_eq!(parse_opencode_auth(auth), ["google"]);
        let kiro = r#"{"models":[{"model_name":"auto","model_id":"auto","rate_multiplier":1.0,"rate_unit":"Credit"}],"default_model":"auto"}"#;
        assert_eq!(parse_kiro_models(kiro)[0].credit_multiplier, Some(1.0));
        assert!(parse_kiro_models("garbage").is_empty());
    }
}
