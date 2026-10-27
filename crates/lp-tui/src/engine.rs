//! `CliPoolEngine`: an `LlmEngine` that runs each call on a pooled CLI account and fails over
//! to the next one when the CLI reports a rate limit or an expired login.

use std::future::Future;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use lp_cli_driver::accounts::{AccountHealth, AccountPool, AccountState, ProviderKind};
use lp_cli_driver::engine::{
    CliKind, DriverError, LlmEngine, StreamDelta, StructuredOutput, StructuredRequest,
    SubprocessEngine, TurnRequest, TurnResponse,
};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub struct CliPoolEngine {
    pool: Arc<AccountPool>,
    /// Slot pin: `<provider>:auto` prefers that provider, an account id pins that account.
    pin: String,
}

impl CliPoolEngine {
    pub fn new(pool: Arc<AccountPool>, pin: impl Into<String>) -> Self {
        Self {
            pool,
            pin: pin.into(),
        }
    }

    fn chain(&self) -> [ProviderKind; 3] {
        use ProviderKind::*;
        match self.pin.split(':').next() {
            Some("codex") => [CodexCli, ClaudeCli, AgyCli],
            Some("agy") => [AgyCli, ClaudeCli, CodexCli],
            _ => [ClaudeCli, CodexCli, AgyCli],
        }
    }

    fn pick(&self) -> Option<AccountState> {
        let snap = self.pool.snapshot();
        let pinned = snap
            .iter()
            .find(|a| a.id.0 == self.pin && a.health == AccountHealth::Healthy);
        let id = match pinned {
            Some(a) => a.id.clone(),
            None => self.pool.acquire(&self.chain(), Instant::now())?,
        };
        snap.into_iter().find(|a| a.id == id)
    }

