//! One failure taxonomy for all five CLIs (claude, codex, agy, kiro, opencode).
//!
//! Every CLI reports problems differently (stderr text, a non-zero exit, an `is_error` result line,
//! an `error` event on stdout). `classify` maps whatever text we captured onto `FailureKind`, so the
//! pool and the UI can react the same way regardless of provider.

use std::time::Duration;

use chrono::{Datelike, NaiveDate, TimeZone, Utc};

use super::DriverError;

/// Upper bound for a wait parsed from CLI output (a hostile message can claim anything). Monthly
/// credit exhaustion legitimately waits weeks.
pub const MAX_PARSED_WAIT: Duration = Duration::from_secs(45 * 24 * 3600);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailureKind {
    /// The CLI binary is not on `PATH`.
    NotInstalled,
    /// Installed but not logged in / not configured.
    NotLoggedIn,
    /// No network (DNS, connect, proxy). Never the account's fault: no cooldown, back off globally.
    Offline,
    /// Model id unknown, unavailable, not entitled, or the model list is empty.
    NoModel,
    /// Quota or rate limit; `resets_at` is unix epoch milliseconds when the CLI said when.
    RateLimited {
        resets_at: Option<u64>,
    },
    TimedOut,
    Other(String),
}

impl FailureKind {
    /// Stable lowercase tag for logs and API payloads.
    pub fn tag(&self) -> &'static str {
        match self {
            Self::NotInstalled => "not_installed",
            Self::NotLoggedIn => "not_logged_in",
            Self::Offline => "offline",
            Self::NoModel => "no_model",
            Self::RateLimited { .. } => "rate_limited",
            Self::TimedOut => "timed_out",
            Self::Other(_) => "other",
        }
    }
}

/// Lines that are CLI chatter and never say why a call failed (kiro INFO logs, codex MCP noise).
fn is_noise(line: &str) -> bool {
    line.starts_with("[INFO]") || line.contains("rmcp::") || line.contains("ExperimentalWarning")
}

/// `n` appears as a standalone number (not inside a longer number or an id).
fn has_code(lower: &str, n: &str) -> bool {
    lower.match_indices(n).any(|(i, _)| {
        let before = lower[..i].chars().next_back();
        let after = lower[i + n.len()..].chars().next();
        !before.is_some_and(|c| c.is_ascii_alphanumeric() || c == '-')
            && !after.is_some_and(|c| c.is_ascii_alphanumeric() || c == '-')
    })
}

/// Maps captured CLI output (stderr plus any error events) to a failure class.
/// `now_ms` anchors relative reset phrases ("resets in 50h").
pub fn classify(text: &str, now_ms: u64) -> FailureKind {
    let cleaned: String = text
        .lines()
        .filter(|l| !is_noise(l))
        .collect::<Vec<_>>()
        .join("\n");
    let lower = cleaned.to_ascii_lowercase();
    let any = |needles: &[&str]| needles.iter().any(|n| lower.contains(n));

    if any(&[
        "not logged in",
        "please run /login",
        "run `codex login`",
        "run codex login",
        "agy auth",
        "login required",
        "authentication required",
        "not authenticated",
        "please log in",
        "no credentials",
    ]) {
        return FailureKind::NotLoggedIn;
    }
    if any(&[
        "rate limit",
        "rate_limit",
        "ratelimit",
        "usage limit",
        "quota",
        "resource_exhausted",
        "too many requests",
        "credits",
        "free usage limit",
        "limit reached",
        "hit your limit",
        "exceeded your",
        "insufficient_quota",
        "overloaded",
    ]) && !any(&["credits remaining"])
        || has_code(&lower, "429")
        || has_code(&lower, "402")
    {
        return FailureKind::RateLimited {
            resets_at: parse_resets_at(&cleaned, now_ms),
        };
    }
    if (lower.contains("model")
        && any(&[
            "not found",
            "not available",
            "unavailable",
            "not supported",
            "does not exist",
            "unknown model",
            "issue with the selected model",
            "not entitled",
            "no access",
            "no models",
        ]))
        || any(&[
            "model_not_found",
            "provider.no-route",
            "no models available",
        ])
    {
        return FailureKind::NoModel;
    }
    if any(&[
        "econnrefused",
        "enotfound",
        "eai_again",
        "etimedout",
        "econnreset",
        "dns error",
        "could not resolve",
        "failed to lookup",
        "network is unreachable",
        "network error",
        "connection refused",
        "connection reset",
        "connection failed",
        "error sending request",
        "waiting for network",
        "unable to connect",
        "fetch failed",
        "offline",
        "proxy",
        "tls handshake",
        "no route to host",
    ]) {
        return FailureKind::Offline;
    }
    if any(&["timed out", "timeout", "deadline exceeded"]) {
        return FailureKind::TimedOut;
    }
    if has_code(&lower, "401") || any(&["unauthorized", "invalid api key", "invalid_token"]) {
        return FailureKind::NotLoggedIn;
    }
    FailureKind::Other(cleaned.trim().chars().take(300).collect())
}

