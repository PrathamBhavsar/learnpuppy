//! The structured "your AI is not available" error. `CliPoolEngine` returns it inside a
//! `DriverError::Failed` (the driver error enum is not ours to extend), and the chat runtime
//! decodes it into the turn error the web UI explains (docs/web/13-ai-connections.md).

use lp_cli_driver::engine::{DriverError, FailureKind};
use serde::{Deserialize, Serialize};

/// Marks a `DriverError::Failed` message that carries an [`AiFailure`] as JSON.
const MARK: &str = "lp-ai:";

/// The turn-error codes the UI knows. Wire value: `SCREAMING_SNAKE`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum AiCode {
    Offline,
    NotConnected,
    NoModel,
    RateLimited,
    TimedOut,
    NoResponse,
    Unknown,
}

impl AiCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Offline => "OFFLINE",
            Self::NotConnected => "NOT_CONNECTED",
            Self::NoModel => "NO_MODEL",
            Self::RateLimited => "RATE_LIMITED",
            Self::TimedOut => "TIMED_OUT",
            Self::NoResponse => "NO_RESPONSE",
            Self::Unknown => "UNKNOWN",
        }
    }
}

/// Why no AI could answer, and what the learner can do about it.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AiFailure {
    pub code: AiCode,
    /// One friendly sentence.
    pub message: String,
    /// Unix epoch ms when the soonest limit lifts (`RATE_LIMITED`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resets_at: Option<u64>,
    /// `claude`, `codex`, `agy`, `kiro` or `opencode`; the provider the message is about.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// A command or place to go next (`kiro-cli login`, `Accounts`, `Settings`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
    /// For `OFFLINE`: seconds until the next automatic retry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_in_secs: Option<u64>,
}

impl AiFailure {
    pub fn new(code: AiCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            resets_at: None,
            provider: None,
            model: None,
            action: None,
            retry_in_secs: None,
        }
    }

    /// The error a failed engine call returns.
    pub fn into_error(self) -> DriverError {
        DriverError::Failed(format!(
            "{MARK}{}",
            serde_json::to_string(&self).unwrap_or_default()
        ))
    }

    /// The structured failure inside `e`, if the pool engine produced it.
    pub fn from_error(e: &DriverError) -> Option<Self> {
        match e {
            DriverError::Failed(m) => serde_json::from_str(m.strip_prefix(MARK)?).ok(),
            _ => None,
        }
    }

    /// A friendly `AiFailure` for any driver error (what the chat runtime shows when the pool
    /// engine did not already produce one).
    pub fn of_driver(e: &DriverError, now_ms: u64) -> Self {
        if let Some(f) = Self::from_error(e) {
            return f;
        }
        match e.failure(now_ms) {
            FailureKind::Offline => Self::new(
                AiCode::Offline,
                "You're offline. LearnPuppy will retry when the network is back.",
            ),
            FailureKind::NotLoggedIn | FailureKind::NotInstalled => Self::new(
                AiCode::NotConnected,
                "No AI is connected. Connect one on the Accounts screen.",
            ),
            FailureKind::NoModel => Self::new(
                AiCode::NoModel,
                "The chosen model is not available. Pick another in Settings.",
            ),
            FailureKind::RateLimited { resets_at } => {
                let mut f = Self::new(AiCode::RateLimited, "Your AI hit its usage limit.");
                f.resets_at = resets_at;
                f
            }
            FailureKind::TimedOut => Self::new(
                AiCode::TimedOut,
                "The model took too long to respond. Try again.",
            ),
            FailureKind::Other(t) => Self::new(
                AiCode::Unknown,
                format!(
                    "The model call failed: {}",
                    t.chars().take(200).collect::<String>()
                ),
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_through_a_driver_error() {
        let mut f = AiFailure::new(AiCode::RateLimited, "limited");
        f.resets_at = Some(1_700_000_000_000);
        f.provider = Some("opencode".into());
        let back = AiFailure::from_error(&f.clone().into_error()).unwrap();
        assert_eq!(back, f);
        assert!(AiFailure::from_error(&DriverError::Failed("plain".into())).is_none());
        assert_eq!(AiCode::NotConnected.as_str(), "NOT_CONNECTED");
        let j = serde_json::to_value(&f).unwrap();
        assert_eq!(j["code"], "RATE_LIMITED");
    }

    #[test]
    fn plain_driver_errors_get_friendly_codes() {
        let t = AiFailure::of_driver(&DriverError::Timeout(std::time::Duration::from_secs(1)), 0);
        assert_eq!(t.code, AiCode::TimedOut);
        let o = AiFailure::of_driver(
            &DriverError::Exit {
                code: Some(1),
                stderr: "connect ECONNREFUSED 127.0.0.1".into(),
            },
            0,
        );
        assert_eq!(o.code, AiCode::Offline);
        let u = AiFailure::of_driver(&DriverError::Parse("x".into()), 0);
        assert_eq!(u.code, AiCode::Unknown);
    }
}