    /// Runs `call` on successive accounts until one succeeds or fails for another reason.
    async fn with_account<T, F, Fut>(&self, call: F) -> Result<T, DriverError>
    where
        F: Fn(SubprocessEngine) -> Fut,
        Fut: Future<Output = Result<T, DriverError>>,
    {
        let mut last = None;
        for _ in 0..self.pool.snapshot().len() {
            let Some(acct) = self.pick() else { break };
            let mut engine = SubprocessEngine::new(match acct.provider {
                ProviderKind::ClaudeCli => CliKind::Claude,
                ProviderKind::CodexCli => CliKind::Codex,
                ProviderKind::AgyCli => CliKind::Agy,
            })?;
            engine.config_dir = acct.config_dir.clone();
            match call(engine).await {
                Ok(v) => {
                    self.pool.report_success(&acct.id, 0, 0);
                    return Ok(v);
                }
                Err(DriverError::Exit { code, stderr }) => match classify(&stderr) {
                    Failure::Auth => self.pool.report_auth_expired(&acct.id),
                    Failure::RateLimit => {
                        let epoch = SystemTime::now()
                            .duration_since(UNIX_EPOCH)
                            .map_or(0, |d| d.as_secs());
                        self.pool
                            .report_rate_limit(&acct.id, &stderr, Instant::now(), epoch);
                    }
                    Failure::Other => return Err(DriverError::Exit { code, stderr }),
                },
                Err(e) => return Err(e),
            }
            last = Some(DriverError::Exit {
                code: None,
                stderr: format!("{} unavailable", acct.id.0),
            });
        }
        Err(last.unwrap_or(DriverError::Exit {
            code: None,
            stderr: "no usable CLI account; run `learnpuppy auth status`".into(),
        }))
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Failure {
    Auth,
    RateLimit,
    Other,
}

fn classify(stderr: &str) -> Failure {
    let s = stderr.to_lowercase();
    let has = |needles: &[&str]| needles.iter().any(|n| s.contains(n));
    if has(&[
        "rate limit",
        "rate_limit",
        "usage limit",
        "limit reached",
        "429",
    ]) {
        Failure::RateLimit
    } else if has(&[
        "not logged in",
        "unauthorized",
        "401",
        "login",
        "token expired",
    ]) {
        Failure::Auth
    } else {
        Failure::Other
    }
}

#[async_trait]
impl LlmEngine for CliPoolEngine {
    async fn stream_turn(
        &self,
        req: TurnRequest,
        event_tx: mpsc::Sender<StreamDelta>,
        cancel: CancellationToken,
    ) -> Result<TurnResponse, DriverError> {
        self.with_account(|e| {
            let (req, tx, cancel) = (req.clone(), event_tx.clone(), cancel.clone());
            async move { e.stream_turn(req, tx, cancel).await }
        })
        .await
    }

    async fn complete_structured<T: DeserializeOwned + JsonSchema + Send>(
        &self,
        req: StructuredRequest,
        cancel: CancellationToken,
    ) -> Result<StructuredOutput<T>, DriverError> {
        self.with_account(|e| {
            let (req, cancel) = (req.clone(), cancel.clone());
            async move { e.complete_structured::<T>(req, cancel).await }
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_cli_driver::accounts::{AccountId, DiscoveredAccount, Strategy};

    fn acct(id: &str, provider: ProviderKind) -> DiscoveredAccount {
        DiscoveredAccount {
            id: AccountId(id.into()),
            provider,
            email: None,
            account_uuid: None,
            config_dir: None,
            rate_limit_tier: None,
            auth_expired: false,
        }
    }

    fn engine(pin: &str) -> CliPoolEngine {
        let pool = AccountPool::new(
            vec![
                acct("claude:a", ProviderKind::ClaudeCli),
                acct("claude:b", ProviderKind::ClaudeCli),
                acct("codex:c", ProviderKind::CodexCli),
            ],
            Strategy::Priority,
        );
        CliPoolEngine::new(Arc::new(pool), pin)
    }

    #[test]
    fn classifies_cli_failures() {
        assert_eq!(
            classify("Claude usage limit reached|1700"),
            Failure::RateLimit
        );
        assert_eq!(classify("Error: Not logged in"), Failure::Auth);
        assert_eq!(classify("segfault"), Failure::Other);
    }

    #[test]
    fn pin_selects_account_then_provider_then_falls_back() {
        let e = engine("claude:b");
        assert_eq!(e.pick().unwrap().id.0, "claude:b");
        let e = engine("codex:auto");
        assert_eq!(e.pick().unwrap().id.0, "codex:c");
        let e = engine("claude:auto");
        assert_eq!(e.pick().unwrap().id.0, "claude:a");
        e.pool.report_auth_expired(&AccountId("claude:a".into()));
        e.pool.report_auth_expired(&AccountId("claude:b".into()));
        assert_eq!(e.pick().unwrap().id.0, "codex:c");
    }

    #[tokio::test]
    async fn fails_over_on_rate_limit_and_stops_on_other_errors() {
        let e = engine("claude:auto");
        let seen = parking_lot::Mutex::new(0);
        let out = e
            .with_account(|_| {
                *seen.lock() += 1;
                let n = *seen.lock();
                async move {
                    if n < 3 {
                        Err(DriverError::Exit {
                            code: Some(1),
                            stderr: "rate limit".into(),
                        })
                    } else {
                        Ok(n)
                    }
                }
            })
            .await
            .unwrap();
        assert_eq!(out, 3);
        let err = e
            .with_account(|_| async {
                Err::<(), _>(DriverError::Exit {
                    code: Some(2),
                    stderr: "boom".into(),
                })
            })
            .await;
        assert!(matches!(err, Err(DriverError::Exit { code: Some(2), .. })));
    }

    #[tokio::test]
    async fn reports_when_nothing_is_usable() {
        let e = CliPoolEngine::new(
            Arc::new(AccountPool::new(vec![], Strategy::Priority)),
            "claude:auto",
        );
        let err = e.with_account(|_| async { Ok(()) }).await.unwrap_err();
        assert!(err.to_string().contains("no usable CLI account"));
    }
}