impl DriverError {
    /// The taxonomy class of this error. `now_ms` anchors relative reset times.
    pub fn failure(&self, now_ms: u64) -> FailureKind {
        match self {
            Self::Timeout(_) => FailureKind::TimedOut,
            Self::Io(e) if e.kind() == std::io::ErrorKind::NotFound => FailureKind::NotInstalled,
            Self::Io(e) => classify(&e.to_string(), now_ms),
            Self::Exit { stderr, .. } => classify(stderr, now_ms),
            Self::Failed(m) | Self::Cli { message: m, .. } => classify(m, now_ms),
            Self::EmptyResponse | Self::NoStopReason => {
                FailureKind::Other("the CLI returned nothing".into())
            }
            Self::Cancelled | Self::Parse(_) | Self::FauxExhausted => {
                FailureKind::Other(self.to_string())
            }
        }
    }
}

/// When a limit resets, as unix epoch milliseconds: `retryAfterMs`, `Resets in 50h7m56s`,
/// `try again in 20 minutes`, `retry after 30 seconds`, `...|<unix secs>` or a calendar date
/// (`resets on 2026-12-01`, `resets Dec 1`). Capped at `MAX_PARSED_WAIT`.
pub fn parse_resets_at(text: &str, now_ms: u64) -> Option<u64> {
    let wait = parse_wait(text, now_ms / 1000)?;
    Some(now_ms.saturating_add(u64::try_from(wait.as_millis()).unwrap_or(u64::MAX)))
}

/// The wait a CLI message asks for (see `parse_resets_at`); `now_epoch_secs` anchors absolute times.
pub fn parse_wait(message: &str, now_epoch_secs: u64) -> Option<Duration> {
    if let Some(ms) = number_after(message, "retryAfterMs") {
        return Some(capped(ms / 1000));
    }
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
    // A duration phrase right after one of the wait keywords: "resets in 50h7m56s".
    for key in [
        "resets in",
        "reset in",
        "try again in",
        "retry in",
        "retry after",
        "retry-after",
        "again in",
        "available in",
        "wait ",
        "after ",
    ] {
        if let Some(i) = lower.find(key) {
            if let Some(s) = duration_phrase(&lower[i + key.len()..]) {
                return Some(capped(s));
            }
        }
    }
    // "resets in" style fallback anywhere ("45s", "2 hours") the old single-token way.
    if let Some(s) = single_token_wait(&lower) {
        return Some(capped(s));
    }
    date_wait(&lower, now_epoch_secs)
}

fn capped(secs: u64) -> Duration {
    Duration::from_secs(secs).min(MAX_PARSED_WAIT)
}

fn number_after(text: &str, key: &str) -> Option<u64> {
    let i = text.find(key)? + key.len();
    let digits: String = text[i..]
        .trim_start_matches(|c: char| !c.is_ascii_digit())
        .chars()
        .take_while(char::is_ascii_digit)
        .collect();
    digits.parse().ok()
}

