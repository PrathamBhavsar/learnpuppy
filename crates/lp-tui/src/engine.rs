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
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Stream channel capacity and how much stdout tail is kept for failure classification.
const STREAM_CAP: usize = 512;
const TAIL: usize = 4096;

/// What one attempt has produced so far.
#[derive(Default)]
struct Seen {
    stdout: Mutex<String>,
    emitted: AtomicBool,
}

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
    async fn with_account<T, F, Fut>(&self, seen: &Seen, call: F) -> Result<T, DriverError>
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
            seen.stdout.lock().clear();
            seen.emitted.store(false, Ordering::Relaxed);
            match call(engine).await {
                Ok(v) => {
                    self.pool.report_success(&acct.id, 0, 0);
                    return Ok(v);
                }
                Err(DriverError::Exit { code, stderr }) => {
                    match classify(&stderr, &seen.stdout.lock()) {
                        // Text already reached the learner; a retry would repeat it.
                        _ if seen.emitted.load(Ordering::Relaxed) => {
                            return Err(DriverError::Exit { code, stderr });
                        }
                        Failure::Auth => self.pool.report_auth_expired(&acct.id),
                        Failure::RateLimit => {
                            let epoch = SystemTime::now()
                                .duration_since(UNIX_EPOCH)
                                .map_or(0, |d| d.as_secs());
                            self.pool
                                .report_rate_limit(&acct.id, &stderr, Instant::now(), epoch);
                        }
                        Failure::Other => return Err(DriverError::Exit { code, stderr }),
                    }
                }
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

/// Words of `text` (split on anything non-alphanumeric), lowercased.
fn words(text: &str) -> Vec<String> {
    text.split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// Whole-word phrase match, so a path or port containing `401` never counts.
fn has_phrase(w: &[String], phrase: &[&str]) -> bool {
    w.windows(phrase.len())
        .any(|win| win.iter().zip(phrase).all(|(a, b)| a == b))
}

/// A bare status code counts only right after a word that introduces one.
fn has_code(w: &[String], code: &str) -> bool {
    w.windows(2).any(|win| {
        win[1] == code && matches!(win[0].as_str(), "status" | "http" | "error" | "code")
    })
}

/// Claude prints its usage-limit message on stdout, so both streams are searched.
fn classify(stderr: &str, stdout: &str) -> Failure {
    let w = words(&format!("{stderr}\n{stdout}"));
    let any = |ps: &[&[&str]]| ps.iter().any(|p| has_phrase(&w, p));
    if any(&[
        &["rate", "limit"],
        &["usage", "limit"],
        &["limit", "reached"],
    ]) || has_code(&w, "429")
    {
        Failure::RateLimit
    } else if any(&[
        &["not", "logged", "in"],
        &["unauthorized"],
        &["token", "expired"],
        &["run", "login"],
    ]) || has_code(&w, "401")
    {
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
        let seen = Seen::default();
        self.with_account(&seen, |e| {
            let (req, cancel) = (req.clone(), cancel.clone());
            // Tee the text deltas so a limit message printed on stdout can be classified.
            let (tx, mut rx) = mpsc::channel(STREAM_CAP);
            let (seen, fwd) = (&seen, event_tx.clone());
            async move {
                let tee = async {
                    while let Some(d) = rx.recv().await {
                        if let StreamDelta::Text(t) = &d {
                            seen.emitted.store(true, Ordering::Relaxed);
                            let mut o = seen.stdout.lock();
                            o.push_str(t);
                            let cut = o.len().saturating_sub(TAIL);
                            let cut = (cut..).find(|i| o.is_char_boundary(*i)).unwrap_or(0);
                            o.drain(..cut);
                        }
                        let _ = fwd.send(d).await;
                    }
                };
                let (r, ()) = tokio::join!(e.stream_turn(req, tx, cancel), tee);
                r
            }
        })
        .await
    }

    async fn complete_structured<T: DeserializeOwned + JsonSchema + Send>(
        &self,
        req: StructuredRequest,
        cancel: CancellationToken,
    ) -> Result<StructuredOutput<T>, DriverError> {
        self.with_account(&Seen::default(), |e| {
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
            classify("Claude usage limit reached|1700", ""),
            Failure::RateLimit
        );
        assert_eq!(classify("Error: Not logged in", ""), Failure::Auth);
        assert_eq!(classify("segfault", ""), Failure::Other);
    }

    #[test]
    fn classify_is_word_bounded_and_reads_stdout() {
        for noise in [
            "open /home/u/401/x: denied",
            "connect 127.0.0.1:4290 refused",
            "cannot read login.rs",
            "429 apples",
        ] {
            assert_eq!(classify(noise, ""), Failure::Other, "{noise}");
        }
        assert_eq!(classify("HTTP 401", ""), Failure::Auth);
        assert_eq!(
            classify("", "Claude usage limit reached|17"),
            Failure::RateLimit
        );
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
    async fn no_failover_after_text_was_emitted() {
        let e = engine("claude:auto");
        let seen = Seen::default();
        let n = parking_lot::Mutex::new(0);
        let r = e
            .with_account(&seen, |_| {
                *n.lock() += 1;
                seen.emitted.store(true, Ordering::Relaxed);
                async {
                    Err::<(), _>(DriverError::Exit {
                        code: Some(1),
                        stderr: "rate limit".into(),
                    })
                }
            })
            .await;
        assert!(r.is_err());
        assert_eq!(*n.lock(), 1);
    }

    #[tokio::test]
    async fn fails_over_on_rate_limit_and_stops_on_other_errors() {
        let e = engine("claude:auto");
        let seen = parking_lot::Mutex::new(0);
        let out = e
            .with_account(&Seen::default(), |_| {
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
            .with_account(&Seen::default(), |_| async {
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
        let err = e
            .with_account(&Seen::default(), |_| async { Ok(()) })
            .await
            .unwrap_err();
        assert!(err.to_string().contains("no usable CLI account"));
    }
}
