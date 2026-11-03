//! `CliPoolEngine`: an `LlmEngine` that runs each call on a pooled CLI account and model and
//! fails over (model to model, account to account, provider to provider) by the failure
//! taxonomy in `docs/web/12-cli-isolation-and-failures.md`. When nothing can answer it returns a
//! structured [`AiFailure`] (docs/web/13-ai-connections.md).

use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use lp_cli_driver::accounts::{AccountHealth, AccountPool, AccountStatus, Pick, ProviderKind};
use lp_cli_driver::engine::{
    classify, CliKind, DriverError, FailureKind, LlmEngine, StreamDelta, StructuredOutput,
    StructuredRequest, SubprocessEngine, TurnRequest, TurnResponse,
};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;

mod ai;
pub use ai::{AiCode, AiFailure};

use crate::accounts::{how_to_connect, provider_name};
use crate::settings::{LearningStyle, Profile, Settings};
use std::sync::atomic::{AtomicBool, Ordering};

use parking_lot::Mutex;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Stream channel capacity and how much stdout tail is kept for failure classification.
const STREAM_CAP: usize = 512;
const TAIL: usize = 4096;
/// Upper bound on attempts of one call (every model of every account, with slack).
const MAX_ATTEMPTS: usize = 40;
/// How many times a call waits out an offline pause before it gives up with `OFFLINE`.
const OFFLINE_RETRIES: u32 = 4;

/// What one attempt has produced so far.
#[derive(Default)]
struct Seen {
    stdout: Mutex<String>,
    emitted: AtomicBool,
    cancel: CancellationToken,
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
    /// Longest single wait while the network is down (tests shrink it).
    offline_wait_cap: Duration,
}

fn provider_kind(p: &str) -> ProviderKind {
    match p {
        "codex" => ProviderKind::CodexCli,
        "agy" => ProviderKind::AgyCli,
        "kiro" => ProviderKind::KiroCli,
        "opencode" => ProviderKind::OpencodeCli,
        _ => ProviderKind::ClaudeCli,
    }
}

fn cli_kind(p: ProviderKind) -> CliKind {
    match p {
        ProviderKind::ClaudeCli => CliKind::Claude,
        ProviderKind::CodexCli => CliKind::Codex,
        ProviderKind::AgyCli => CliKind::Agy,
        ProviderKind::KiroCli => CliKind::Kiro,
        ProviderKind::OpencodeCli => CliKind::Opencode,
    }
}

fn is_free_provider(p: ProviderKind) -> bool {
    matches!(p, ProviderKind::KiroCli | ProviderKind::OpencodeCli)
}

fn now_ms() -> u64 {
    u64::try_from(crate::now_ms()).unwrap_or(0)
}