/// `50h7m56s`, `1h 30m`, `20 minutes`, `2 days`, `30` (bare number = seconds, only right after a keyword).
fn duration_phrase(s: &str) -> Option<u64> {
    let s = s.trim_start_matches([' ', ':', '=']);
    let (mut total, mut groups, mut rest) = (0u64, 0, s);
    loop {
        rest = rest.trim_start();
        let digits_end = rest
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(rest.len());
        if digits_end == 0 {
            break;
        }
        let Ok(n) = rest[..digits_end].parse::<u64>() else {
            return None;
        };
        let after = rest[digits_end..].trim_start();
        let unit_end = after
            .find(|c: char| !c.is_ascii_alphabetic())
            .unwrap_or(after.len());
        let mult = match &after[..unit_end] {
            "d" | "day" | "days" => 86_400,
            "h" | "hr" | "hrs" | "hour" | "hours" => 3600,
            "m" | "min" | "mins" | "minute" | "minutes" => 60,
            "s" | "sec" | "secs" | "second" | "seconds" => 1,
            "" if groups == 0 => 1,
            _ => break,
        };
        total = total.saturating_add(n.saturating_mul(mult));
        groups += 1;
        rest = &after[unit_end..];
        if mult == 1 && unit_end == 0 {
            break;
        }
    }
    (groups > 0).then_some(total)
}

fn single_token_wait(lower: &str) -> Option<u64> {
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
            "d" | "day" | "days" => n.saturating_mul(86_400),
            _ if i > 0 && tokens[i - 1] == "after" => n,
            _ => continue,
        };
        return Some(secs);
    }
    None
}

const MONTHS: [&str; 12] = [
    "jan", "feb", "mar", "apr", "may", "jun", "jul", "aug", "sep", "oct", "nov", "dec",
];

/// `resets on 2026-12-01` or `resets dec 1 [2026]` / `december 1st`: midnight UTC of the next such date.
fn date_wait(lower: &str, now_epoch_secs: u64) -> Option<Duration> {
    let i = lower.find("reset")?;
    let tail = &lower[i..];
    let now = Utc
        .timestamp_opt(i64::try_from(now_epoch_secs).ok()?, 0)
        .single()?;
    let words: Vec<&str> = tail
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '-')
        .filter(|w| !w.is_empty())
        .collect();
    for (k, w) in words.iter().enumerate() {
        if let Ok(d) = NaiveDate::parse_from_str(w, "%Y-%m-%d") {
            return until(d, now_epoch_secs);
        }
        if let Some(m) = MONTHS.iter().position(|m| w.starts_with(m)) {
            let day: u32 = words
                .get(k + 1)?
                .trim_end_matches(|c: char| c.is_ascii_alphabetic())
                .parse()
                .ok()?;
            let year_given: Option<i32> = words
                .get(k + 2)
                .and_then(|y| y.parse().ok())
                .filter(|y| (2000..3000).contains(y));
            let month = u32::try_from(m).ok()? + 1;
            let year = year_given.unwrap_or_else(|| {
                let this = NaiveDate::from_ymd_opt(now.year(), month, day);
                if this.is_some_and(|d| until(d, now_epoch_secs).is_some()) {
                    now.year()
                } else {
                    now.year() + 1
                }
            });
            return until(NaiveDate::from_ymd_opt(year, month, day)?, now_epoch_secs);
        }
    }
    None
}

