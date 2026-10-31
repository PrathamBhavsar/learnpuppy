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

use crate::settings::{LearningStyle, Profile, Settings};
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

/// Appended to the prober and teacher prompts in the `visual` learning style.
const VISUAL_STYLE: &str = "\n\nStyle: explain with ASCII or Mermaid diagrams, tables and small worked examples first, then prose.";

pub struct CliPoolEngine {
    pool: Arc<AccountPool>,
    /// Slot pin: `<provider>:auto` prefers that provider, an account id pins that account.
    pin: String,
    /// The role's provider, model and effort; the pool stays inside its provider unless it allows fallback.
    profile: Option<Profile>,
    visual: bool,
    role: String,
}

fn provider_kind(p: &str) -> ProviderKind {
    match p {
        "codex" => ProviderKind::CodexCli,
        "agy" => ProviderKind::AgyCli,
        _ => ProviderKind::ClaudeCli,
    }
}

impl CliPoolEngine {
    pub fn new(pool: Arc<AccountPool>, pin: impl Into<String>) -> Self {
        Self {
            pool,
            pin: pin.into(),
            profile: None,
            visual: false,
            role: String::new(),
        }
    }

    /// The engine for `role`: its profile (if any) overrides the pin's provider, and the
    /// visual learning style tags the prober and teacher prompts.
    pub fn for_role(pool: Arc<AccountPool>, settings: &Settings, role: &str, pin: &str) -> Self {
        let mut e = Self::new(pool, pin);
        e.role = role.into();
        e.profile = settings.profiles.get(role).cloned();
        if let Some(p) = &e.profile {
            // An account pin of the profile's provider stays; anything else becomes `<provider>:auto`.
            if e.pin.split(':').next() != Some(p.provider.as_str()) {
                e.pin = format!("{}:auto", p.provider);
            }
        }
        e.visual = settings.learning_style == LearningStyle::Visual
            && matches!(role, "prober" | "teacher");
        e
    }

    fn chain(&self) -> Vec<ProviderKind> {
        use ProviderKind::*;
        match (&self.profile, self.pin.split(':').next()) {
            (Some(p), _) if !p.fallback => vec![provider_kind(&p.provider)],
            (_, Some("codex")) => vec![CodexCli, ClaudeCli, AgyCli],
            (_, Some("agy")) => vec![AgyCli, ClaudeCli, CodexCli],
            _ => vec![ClaudeCli, CodexCli, AgyCli],
        }
    }

    fn styled(&self, mut prompt: String) -> String {
        if self.visual {
            prompt.push_str(VISUAL_STYLE);
        }
        prompt
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
            engine.role = self.role.clone();
            engine.account = acct.id.0.clone();
            // A fallback provider cannot run another provider's model slug.
            if let Some(p) = self
                .profile
                .as_ref()
                .filter(|p| provider_kind(&p.provider) == acct.provider)
            {
                engine.model = Some(p.model.clone());
                engine.effort = p.effort.clone();
            }
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

/// Outcome of a role-slot test ping.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PingOutcome {
    pub ok: bool,
    pub latency_ms: u64,
    pub message: String,
}

/// How long a test ping may take before it counts as failed.
pub const PING_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(45);

/// A tiny turn through `engine`. Never errors: a bad model or a missing login is `ok: false`
/// with the reason in `message`.
pub async fn ping<E: LlmEngine>(
    engine: &E,
    model: &str,
    effort: Option<&str>,
    cancel: CancellationToken,
) -> PingOutcome {
    let req = TurnRequest {
        prompt: "Reply with the single word: pong".into(),
        model: Some(model.into()),
        effort: effort.map(Into::into),
        tools: vec![],
    };
    let (tx, mut rx) = mpsc::channel(STREAM_CAP);
    let drain = async { while rx.recv().await.is_some() {} };
    let started = Instant::now();
    let run = tokio::time::timeout(PING_TIMEOUT, engine.stream_turn(req, tx, cancel));
    let (res, ()) = tokio::join!(run, drain);
    let latency_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    match res {
        Ok(Ok(r)) if !r.text.trim().is_empty() => PingOutcome {
            ok: true,
            latency_ms,
            message: r.text.trim().chars().take(80).collect(),
        },
        Ok(Ok(_)) => PingOutcome {
            ok: false,
            latency_ms,
            message: "the model returned an empty reply".into(),
        },
        Ok(Err(e)) => PingOutcome {
            ok: false,
            latency_ms,
            message: e.to_string().chars().take(300).collect(),
        },
        Err(_) => PingOutcome {
            ok: false,
            latency_ms,
            message: format!("no reply within {}s", PING_TIMEOUT.as_secs()),
        },
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
        mut req: TurnRequest,
        event_tx: mpsc::Sender<StreamDelta>,
        cancel: CancellationToken,
    ) -> Result<TurnResponse, DriverError> {
        req.prompt = self.styled(req.prompt);
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

    fn settings_with(role: &str, p: Profile, style: LearningStyle) -> Settings {
        let mut s = Settings::default();
        s.profiles.insert(role.into(), p);
        s.learning_style = style;
        s
    }

    #[test]
    fn profile_routes_to_its_provider_without_fallback() {
        let pool = engine("claude:auto").pool;
        let p = Profile::new("codex", "gpt-6-sol", Some("high")).unwrap();
        let s = settings_with("prober", p, LearningStyle::Default);
        let e = CliPoolEngine::for_role(pool.clone(), &s, "prober", "claude:auto");
        assert_eq!(e.pick().unwrap().id.0, "codex:c");
        // The only codex account is out: no silent switch to claude.
        pool.report_auth_expired(&AccountId("codex:c".into()));
        assert!(e.pick().is_none());
        // Unprofiled roles keep the pin and the old chain.
        let e = CliPoolEngine::for_role(pool.clone(), &s, "teacher", "claude:auto");
        assert_eq!(e.pick().unwrap().id.0, "claude:a");
        let mut s = s;
        s.profiles.get_mut("prober").unwrap().fallback = true;
        let e = CliPoolEngine::for_role(pool, &s, "prober", "claude:auto");
        assert_eq!(e.pick().unwrap().id.0, "claude:a");
    }

    #[test]
    fn visual_style_tags_only_prober_and_teacher_prompts() {
        let pool = engine("claude:auto").pool;
        let p = Profile::new("claude", "m", None).unwrap();
        let tagged = |role, style| {
            let s = settings_with(role, p.clone(), style);
            CliPoolEngine::for_role(pool.clone(), &s, role, "claude:auto").styled("hi".into())
        };
        assert!(tagged("teacher", LearningStyle::Visual).contains("Mermaid"));
        assert!(tagged("prober", LearningStyle::Visual).contains("diagrams"));
        assert_eq!(tagged("teacher", LearningStyle::Default), "hi");
        assert_eq!(tagged("practice", LearningStyle::Visual), "hi");
    }

    #[tokio::test]
    async fn profile_model_and_effort_reach_the_cli() {
        let pool = engine("claude:auto").pool;
        let p = Profile::new("codex", "gpt-6-sol", Some("high")).unwrap();
        let s = settings_with("prober", p, LearningStyle::Default);
        let e = CliPoolEngine::for_role(pool, &s, "prober", "claude:auto");
        let got = e
            .with_account(&Seen::default(), |x| async move {
                Ok((x.kind, x.model, x.effort))
            })
            .await
            .unwrap();
        assert_eq!(
            got,
            (
                CliKind::Codex,
                Some("gpt-6-sol".into()),
                Some("high".into())
            )
        );
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