impl CliPoolEngine {
    pub fn new(pool: Arc<AccountPool>, pin: impl Into<String>) -> Self {
        Self {
            pool,
            pin: pin.into(),
            profile: None,
            visual: false,
            role: String::new(),
            offline_wait_cap: Duration::from_secs(120),
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
        let first = |p: ProviderKind| {
            let mut v = vec![p];
            v.extend(
                [ClaudeCli, CodexCli, AgyCli, KiroCli, OpencodeCli]
                    .into_iter()
                    .filter(|x| *x != p),
            );
            v
        };
        match (&self.profile, self.pin.split(':').next()) {
            (Some(p), _) if !p.fallback => vec![provider_kind(&p.provider)],
            // A free profile with fallback only falls back to the other free CLI, never to a paid slug.
            (Some(p), _) if is_free_provider(provider_kind(&p.provider)) => {
                let k = provider_kind(&p.provider);
                [k, if k == KiroCli { OpencodeCli } else { KiroCli }].to_vec()
            }
            (_, Some("codex")) => first(CodexCli),
            (_, Some("agy")) => first(AgyCli),
            (_, Some("kiro")) => first(KiroCli),
            (_, Some("opencode")) => first(OpencodeCli),
            _ => first(ClaudeCli),
        }
    }

    fn styled(&self, mut prompt: String) -> String {
        if self.visual {
            prompt.push_str(VISUAL_STYLE);
        }
        prompt
    }

    /// The account (and model, on multi-model CLIs) to run on next.
    fn pick(&self) -> Option<Pick> {
        let snap = self.pool.snapshot();
        let pinned = snap.iter().find(|a| {
            a.id.0 == self.pin
                && a.health == AccountHealth::Healthy
                && !a.disabled
                && !a.missing
                && a.models.is_empty()
        });
        if let Some(a) = pinned {
            return Some(Pick {
                account: a.id.clone(),
                model: None,
            });
        }
        let preferred = self.profile.as_ref().map(|p| p.model.as_str());
        self.pool
            .acquire_model(&self.chain(), preferred, Instant::now())
    }

    /// Waits `d` (capped); false when cancelled meanwhile.
    async fn pause(&self, d: Duration, cancel: &CancellationToken) -> bool {
        tokio::select! {
            () = tokio::time::sleep(d.min(self.offline_wait_cap)) => true,
            () = cancel.cancelled() => false,
        }
    }

    /// Runs `call` on successive accounts and models until one succeeds, a non-failover error
    /// occurs, or nothing is left (a structured [`AiFailure`]).
    async fn with_account<T, F, Fut>(&self, seen: &Seen, call: F) -> Result<T, DriverError>
    where
        F: Fn(SubprocessEngine) -> Fut,
        Fut: Future<Output = Result<T, DriverError>>,
    {
        let mut offline_waits = 0;
        let mut last: Option<(ProviderKind, Option<String>, FailureKind)> = None;
        for _ in 0..MAX_ATTEMPTS {
            if let Some(wait) = self.pool.offline_for(Instant::now()) {
                if offline_waits >= OFFLINE_RETRIES {
                    return Err(Self::offline_failure(wait).into_error());
                }
                offline_waits += 1;
                if !self.pause(wait, &seen.cancel).await {
                    return Err(DriverError::Cancelled);
                }
            }
            let Some(pick) = self.pick() else { break };
            let Some(acct) = self
                .pool
                .snapshot()
                .into_iter()
                .find(|a| a.id == pick.account)
            else {
                break;
            };
            let mut engine = SubprocessEngine::new(cli_kind(acct.provider))?;
            engine.config_dir = acct.config_dir.clone();
            engine.role = self.role.clone();
            engine.account = acct.id.0.clone();
            let usage = engine.usage.clone();
            // A fallback provider cannot run another provider's model slug.
            let profile = self
                .profile
                .as_ref()
                .filter(|p| provider_kind(&p.provider) == acct.provider);
            engine.model = pick
                .model
                .clone()
                .or_else(|| profile.map(|p| p.model.clone()));
            if let Some(p) = profile {
                engine.effort = p.effort.clone();
            }
            seen.stdout.lock().clear();
            seen.emitted.store(false, Ordering::Relaxed);
            let err = match call(engine).await {
                Ok(v) => {
                    let (i, o) = (
                        usage[0].load(Ordering::Relaxed),
                        usage[1].load(Ordering::Relaxed),
                    );
                    self.pool.report_success(&acct.id, i, o);
                    return Ok(v);
                }
                Err(e) => e,
            };
            let (text, kind) = failure_of(&err, &seen.stdout.lock());
            if matches!(kind, FailureKind::TimedOut | FailureKind::Other(_)) {
                return Err(err);
            }
            self.pool.report_failure(
                &acct.id,
                pick.model.as_deref(),
                &kind,
                &text,
                Instant::now(),
                now_ms(),
            );
            // Text already reached the learner; a retry would repeat it.
            if seen.emitted.load(Ordering::Relaxed) {
                return Err(err);
            }
            last = Some((acct.provider, pick.model, kind));
        }
        Err(self.unavailable(last).into_error())
    }

    fn offline_failure(wait: Duration) -> AiFailure {
        let mut f = AiFailure::new(
            AiCode::Offline,
            "You're offline, so no AI can answer. LearnPuppy retries on its own.",
        );
        f.retry_in_secs = Some(wait.as_secs().max(1));
        f
    }

    /// Why nothing could run: the soonest reset when every AI is limited, else the login or
    /// model problem, else "nothing connected".
    fn unavailable(&self, last: Option<(ProviderKind, Option<String>, FailureKind)>) -> AiFailure {
        let now = Instant::now();
        if let Some(wait) = self.pool.offline_for(now) {
            return Self::offline_failure(wait);
        }
        let chain = self.chain();
        let disabled: Vec<_> = self
            .pool
            .snapshot()
            .into_iter()
            .filter(|a| a.disabled)
            .map(|a| a.id)
            .collect();
        let rows: Vec<_> = self
            .pool
            .statuses(now, now_ms())
            .into_iter()
            .filter(|r| chain.contains(&r.provider) && !disabled.contains(&r.id))
            .collect();
        let soonest = rows
            .iter()
            .filter_map(|r| match r.status {
                AccountStatus::CoolingDown { until } => Some((until, r)),
                _ => None,
            })
            .min_by_key(|(u, _)| *u);
        if let Some((until, r)) = soonest {
            let provider = provider_name(r.provider);
            let mut f = AiFailure::new(
                AiCode::RateLimited,
                format!("Every AI you connected is at its usage limit; {provider} is back first."),
            );
            f.resets_at = Some(until);
            f.provider = Some(provider.into());
            f.model = r
                .models
                .iter()
                .filter_map(|m| match m.status {
                    AccountStatus::CoolingDown { until: u } => Some((u, &m.id)),
                    _ => None,
                })
                .min()
                .map(|(_, id)| id.clone());
            return f;
        }
        if let Some(r) = rows.iter().find(|r| r.status == AccountStatus::NoModel) {
            let provider = provider_name(r.provider);
            let mut f = AiFailure::new(
                AiCode::NoModel,
                format!(
                    "{provider} has no model that can run this. Pick another model in Settings."
                ),
            );
            f.provider = Some(provider.into());
            f.action = Some("Settings".into());
            return f;
        }
        let blocked = match last {
            Some((p, _, FailureKind::NotLoggedIn)) => Some(p),
            _ => None,
        }
        .or_else(|| {
            rows.iter()
                .find(|r| matches!(r.status, AccountStatus::NeedsLogin))
                .map(|r| r.provider)
        });
        let provider =
            blocked.unwrap_or_else(|| *chain.first().unwrap_or(&ProviderKind::ClaudeCli));
        let name = provider_name(provider);
        let mut f = AiFailure::new(
            AiCode::NotConnected,
            if blocked.is_some() {
                format!("{name} is signed out. Sign in again to keep learning.")
            } else {
                "No AI is connected yet. Connect one on the Accounts screen; kiro and opencode have free options.".to_owned()
            },
        );
        f.provider = Some(name.into());
        f.action = Some(how_to_connect(name).into());
        f
    }
}

/// The text a failure is classified on (error text plus the stdout tail) and its class.
fn failure_of(err: &DriverError, tail: &str) -> (String, FailureKind) {
    let msg = match err {
        DriverError::Exit { stderr, .. } => stderr.clone(),
        DriverError::Failed(m) | DriverError::Cli { message: m } => m.clone(),
        _ => return (err.to_string(), err.failure(now_ms())),
    };
    let text = if tail.is_empty() {
        msg
    } else {
        format!("{msg}\n{tail}")
    };
    let kind = classify(&text, now_ms());
    (text, kind)
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
        system: None,
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

#[async_trait]
impl LlmEngine for CliPoolEngine {
    async fn stream_turn(
        &self,
        mut req: TurnRequest,
        event_tx: mpsc::Sender<StreamDelta>,
        cancel: CancellationToken,
    ) -> Result<TurnResponse, DriverError> {
        req.prompt = self.styled(req.prompt);
        let seen = Seen {
            cancel: cancel.clone(),
            ..Seen::default()
        };
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
        let seen = Seen {
            cancel: cancel.clone(),
            ..Seen::default()
        };
        self.with_account(&seen, |e| {
            let (req, cancel) = (req.clone(), cancel.clone());
            async move { e.complete_structured::<T>(req, cancel).await }
        })
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_cli_driver::accounts::{AccountId, DiscoveredAccount, ModelInfo, Strategy};

    fn acct(id: &str, provider: ProviderKind) -> DiscoveredAccount {
        DiscoveredAccount {
            id: AccountId(id.into()),
            provider,
            email: None,
            account_uuid: None,
            config_dir: None,
            rate_limit_tier: None,
            auth_expired: false,
            models: vec![],
        }
    }

    fn with_models(mut a: DiscoveredAccount, ids: &[&str]) -> DiscoveredAccount {
        a.models = ids
            .iter()
            .map(|id| ModelInfo {
                id: (*id).into(),
                free: id.ends_with("-free") || id.contains("pickle"),
                credit_multiplier: None,
            })
            .collect();
        a
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

    /// An engine over opencode (three free models) and kiro (two models), pinned to opencode.
    fn free_engine() -> CliPoolEngine {
        let pool = AccountPool::new(
            vec![
                with_models(
                    acct("opencode:opencode", ProviderKind::OpencodeCli),
                    &["opencode/big-pickle", "opencode/a-free", "opencode/b-free"],
                ),
                with_models(
                    acct("kiro:k@x.com", ProviderKind::KiroCli),
                    &["auto", "qwen3-coder-next"],
                ),
            ],
            Strategy::Priority,
        );
        let mut e = CliPoolEngine::new(Arc::new(pool), "opencode:auto");
        e.offline_wait_cap = Duration::from_millis(5);
        e
    }

    fn exit(stderr: &str) -> DriverError {
        DriverError::Exit {
            code: Some(1),
            stderr: stderr.into(),
        }
    }

    /// Runs `script` entries in order as attempt results; records `(kind, model)` per attempt.
    async fn run_script(
        e: &CliPoolEngine,
        script: Vec<Result<&'static str, DriverError>>,
    ) -> (
        Result<&'static str, DriverError>,
        Vec<(CliKind, Option<String>)>,
    ) {
        let script = Mutex::new(
            script
                .into_iter()
                .collect::<std::collections::VecDeque<_>>(),
        );
        let calls = Mutex::new(Vec::new());
        let out = e
            .with_account(&Seen::default(), |x| {
                calls.lock().push((x.kind, x.model.clone()));
                let r = script.lock().pop_front().unwrap_or(Ok("end"));
                async move { r }
            })
            .await;
        (out, calls.into_inner())
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
        assert_eq!(e.pick().unwrap().account.0, "codex:c");
        // The only codex account is out: no silent switch to claude.
        pool.report_auth_expired(&AccountId("codex:c".into()));
        assert!(e.pick().is_none());
        // Unprofiled roles keep the pin and the old chain.
        let e = CliPoolEngine::for_role(pool.clone(), &s, "teacher", "claude:auto");
        assert_eq!(e.pick().unwrap().account.0, "claude:a");
        let mut s = s;
        s.profiles.get_mut("prober").unwrap().fallback = true;
        let e = CliPoolEngine::for_role(pool, &s, "prober", "claude:auto");
        assert_eq!(e.pick().unwrap().account.0, "claude:a");
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
    fn pin_selects_account_then_provider_then_falls_back() {
        let e = engine("claude:b");
        assert_eq!(e.pick().unwrap().account.0, "claude:b");
        let e = engine("codex:auto");
        assert_eq!(e.pick().unwrap().account.0, "codex:c");
        let e = engine("claude:auto");
        assert_eq!(e.pick().unwrap().account.0, "claude:a");
        e.pool.report_auth_expired(&AccountId("claude:a".into()));
        e.pool.report_auth_expired(&AccountId("claude:b".into()));
        assert_eq!(e.pick().unwrap().account.0, "codex:c");
    }

    #[tokio::test]
    async fn no_failover_after_text_was_emitted_but_the_limit_is_recorded() {
        let e = engine("claude:auto");
        let seen = Seen::default();
        let n = Mutex::new(0);
        let r = e
            .with_account(&seen, |_| {
                *n.lock() += 1;
                seen.emitted.store(true, Ordering::Relaxed);
                async { Err::<(), _>(exit("rate limit")) }
            })
            .await;
        assert!(r.is_err());
        assert_eq!(*n.lock(), 1);
        let st = e.pool.statuses(Instant::now(), now_ms());
        assert!(matches!(st[0].status, AccountStatus::CoolingDown { .. }));
    }

    #[tokio::test]
    async fn fails_over_on_rate_limit_and_stops_on_other_errors() {
        let e = engine("claude:auto");
        let (out, calls) = run_script(
            &e,
            vec![Err(exit("rate limit")), Err(exit("rate limit")), Ok("done")],
        )
        .await;
        assert_eq!(out.unwrap(), "done");
        assert_eq!(calls.len(), 3);
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
    async fn a_model_limit_with_reset_fails_over_to_the_next_free_model() {
        let e = free_engine();
        let (out, calls) = run_script(
            &e,
            vec![
                Err(exit("Error 429 too many requests, try again in 20 minutes")),
                Ok("ok"),
            ],
        )
        .await;
        assert_eq!(out.unwrap(), "ok");
        let models: Vec<_> = calls.iter().map(|c| c.1.clone().unwrap()).collect();
        assert_eq!(models[0], "opencode/big-pickle");
        assert_ne!(models[1], models[0]);
        assert!(models[1].starts_with("opencode/"), "{models:?}");
        // Only that model is cooling; the account is still ready.
        let st = e.pool.statuses(Instant::now(), now_ms());
        assert_eq!(st[0].status, AccountStatus::Ready);
        let m = &st[0].models[0];
        assert!(matches!(m.status, AccountStatus::CoolingDown { .. }));
        assert!(m.resets_at.unwrap() > now_ms() + 19 * 60_000);
    }

    #[tokio::test]
    async fn a_free_tier_quota_cools_opencode_and_moves_on_to_kiro() {
        let e = free_engine();
        let (out, calls) = run_script(
            &e,
            vec![
                Err(exit("Free usage limit reached. Try again in 5 hours")),
                Ok("ok"),
            ],
        )
        .await;
        assert_eq!(out.unwrap(), "ok");
        assert_eq!(calls[0].0, CliKind::Opencode);
        assert_eq!(calls[1].0, CliKind::Kiro);
        let st = e.pool.statuses(Instant::now(), now_ms());
        assert!(matches!(st[0].status, AccountStatus::CoolingDown { .. }));
    }

    #[tokio::test]
    async fn offline_pauses_and_retries_without_cooling_any_account() {
        let e = free_engine();
        let (out, calls) = run_script(
            &e,
            vec![Err(exit("connect ECONNREFUSED 1.2.3.4:443")), Ok("ok")],
        )
        .await;
        assert_eq!(out.unwrap(), "ok");
        // The retry runs the same model: nothing was benched.
        assert_eq!(calls[0], calls[1]);
        let st = e.pool.statuses(Instant::now(), now_ms());
        assert!(
            st.iter().all(|r| r.status == AccountStatus::Ready),
            "{st:?}"
        );
        assert!(e.pool.offline_for(Instant::now()).is_none());
    }

    #[tokio::test]
    async fn staying_offline_ends_in_an_offline_error_with_all_accounts_intact() {
        let e = free_engine();
        let script = (0..20)
            .map(|_| Err(exit("getaddrinfo ENOTFOUND api.x.com")))
            .collect();
        let (out, calls) = run_script(&e, script).await;
        let f = AiFailure::from_error(&out.unwrap_err()).unwrap();
        assert_eq!(f.code, AiCode::Offline);
        assert!(f.retry_in_secs.is_some());
        assert_eq!(calls.len(), (OFFLINE_RETRIES + 1) as usize);
        let st = e.pool.statuses(Instant::now(), now_ms());
        // Offline reads as `Offline`, never as a cooldown or logout.
        assert!(st.iter().all(|r| r.status == AccountStatus::Offline));
    }

    #[tokio::test]
    async fn not_logged_in_skips_that_account_and_names_the_login() {
        let e = free_engine();
        let (out, calls) = run_script(
            &e,
            vec![Err(exit("Not logged in · Please run /login")), Ok("ok")],
        )
        .await;
        assert_eq!(out.unwrap(), "ok");
        assert_eq!(calls[1].0, CliKind::Kiro);
        let st = e.pool.statuses(Instant::now(), now_ms());
        assert_eq!(st[0].status, AccountStatus::NeedsLogin);

        // Everything signed out: NOT_CONNECTED with the command to run.
        let (out, _) = run_script(&e, vec![Err(exit("Not logged in"))]).await;
        let f = AiFailure::from_error(&out.unwrap_err()).unwrap();
        assert_eq!(f.code, AiCode::NotConnected);
        assert_eq!(f.provider.as_deref(), Some("kiro"));
        assert_eq!(f.action.as_deref(), Some("kiro-cli login"));
    }

    #[tokio::test]
    async fn an_unknown_model_is_benched_and_a_sibling_serves() {
        let e = free_engine();
        let (out, calls) = run_script(
            &e,
            vec![
                Err(exit("ProviderModelNotFound: model not found")),
                Ok("ok"),
            ],
        )
        .await;
        assert_eq!(out.unwrap(), "ok");
        assert_ne!(calls[0].1, calls[1].1);
        assert_eq!(calls[1].0, CliKind::Opencode);
        let st = e.pool.statuses(Instant::now(), now_ms());
        assert_eq!(st[0].models[0].status, AccountStatus::NoModel);
    }

    #[tokio::test]
    async fn all_limited_returns_the_soonest_reset_and_provider() {
        let e = free_engine();
        let script = vec![
            Err(exit("Free usage limit reached. Try again in 5 hours")),
            Err(exit("monthly credits exhausted. Try again in 2 hours")),
        ];
        let (out, calls) = run_script(&e, script).await;
        assert_eq!(calls.len(), 2);
        let f = AiFailure::from_error(&out.unwrap_err()).unwrap();
        assert_eq!(f.code, AiCode::RateLimited);
        assert_eq!(f.provider.as_deref(), Some("kiro"), "{f:?}");
        let at = f.resets_at.unwrap();
        assert!(
            at > now_ms() + 90 * 60_000 && at < now_ms() + 150 * 60_000,
            "{at}"
        );
    }

    #[tokio::test]
    async fn reports_when_nothing_is_connected() {
        let e = CliPoolEngine::new(
            Arc::new(AccountPool::new(vec![], Strategy::Priority)),
            "claude:auto",
        );
        let err = e
            .with_account(&Seen::default(), |_| async { Ok(()) })
            .await
            .unwrap_err();
        let f = AiFailure::from_error(&err).unwrap();
        assert_eq!(f.code, AiCode::NotConnected);
        assert!(f.message.contains("Connect"));
    }

    #[test]
    fn free_profiles_with_fallback_never_reach_a_paid_provider() {
        let pool = engine("claude:auto").pool;
        let mut p = Profile::new("opencode", "opencode/big-pickle", None).unwrap();
        p.fallback = true;
        let s = settings_with("teacher", p, LearningStyle::Default);
        let e = CliPoolEngine::for_role(pool, &s, "teacher", "claude:auto");
        assert_eq!(
            e.chain(),
            vec![ProviderKind::OpencodeCli, ProviderKind::KiroCli]
        );
    }
}