fn until(date: NaiveDate, now_epoch_secs: u64) -> Option<Duration> {
    let at = u64::try_from(date.and_hms_opt(0, 0, 0)?.and_utc().timestamp()).ok()?;
    (at > now_epoch_secs).then(|| capped(at - now_epoch_secs))
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: u64 = 1_800_000_000_000;

    fn fixture(name: &str) -> String {
        let p = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src/engine/fixtures")
            .join(name);
        std::fs::read_to_string(p).unwrap()
    }

    #[test]
    fn real_agy_quota_fixture_is_rate_limited_with_reset_time() {
        let k = classify(&fixture("agy_rate_limited.jsonl"), NOW);
        let FailureKind::RateLimited {
            resets_at: Some(at),
        } = k
        else {
            panic!("{k:?}")
        };
        // "Resets in 44h14m34s"
        assert_eq!(at, NOW + (44 * 3600 + 14 * 60 + 34) * 1000);
    }

    #[test]
    fn real_opencode_free_limit_fixture_uses_retry_after_ms() {
        let k = classify(&fixture("opencode_free_limit.jsonl"), NOW);
        assert_eq!(
            k,
            FailureKind::RateLimited {
                resets_at: Some(NOW + 19_898_000)
            }
        );
    }

    #[test]
    fn real_bad_model_fixtures_are_no_model() {
        for f in [
            "opencode_no_model.jsonl",
            "claude_bad_model.jsonl",
            "codex_bad_model.jsonl",
            "kiro_bad_model.jsonl",
        ] {
            assert_eq!(classify(&fixture(f), NOW), FailureKind::NoModel, "{f}");
        }
    }

    #[test]
    fn real_offline_fixtures_are_offline() {
        for f in ["opencode_offline.jsonl", "codex_offline.jsonl"] {
            assert_eq!(classify(&fixture(f), NOW), FailureKind::Offline, "{f}");
        }
    }

    #[test]
    fn real_not_logged_in_fixture() {
        assert_eq!(
            classify(&fixture("claude_not_logged_in.jsonl"), NOW),
            FailureKind::NotLoggedIn
        );
    }

    #[test]
    fn reset_phrasings() {
        let s = |t: &str| parse_wait(t, 1_800_000_000).map(|d| d.as_secs());
        assert_eq!(s("Resets in 50h7m56s."), Some(50 * 3600 + 7 * 60 + 56));
        assert_eq!(s("please try again in 20 minutes"), Some(1200));
        assert_eq!(s("retry after 30 seconds"), Some(30));
        assert_eq!(s("retry-after: 30"), Some(30));
        assert_eq!(s("resets in 45s"), Some(45));
        assert_eq!(s("try again in 1h 30m"), Some(5400));
        assert_eq!(s("try again in 2 days"), Some(172_800));
        assert_eq!(s("limit reached|1800000500"), Some(500));
        assert_eq!(s(r#"{"retryAfterMs":19898000}"#), Some(19_898));
        assert_eq!(s("HTTP 429"), None);
        // 1_800_000_000 = 2027-01-15T08:00:00Z
        let d = s("Monthly credits exhausted. Resets on 2027-02-01").unwrap();
        assert_eq!(d, (16 * 86_400) + 16 * 3600);
        assert_eq!(s("credits used up, resets Feb 1"), Some(d));
        assert_eq!(s("credits used up, resets February 1st 2027"), Some(d));
        assert_eq!(s("resets on 2020-01-01"), None);
    }

    #[test]
    fn quota_text_variants() {
        for t in [
            "HTTP 429 Too Many Requests",
            "{\"status\":402,\"message\":\"payment required\"}",
            "You have exceeded your monthly credit quota",
            "rate_limit_error: slow down",
            "RESOURCE_EXHAUSTED (code 429)",
        ] {
            assert!(
                matches!(classify(t, NOW), FailureKind::RateLimited { .. }),
                "{t}"
            );
        }
        // A request id containing 429 is not an HTTP status.
        assert!(matches!(
            classify("boom (request_id: 85b-429-0d69)", NOW),
            FailureKind::Other(_)
        ));
    }

    #[test]
    fn other_classes() {
        assert_eq!(classify("Not logged in", NOW), FailureKind::NotLoggedIn);
        assert_eq!(classify("401 Unauthorized", NOW), FailureKind::NotLoggedIn);
        assert_eq!(
            classify("dns error: failed to lookup address", NOW),
            FailureKind::Offline
        );
        assert_eq!(classify("request timed out", NOW), FailureKind::TimedOut);
        assert_eq!(
            classify("model gpt-9 does not exist", NOW),
            FailureKind::NoModel
        );
        // Chatter is ignored: kiro INFO lines and codex MCP noise never decide the class.
        assert!(matches!(
            classify("[INFO] network error retrying\nboom", NOW),
            FailureKind::Other(m) if m == "boom"
        ));
    }

    #[test]
    fn driver_error_maps_to_the_taxonomy() {
        assert_eq!(
            DriverError::Timeout(Duration::from_secs(1)).failure(NOW),
            FailureKind::TimedOut
        );
        assert_eq!(
            DriverError::Io(std::io::ErrorKind::NotFound.into()).failure(NOW),
            FailureKind::NotInstalled
        );
        assert_eq!(
            DriverError::Exit {
                code: Some(1),
                stderr: "429 slow down".into()
            }
            .failure(NOW)
            .tag(),
            "rate_limited"
        );
    }
}
