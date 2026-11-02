//! The chat engine behind `POST …/chats` and `POST …/chats/{id}/turn` (docs/web/03 §Turn state
//! machine): one active turn per chat, each turn a spawned task that runs the stateless agents
//! until the learner is up, streams deltas through the [`DeltaPump`], persists the session and
//! then announces `chat.rest`.
//!
//! - `idle → running → idle` on success; `cancel` keeps the partial text marked interrupted.
//! - Engine failures leave the chat in a retryable `error`; posting the same `clientId` again
//!   re-runs the turn from the saved transcript.
//! - Rate limits queue the turn (see [`gate`](super::gate)).
//! - A turn marker under `turns/` survives a crash or shutdown, so after a restart the chat opens
//!   as `error{interrupted}` with Resume.
//! - Workbench jobs run per workspace; their results wake idle chats (the wake pump).

use std::collections::HashMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lp_agent::loops::{Call, Effect, Kind, Msg, Session, TurnError};
use lp_agent::workbench::{HttpWeb, Pdf, PdftotextPdf, Web, Workbench};
use lp_cli_driver::accounts::AccountPool;
use lp_cli_driver::engine::{DriverError, LlmEngine, StreamDelta, TurnRequest};
use lp_core::plan::QuizInput;
use lp_core::ports::{
    ChatRecord, ChatStore, DeliveryStatus, JobRecord, JobStatus, JobStore, StoreError,
};
use lp_store::FsJobStore;
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tokio::sync::{mpsc, OwnedMutexGuard};
use tokio_util::sync::CancellationToken;

use super::answer::{render_answer, Answer, IDK};
use super::gate::{CliPermits, GateObserver, Gated, Permitted, TurnGate};
use super::jobs::{job_artifact, job_title, kind_name, ObservedJobs};
use super::meta::ChatMeta;
use super::script::FaultPlan;
use super::turn::{role, run_to_rest};
use super::view::pending_call;
use crate::engine::{AiCode, AiFailure};
use crate::events::{self as ev, DeltaKind, DeltaPump, Event, Scope};
use crate::workspace::WorkspaceHandle;

/// Engine for a role (`prober`, `teacher`, `practice`, `research`, …).
pub type Engines<E> = Arc<dyn Fn(&str) -> Arc<E> + Send + Sync>;

/// Continuation rounds (job results, steers, handoffs) one turn may run before it rests.
const MAX_ROUNDS: usize = 8;
/// Longest learner text accepted in one message.
pub const MAX_TEXT_CHARS: usize = 20_000;
/// Estimated transcript tokens above which the oldest part is summarized.
const COMPACT_TRIGGER_TOKENS: u64 = 60_000;
const EVENT_CAP: usize = 512;
const ERROR_CHARS: usize = 400;
const TITLE_CHARS: usize = 80;

pub struct RuntimeConfig {
    /// How long a rate-limited turn waits before the error is surfaced (docs: 60 s).
    pub queue_limit: Duration,
    pub web: Arc<dyn Web>,
    pub pdf: Arc<dyn Pdf>,
    /// Server time in epoch ms.
    pub clock: Arc<dyn Fn() -> i64 + Send + Sync>,
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            queue_limit: Duration::from_secs(60),
            web: Arc::new(HttpWeb::default()),
            pdf: Arc::new(PdftotextPdf),
            clock: Arc::new(crate::now_ms),
        }
    }
}

/// What a turn runs with: the workspace, the role engines from the current settings, the CLI
/// concurrency cap and the server shutdown token.
pub struct TurnCtx<E> {
    pub handle: Arc<WorkspaceHandle>,
    pub engines: Engines<E>,
    pub cap: usize,
    pub shutdown: CancellationToken,
}

impl<E> Clone for TurnCtx<E> {
    fn clone(&self) -> Self {
        Self {
            handle: self.handle.clone(),
            engines: self.engines.clone(),
            cap: self.cap,
            shutdown: self.shutdown.clone(),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TurnState {
    #[default]
    Idle,
    Running,
    Queued,
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnFailure {
    /// An `ErrorCode` wire value (`engine_unavailable`, `rate_limited`, `internal`, …) or
    /// `interrupted` for a turn a restart cut off.
    pub code: String,
    pub message: String,
    pub retryable: bool,
    pub retry_after_ms: Option<u64>,
    pub interrupted: bool,
    /// Set when no AI could answer: the structured reason the UI explains.
    pub ai: Option<AiFailure>,
}

/// The live turn state of one chat.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TurnStatus {
    pub state: TurnState,
    pub turn_id: Option<String>,
    /// The request that started the turn; post it again to retry an error.
    pub client_id: Option<String>,
    pub started_at: Option<i64>,
    /// While queued on a rate limit: when the next attempt starts (epoch ms).
    pub retry_at: Option<i64>,
    pub error: Option<TurnFailure>,
}

/// A learner's answer to the pending question.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnswerInput {
    /// Picked option keys (or [`IDK`]); several only for multi-select questions.
    pub choice: Vec<String>,
    pub text: String,
    pub reading_notes: String,
    /// 1..=5.
    pub confidence: Option<u8>,
    /// Practice problems: ask for the next hint rung instead of (or with) an attempt.
    pub hint: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TurnInput {
    User(String),
    Answer {
        call_id: String,
        answer: AnswerInput,
    },
    Steer(String),
    /// Re-run the interrupted or failed last step.
    Resume,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Accepted {
    /// A new turn started.
    Started(String),
    /// This `clientId` was applied already; its turn.
    Duplicate(String),
    /// A steer for the running turn; delivered between its model steps.
    SteerQueued(String),
    /// Saved without running the model (a steer while a question waits).
    Recorded(String),
}

impl Accepted {
    pub fn turn_id(&self) -> &str {
        match self {
            Accepted::Started(t)
            | Accepted::Duplicate(t)
            | Accepted::SteerQueued(t)
            | Accepted::Recorded(t) => t,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum Rejected {
    #[error("no chat {0}")]
    NotFound(String),
    #[error("a turn is already running")]
    InProgress { turn_id: Option<String> },
    #[error("that question is no longer waiting for an answer")]
    StaleQuestion,
    #[error("answer the pending question first")]
    PendingQuestion { call_id: String },
    #[error("nothing to resume: the chat is waiting for the learner")]
    NothingToResume,
    #[error("{field}: {message}")]
    Invalid {
        field: &'static str,
        message: String,
    },
    #[error("{0}")]
    Conflict(String),
    #[error(transparent)]
    Store(#[from] StoreError),
}

fn invalid(field: &'static str, message: impl Into<String>) -> Rejected {
    Rejected::Invalid {
        field,
        message: message.into(),
    }
}

/// Kind of a new chat.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NewKind {
    Teach,
    Practice,
}

/// A graph proposition a new chat starts from.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Seed {
    pub id: String,
    pub name: String,
    pub statement: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct NewChat {
    pub kind: NewKind,
    pub topic: String,
    pub seeds: Vec<Seed>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Created {
    pub chat_id: String,
    pub turn_id: String,
}

/// A workbench job as the UI lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobView {
    pub id: String,
    pub chat_id: String,
    pub kind: &'static str,
    pub status: JobStatus,
    pub title: String,
    pub artifact_id: Option<String>,
    pub error: Option<String>,
    /// PDF jobs: pages read so far and the document's page count.
    pub pages_read: Option<u64>,
    pub total_pages: Option<u64>,
    pub created_at: i64,
}

impl From<&JobRecord> for JobView {
    fn from(j: &JobRecord) -> Self {
        Self {
            id: j.id.to_string(),
            chat_id: j.chat_id.to_string(),
            kind: kind_name(j.kind),
            status: j.status,
            title: job_title(j),
            artifact_id: job_artifact(j),
            error: j
                .result
                .as_ref()
                .and_then(|r| r.get("error"))
                .and_then(Value::as_str)
                .map(str::to_owned),
            pages_read: progress_field(j, "pagesRead"),
            total_pages: progress_field(j, "totalPages"),
            created_at: j.created_at,
        }
    }
}

fn progress_field(j: &JobRecord, key: &str) -> Option<u64> {
    j.result.as_ref()?.get("progress")?.get(key)?.as_u64()
}

fn job_info(j: &JobRecord) -> ev::JobInfo {
    let v = JobView::from(j);
    ev::JobInfo {
        id: v.id,
        kind: v.kind.to_owned(),
        status: match v.status {
            JobStatus::Running => "running",
            JobStatus::Done => "done",
            JobStatus::Failed => "failed",
        }
        .to_owned(),
        title: v.title,
        artifact_id: v.artifact_id,
    }
}

// ---------------------------------------------------------------- per-chat slot

#[derive(Default)]
struct Live {
    state: TurnState,
    turn_id: Option<String>,
    client_id: Option<String>,
    started_at: Option<i64>,
    retry_at: Option<i64>,
    error: Option<TurnFailure>,
    cancel: Option<CancellationToken>,
    user_cancel: bool,
    steers: Vec<(String, String)>,
}

#[derive(Default)]
struct Slot {
    /// Held for the whole turn: one active turn per chat.
    lock: Arc<tokio::sync::Mutex<()>>,
    live: Mutex<Live>,
}

impl Slot {
    fn status(&self) -> TurnStatus {
        let l = self.live.lock();
        TurnStatus {
            state: l.state,
            turn_id: l.turn_id.clone(),
            client_id: l.client_id.clone(),
            started_at: l.started_at,
            retry_at: l.retry_at,
            error: l.error.clone(),
        }
    }

    fn start(&self, turn_id: &str, client_id: &str, started_at: i64) -> CancellationToken {
        let cancel = CancellationToken::new();
        let mut l = self.live.lock();
        *l = Live {
            state: TurnState::Running,
            turn_id: Some(turn_id.to_owned()),
            client_id: Some(client_id.to_owned()),
            started_at: Some(started_at),
            cancel: Some(cancel.clone()),
            steers: std::mem::take(&mut l.steers),
            ..Live::default()
        };
        cancel
    }

    fn finish(&self, failure: Option<TurnFailure>) {
        let mut l = self.live.lock();
        l.cancel = None;
        l.retry_at = None;
        l.user_cancel = false;
        match failure {
            Some(f) => {
                l.state = TurnState::Error;
                l.error = Some(f);
            }
            None => {
                l.state = TurnState::Idle;
                l.error = None;
                l.started_at = None;
            }
        }
    }

    fn take_steers(&self) -> Vec<(String, String)> {
        std::mem::take(&mut self.live.lock().steers)
    }
}

struct SlotObserver {
    slot: Arc<Slot>,
    pump: Arc<DeltaPump>,
    ws: String,
    chat: String,
}

impl SlotObserver {
    fn set(&self, state: TurnState, retry_at: Option<i64>) {
        {
            let mut l = self.slot.live.lock();
            if l.state == state && l.retry_at == retry_at {
                return;
            }
            l.state = state;
            l.retry_at = retry_at;
        }
        self.pump.publish(
            Scope::Workspace(self.ws.clone()),
            Event::ChatUpdated {
                chat_id: self.chat.clone(),
            },
        );
    }
}

impl GateObserver for SlotObserver {
    fn queued(&self, retry_at: Option<i64>) {
        self.set(TurnState::Queued, retry_at);
    }
    fn running(&self) {
        self.set(TurnState::Running, None);
    }
    fn guidance(&self) -> String {
        self.slot
            .live
            .lock()
            .steers
            .iter()
            .map(|(id, text)| format!("\n[steer {id}] {text}"))
            .collect()
    }
}

// ---------------------------------------------------------------- turn markers

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Marker {
    turn_id: String,
    client_id: String,
    started_at: i64,
}

fn marker_path(root: &Path, chat: &str) -> PathBuf {
    root.join("turns").join(format!("{chat}.json"))
}

/// A torn or zero-byte marker (left by a crash mid-write) is treated as "no marker", never as an
/// error: the chat's own record is the source of truth, so losing just the interrupted-turn
/// indicator is safe.
fn read_marker(root: &Path, chat: &str) -> Option<Marker> {
    let bytes = match std::fs::read(marker_path(root, chat)) {
        Ok(b) if !b.is_empty() => b,
        Ok(_) => return None,
        Err(_) => return None,
    };
    match serde_json::from_slice(&bytes) {
        Ok(m) => Some(m),
        Err(e) => {
            tracing::warn!(chat, "turn marker unreadable, treating as none: {e}");
            None
        }
    }
}

/// Temp file + fsync + rename + directory fsync, so a crash mid-write never leaves a torn marker
/// where the previous, complete one used to be.
fn write_marker(root: &Path, chat: &str, m: &Marker) -> std::io::Result<()> {
    let path = marker_path(root, chat);
    let dir = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "marker path has no parent",
        )
    })?;
    std::fs::create_dir_all(dir)?;
    let tmp = path.with_extension(format!("json.tmp.{}", std::process::id()));
    let mut f = std::fs::File::create(&tmp)?;
    f.write_all(&serde_json::to_vec(m).map_err(std::io::Error::other)?)?;
    f.sync_all()?;
    drop(f);
    std::fs::rename(&tmp, &path)?;
    std::fs::File::open(dir)?.sync_all()
}

fn remove_marker(root: &Path, chat: &str) {
    match std::fs::remove_file(marker_path(root, chat)) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
        Err(e) => tracing::warn!(chat, "turn marker not removed: {e}"),
    }
}

// ---------------------------------------------------------------- records

fn session_from(rec: &ChatRecord) -> Result<Session, Rejected> {
    serde_json::from_value(rec.body.clone()).map_err(|e| {
        Rejected::Store(StoreError::Backend(format!(
            "unreadable chat {}: {e}",
            rec.id
        )))
    })
}

fn load(chats: &dyn ChatStore, chat: &str) -> Result<(ChatRecord, Session, ChatMeta), Rejected> {
    let rec = chats.load_chat(chat).map_err(|e| match e {
        StoreError::NotFound(_) => Rejected::NotFound(chat.to_owned()),
        other => Rejected::Store(other),
    })?;
    let s = session_from(&rec)?;
    let meta = ChatMeta::of(&rec.body);
    Ok((rec, s, meta))
}

/// Saves `s` with `meta` over `rec`; returns the new view version.
fn save(
    chats: &dyn ChatStore,
    rec: &mut ChatRecord,
    s: &Session,
    meta: &mut ChatMeta,
    now: i64,
) -> Result<u64, StoreError> {
    meta.stamp(s, now);
    rec.body = meta
        .body(s)
        .map_err(|e| StoreError::Backend(e.to_string()))?;
    rec.updated_at = now.max(rec.updated_at.saturating_add(1));
    chats.save_chat(rec)?;
    Ok(u64::try_from(rec.updated_at).unwrap_or(0))
}

fn new_id(prefix: &str) -> String {
    use rand::Rng;
    let n: u64 = rand::thread_rng().gen();
    format!("{prefix}_{:012x}", n & 0xffff_ffff_ffff)
}

/// The transcript ends where the model has to speak next.
fn awaiting_model(s: &Session) -> bool {
    !s.pending_question()
        && matches!(
            s.messages.last(),
            Some(Msg::User(_) | Msg::Answer { .. } | Msg::Steer { .. } | Msg::ToolResult { .. })
        )
}

fn capped(s: &str, n: usize) -> String {
    let s = lp_core::redact::redact(s.trim());
    if s.chars().count() <= n {
        return s;
    }
    let cut: String = s.chars().take(n - 1).collect();
    format!("{cut}…")
}

fn check_text(field: &'static str, text: &str) -> Result<String, Rejected> {
    let t = text.trim();
    if t.is_empty() {
        return Err(invalid(field, "must not be empty"));
    }
    if t.chars().count() > MAX_TEXT_CHARS {
        return Err(invalid(
            field,
            format!("must be at most {MAX_TEXT_CHARS} characters"),
        ));
    }
    Ok(t.to_owned())
}

/// Validates `a` against the pending call and renders it the way the model reads answers.
fn render_for(call: &Call, a: &AnswerInput) -> Result<String, Rejected> {
    let confidence = a.confidence.unwrap_or(3);
    if !(1..=5).contains(&confidence) {
        return Err(invalid("answer.confidence", "must be 1..=5"));
    }
    if a.text.chars().count() > MAX_TEXT_CHARS || a.reading_notes.chars().count() > MAX_TEXT_CHARS {
        return Err(invalid(
            "answer.text",
            format!("must be at most {MAX_TEXT_CHARS} characters"),
        ));
    }
    let text = a.text.trim();
    let options: Option<(Vec<String>, bool)> = match call.name.as_str() {
        "probe" | "quiz" => serde_json::from_value::<QuizInput>(call.args.clone())
            .ok()
            .map(|q| {
                (
                    q.options.into_iter().map(|o| o.value).collect(),
                    q.multi_select,
                )
            }),
        "ask_user_question" => call
            .args
            .get("options")
            .and_then(|o| serde_json::from_value::<Vec<lp_core::plan::QuizOption>>(o.clone()).ok())
            .filter(|o| !o.is_empty())
            .map(|o| (o.into_iter().map(|o| o.value).collect(), false)),
        _ => None,
    };
    let choice: Vec<&str> = a
        .choice
        .iter()
        .map(|c| c.trim())
        .filter(|c| !c.is_empty())
        .collect();
    if let Some((values, multi)) = &options {
        for c in &choice {
            if *c != IDK && !values.iter().any(|v| v == c) {
                return Err(invalid("answer.choice", format!("{c:?} is not an option")));
            }
        }
        if choice.len() > 1 && (!multi || choice.contains(&IDK)) {
            return Err(invalid("answer.choice", "pick one option"));
        }
    } else if !choice.is_empty() && choice != [IDK] {
        return Err(invalid("answer.choice", "this question has no options"));
    }
    let free_ok = call.name != "quiz";
    let hint = a.hint && call.name == "problem";
    if choice.is_empty() && (text.is_empty() || !free_ok) && !hint {
        return Err(if free_ok {
            invalid("answer.text", "must not be empty")
        } else {
            invalid("answer.choice", "pick an option")
        });
    }
    let thinking = match (hint, text.is_empty()) {
        (true, true) => "(asking for a hint)".to_owned(),
        (true, false) => format!("{text} (asking for a hint)"),
        _ => text.to_owned(),
    };
    Ok(render_answer(&Answer {
        value: choice.join(","),
        confidence,
        reading_thinking: a.reading_notes.trim().to_owned(),
        thinking,
    }))
}

/// Rough token estimate of the whole transcript.
fn estimate_tokens(s: &Session) -> u64 {
    s.messages
        .iter()
        .map(|m| serde_json::to_string(m).map_or(0, |t| t.chars().count() as u64 / 4 + 1))
        .sum()
}

#[derive(Debug, thiserror::Error)]
enum LoopError {
    #[error(transparent)]
    Turn(#[from] TurnError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl LoopError {
    fn cancelled(&self) -> bool {
        matches!(
            self,
            LoopError::Turn(TurnError::Driver(DriverError::Cancelled))
        )
    }
}

fn failure_of(e: &LoopError, gate: &TurnGate) -> TurnFailure {
    let fail = |code: &str, message: String| TurnFailure {
        code: code.to_owned(),
        message: capped(&message, ERROR_CHARS),
        retryable: true,
        retry_after_ms: None,
        interrupted: false,
        ai: None,
    };
    let from_ai = |f: AiFailure| {
        let mut t = fail(f.code.as_str(), f.message.clone());
        t.retry_after_ms = f
            .resets_at
            .map(|r| r.saturating_sub(u64::try_from(crate::now_ms()).unwrap_or(0)));
        t.ai = Some(f);
        t
    };
    if let Some(ms) = *gate.surfaced_retry_after.lock() {
        let mut f = fail(
            "rate_limited",
            format!(
                "Every model account is rate limited; try again in about {} s.",
                ms.div_ceil(1000)
            ),
        );
        f.retry_after_ms = Some(ms);
        return f;
    }
    match e {
        LoopError::Turn(TurnError::Driver(DriverError::FauxExhausted)) => fail(
            "engine_unavailable",
            "The scripted engine has no response left.".into(),
        ),
        LoopError::Turn(TurnError::Driver(d @ DriverError::Timeout(_))) => {
            from_ai(AiFailure::of_driver(d, 0))
        }
        LoopError::Turn(TurnError::NoResponse) => from_ai(AiFailure::new(
            AiCode::NoResponse,
            "The tutor had nothing to say. Try again.",
        )),
        LoopError::Turn(TurnError::Driver(d)) => match AiFailure::from_error(d) {
            Some(f) => from_ai(f),
            None => fail("engine_unavailable", format!("The model call failed: {d}")),
        },
        LoopError::Turn(t) => fail("internal", format!("The turn failed: {t}")),
        LoopError::Store(StoreError::Backend(m))
            if m.to_ascii_lowercase().contains("no space left") =>
        {
            fail("storage_full", m.clone())
        }
        LoopError::Store(s) => fail("internal", format!("Saving the chat failed: {s}")),
    }
}

fn interrupted() -> TurnFailure {
    TurnFailure {
        code: "interrupted".into(),
        message: "The server stopped during this turn. Resume to continue.".into(),
        retryable: true,
        retry_after_ms: None,
        interrupted: true,
        ai: None,
    }
}

/// Runs `f` on the session off the async runtime.
async fn on_session<T: Send + 'static>(
    s: &mut Session,
    f: impl FnOnce(&mut Session) -> T + Send + 'static,
) -> T {
    let mut taken = std::mem::replace(s, Session::new("", Kind::Teach));
    let (back, out) = match tokio::task::spawn_blocking(move || {
        let out = f(&mut taken);
        (taken, out)
    })
    .await
    {
        Ok(v) => v,
        Err(e) => std::panic::resume_unwind(e.into_panic()),
    };
    *s = back;
    out
}

async fn blocking<T: Send + 'static>(
    f: impl FnOnce() -> Result<T, Rejected> + Send + 'static,
) -> Result<T, Rejected> {
    match tokio::task::spawn_blocking(f).await {
        Ok(r) => r,
        Err(e) => std::panic::resume_unwind(e.into_panic()),
    }
}

// ---------------------------------------------------------------- workbench per workspace

type Jobs = ObservedJobs<FsJobStore>;

struct Bench<E: LlmEngine + 'static> {
    wb: Arc<Workbench<Permitted<E>, Jobs>>,
    jobs: Arc<Jobs>,
    recovered: tokio::sync::OnceCell<()>,
}

// ---------------------------------------------------------------- the runtime

pub struct ChatRuntime<E: LlmEngine + 'static> {
    pump: Arc<DeltaPump>,
    pool: Arc<AccountPool>,
    permits: Arc<CliPermits>,
    faults: Arc<FaultPlan>,
    config: RuntimeConfig,
    slots: Mutex<HashMap<(String, String), Arc<Slot>>>,
    benches: Mutex<HashMap<String, Arc<Bench<E>>>>,
    booted: std::sync::atomic::AtomicBool,
    script: tokio::sync::OnceCell<()>,
    active_jobs: Mutex<HashMap<(String, String), crate::workspace::BusyGuard>>,
}

impl<E: LlmEngine + 'static> ChatRuntime<E> {
    pub fn new(pump: Arc<DeltaPump>, pool: Arc<AccountPool>, config: RuntimeConfig) -> Arc<Self> {
        Arc::new(Self {
            pump,
            pool,
            permits: Arc::new(CliPermits::new(usize::from(
                crate::settings::DEFAULT_CONCURRENCY,
            ))),
            faults: Arc::new(FaultPlan::default()),
            config,
            slots: Mutex::default(),
            benches: Mutex::default(),
            booted: std::sync::atomic::AtomicBool::new(false),
            script: tokio::sync::OnceCell::new(),
            active_jobs: Mutex::default(),
        })
    }

    pub fn start_boot(&self) -> bool {
        !self.booted.swap(true, std::sync::atomic::Ordering::AcqRel)
    }

    /// Optional offline scenario for a running faux server; loaded once before its first command.
    pub async fn load_script(
        &self,
        faux: &lp_cli_driver::engine::FauxEngine,
        path: PathBuf,
    ) -> Result<(), Rejected> {
        self.script
            .get_or_try_init(|| async {
                let script = blocking(move || {
                    let text = std::fs::read_to_string(path)
                        .map_err(|e| invalid("script", e.to_string()))?;
                    super::Script::parse(&text).map_err(|e| invalid("script", e.to_string()))
                })
                .await?;
                script.apply(faux, &self.faults);
                Ok(())
            })
            .await
            .copied()
    }

    /// Scripted engine failures (faux scenarios and tests).
    pub fn faults(&self) -> &Arc<FaultPlan> {
        &self.faults
    }

    /// The process-wide CLI subprocess cap.
    pub fn permits(&self) -> &Arc<CliPermits> {
        &self.permits
    }

    fn now(&self) -> i64 {
        (self.config.clock)()
    }

    fn slot(&self, h: &WorkspaceHandle, chat: &str) -> Arc<Slot> {
        let ws = h.id();
        let mut slots = self.slots.lock();
        // Forget idle slots nobody holds, so the map tracks live chats only.
        slots.retain(|k, s| {
            (k.0 == ws && k.1 == chat)
                || s.lock.try_lock().is_err()
                || Arc::strong_count(s) > 1
                || s.live.lock().state != TurnState::Idle
        });
        slots
            .entry((ws.to_owned(), chat.to_owned()))
            .or_insert_with(|| {
                Arc::new(Slot {
                    lock: h.chat_lock(chat),
                    ..Slot::default()
                })
            })
            .clone()
    }

    fn publish(&self, ws: &str, event: Event) {
        self.pump.publish(Scope::Workspace(ws.to_owned()), event);
    }

    /// The live turn state of `chat`. Reads the turn marker: call off the async runtime.
    pub fn status(&self, h: &WorkspaceHandle, chat: &str) -> TurnStatus {
        let live = self
            .slots
            .lock()
            .get(&(h.id().to_owned(), chat.to_owned()))
            .map(|s| s.status());
        match live {
            Some(st) if st.state != TurnState::Idle => st,
            _ => match read_marker(h.root(), chat) {
                Some(m) => TurnStatus {
                    state: TurnState::Error,
                    turn_id: Some(m.turn_id),
                    client_id: Some(m.client_id),
                    started_at: Some(m.started_at),
                    retry_at: None,
                    error: Some(interrupted()),
                },
                None => TurnStatus::default(),
            },
        }
    }

    /// Serializes chat housekeeping with turns so a rename or delete cannot be overwritten.
    pub fn edit_guard(
        &self,
        h: &WorkspaceHandle,
        chat: &str,
    ) -> Result<OwnedMutexGuard<()>, Rejected> {
        let slot = self.slot(h, chat);
        slot.lock
            .clone()
            .try_lock_owned()
            .map_err(|_| Rejected::InProgress {
                turn_id: slot.live.lock().turn_id.clone(),
            })
    }

    /// Whether a turn of `chat` is running or queued in this process.
    pub fn busy(&self, ws: &str, chat: &str) -> bool {
        self.slots
            .lock()
            .get(&(ws.to_owned(), chat.to_owned()))
            .is_some_and(|s| {
                s.lock.try_lock().is_err()
                    || matches!(s.live.lock().state, TurnState::Running | TurnState::Queued)
            })
    }

    /// The workspace's workbench, created and crash-recovered on first use.
    async fn bench(self: &Arc<Self>, ctx: &TurnCtx<E>) -> Result<Arc<Bench<E>>, Rejected> {
        let h = &ctx.handle;
        let bench = {
            let mut benches = self.benches.lock();
            if let Some(b) = benches.get(h.id()) {
                b.clone()
            } else {
                let weak = Arc::downgrade(self);
                let notify_ctx = ctx.clone();
                let jobs = Arc::new(ObservedJobs::new(
                    h.stores().jobs.clone(),
                    Arc::new(move |job| {
                        if let Some(runtime) = weak.upgrade() {
                            runtime.job_updated(&notify_ctx, job);
                        }
                    }),
                ));
                let (engines, permits) = (ctx.engines.clone(), self.permits.clone());
                let workspace = h.id().to_owned();
                let wb = Workbench::routed(
                    move |role| {
                        Arc::new(Permitted {
                            workspace: workspace.clone(),
                            inner: engines(role),
                            permits: permits.clone(),
                        })
                    },
                    jobs.clone(),
                    self.config.web.clone(),
                    self.config.pdf.clone(),
                    h.root().join("artifacts"),
                );
                let b = Arc::new(Bench {
                    wb,
                    jobs,
                    recovered: tokio::sync::OnceCell::new(),
                });
                benches.insert(h.id().to_owned(), b.clone());
                self.spawn_watch(ctx.clone());
                b
            }
        };
        bench
            .recovered
            .get_or_try_init(|| {
                let (wb, ws) = (bench.wb.clone(), h.id().to_owned());
                blocking(move || {
                    let n = wb.recover()?;
                    if n > 0 {
                        tracing::info!(workspace = ws, jobs = n, "recovered interrupted jobs");
                    }
                    Ok(())
                })
            })
            .await?;
        Ok(bench)
    }

    /// Whether `ws`'s jobs were already crash-recovered by this process.
    pub fn recovered(&self, ws: &str) -> bool {
        self.benches
            .lock()
            .get(ws)
            .is_some_and(|b| b.recovered.initialized())
    }

    /// Makes sure jobs left running by a previous server are marked interrupted.
    pub async fn ensure(self: &Arc<Self>, ctx: &TurnCtx<E>) -> Result<(), Rejected> {
        self.bench(ctx).await.map(|_| ())
    }

    fn job_updated(self: Arc<Self>, ctx: &TurnCtx<E>, job: JobRecord) {
        let key = (ctx.handle.id().to_owned(), job.id.to_string());
        if job.status == JobStatus::Running {
            self.active_jobs
                .lock()
                .entry(key)
                .or_insert_with(|| ctx.handle.busy());
        } else {
            self.active_jobs.lock().remove(&key);
        }
        self.publish(
            ctx.handle.id(),
            Event::JobUpdated {
                job: job_info(&job),
            },
        );
        if job.status == JobStatus::Done && job.delivery == DeliveryStatus::Pending {
            tokio::spawn(self.wake(ctx.clone(), job.chat_id.to_string()));
        }
    }

    /// Stop jobs and release the workbench when its workspace closes or the server shuts down.
    fn spawn_watch(self: &Arc<Self>, ctx: TurnCtx<E>) {
        let runtime = self.clone();
        tokio::spawn(async move {
            let workspace_cancel = ctx.handle.cancel_token();
            tokio::select! {
                () = ctx.shutdown.cancelled() => {},
                () = workspace_cancel.cancelled() => {},
            }
            let bench = runtime.benches.lock().remove(ctx.handle.id());
            if let Some(bench) = bench {
                let jobs = ctx.handle.stores().jobs.clone();
                let _ = blocking(move || {
                    for job in jobs.interrupted_jobs()? {
                        bench.wb.abort(job.id.as_str());
                    }
                    Ok(())
                })
                .await;
            }
        });
    }

    /// The wake pump: delivers finished jobs to an idle chat and runs the continuation turn.
    async fn wake(self: Arc<Self>, ctx: TurnCtx<E>, chat: String) {
        let h = ctx.handle.clone();
        let slot = self.slot(&h, &chat);
        let guard = slot.lock.clone().lock_owned().await;
        let Ok(bench) = self.bench(&ctx).await else {
            return;
        };
        let chats = h.stores().chats.clone();
        let id = chat.clone();
        let Ok((rec, mut s, meta)) = blocking(move || load(chats.as_ref(), &id)).await else {
            return;
        };
        let wb = bench.wb.clone();
        let woke = on_session(&mut s, move |s| wb.deliver(s)).await;
        match woke {
            Ok(true) => {}
            Ok(false) => return,
            Err(e) => {
                tracing::warn!(chat, "job results not delivered: {e}");
                return;
            }
        }
        let turn_id = new_id("turn");
        let client = format!("wake:{turn_id}");
        self.begin(ctx, chat, slot, guard, rec, s, meta, turn_id, client, true)
            .await;
    }

    /// Creates a chat and starts its first turn.
    pub async fn create_chat(
        self: &Arc<Self>,
        ctx: &TurnCtx<E>,
        new: NewChat,
    ) -> Result<Created, Rejected> {
        let topic = new.topic.trim().to_owned();
        if topic.chars().count() > MAX_TEXT_CHARS {
            return Err(invalid(
                "topic",
                format!("must be at most {MAX_TEXT_CHARS} characters"),
            ));
        }
        if new.kind == NewKind::Teach && topic.is_empty() {
            return Err(invalid("topic", "must not be empty"));
        }
        if new.kind == NewKind::Practice && topic.is_empty() && new.seeds.is_empty() {
            return Err(invalid("seedNodeIds", "pick propositions or give a topic"));
        }
        let chat = new_id("chat");
        let kind = match new.kind {
            NewKind::Teach => Kind::Teach,
            NewKind::Practice => Kind::Practice,
        };
        let mut s = Session::new(chat.clone(), kind);
        let mut first = match (new.kind, topic.is_empty()) {
            (NewKind::Practice, true) => "Practice these propositions with me.".to_owned(),
            (NewKind::Practice, false) => format!("Practice with me: {topic}"),
            (NewKind::Teach, _) => topic.clone(),
        };
        if !new.seeds.is_empty() {
            first.push_str("\n\nFrom my knowledge graph:");
            for n in &new.seeds {
                first.push_str(&format!("\n- {} ({}): {}", n.name, n.id, n.statement));
            }
        }
        s.push_user(first);
        let title = {
            let base = if topic.is_empty() {
                new.seeds
                    .first()
                    .map_or_else(String::new, |n| format!("Practice: {}", n.name))
            } else {
                topic.lines().next().unwrap_or_default().trim().to_owned()
            };
            if base.chars().count() <= TITLE_CHARS {
                base
            } else {
                let cut: String = base.chars().take(TITLE_CHARS - 1).collect();
                format!("{}…", cut.trim_end())
            }
        };
        let now = self.now();
        let mut rec = ChatRecord {
            id: chat.as_str().into(),
            title,
            created_at: now,
            updated_at: now - 1,
            body: Value::Null,
            turns: vec![],
            extractions: vec![],
        };
        self.permits.resize(ctx.cap);
        let h = ctx.handle.clone();
        let slot = self.slot(&h, &chat);
        let guard = slot
            .lock
            .clone()
            .try_lock_owned()
            .map_err(|_| Rejected::InProgress { turn_id: None })?;
        let turn_id = new_id("turn");
        let client = format!("create:{chat}");
        let mut meta = ChatMeta::default();
        meta.remember(&client, &turn_id);
        let (chats, sess) = (h.stores().chats.clone(), s.clone());
        let rec = blocking(move || {
            save(chats.as_ref(), &mut rec, &sess, &mut meta, now)?;
            Ok(rec)
        })
        .await?;
        h.emit("chat.created", json!({ "chatId": chat }));
        let meta = ChatMeta::of(&rec.body);
        self.begin(
            ctx.clone(),
            chat.clone(),
            slot,
            guard,
            rec,
            s,
            meta,
            turn_id.clone(),
            client,
            false,
        )
        .await;
        Ok(Created {
            chat_id: chat,
            turn_id,
        })
    }

    /// `POST …/chats/{id}/turn`.
    pub async fn post_turn(
        self: &Arc<Self>,
        ctx: &TurnCtx<E>,
        chat: &str,
        client_id: &str,
        input: TurnInput,
    ) -> Result<Accepted, Rejected> {
        let client_id = client_id.trim().to_owned();
        if client_id.is_empty() || client_id.len() > 128 {
            return Err(invalid("clientId", "must be 1..=128 characters"));
        }
        let input = match input {
            TurnInput::User(t) => TurnInput::User(check_text("text", &t)?),
            TurnInput::Steer(t) => TurnInput::Steer(check_text("text", &t)?),
            other => other,
        };
        self.permits.resize(ctx.cap);
        let h = ctx.handle.clone();
        let slot = self.slot(&h, chat);
        let guard = match slot.lock.clone().try_lock_owned() {
            Ok(g) => g,
            Err(_) => {
                let mut l = slot.live.lock();
                let running = matches!(l.state, TurnState::Running | TurnState::Queued);
                let turn = l.turn_id.clone();
                if running && l.client_id.as_deref() == Some(client_id.as_str()) {
                    if let Some(t) = turn {
                        return Ok(Accepted::Duplicate(t));
                    }
                }
                if let (true, Some(t), TurnInput::Steer(text)) = (running, &turn, &input) {
                    let id = format!("steer_user_{}", client_id);
                    if !l.steers.iter().any(|(s, _)| *s == id) {
                        l.steers.push((id, text.clone()));
                    }
                    return Ok(Accepted::SteerQueued(t.clone()));
                }
                // A running turn means no question is pending, so an answer can only be stale.
                if running && matches!(input, TurnInput::Answer { .. }) {
                    return Err(Rejected::StaleQuestion);
                }
                return Err(Rejected::InProgress { turn_id: turn });
            }
        };
        let (chats, id) = (h.stores().chats.clone(), chat.to_owned());
        let (rec, mut s, mut meta) = blocking(move || load(chats.as_ref(), &id)).await?;
        let status = {
            let (rt, h2, c) = (self.clone(), h.clone(), chat.to_owned());
            blocking(move || Ok(rt.status(&h2, &c))).await?
        };
        let failed = status.state == TurnState::Error;
        let retry = failed
            && status.client_id.as_deref() == Some(client_id.as_str())
            && meta.applied(&client_id).is_some();
        if let Some(t) = meta.applied(&client_id) {
            if !retry {
                return Ok(Accepted::Duplicate(t.to_owned()));
            }
        }
        let turn_id = new_id("turn");
        if retry || input == TurnInput::Resume {
            if !failed && !awaiting_model(&s) {
                return Err(Rejected::NothingToResume);
            }
        } else {
            let pending = pending_call(&s).cloned();
            match input {
                TurnInput::User(text) => {
                    if let Some(p) = pending {
                        return Err(Rejected::PendingQuestion { call_id: p.id });
                    }
                    s.push_user(text);
                }
                TurnInput::Answer { call_id, answer } => {
                    let call = pending
                        .filter(|p| p.id == call_id)
                        .ok_or(Rejected::StaleQuestion)?;
                    let text = render_for(&call, &answer)?;
                    s.answer(&call_id, text);
                }
                TurnInput::Steer(text) => {
                    let id = format!("steer_user_{}", client_id);
                    if pending.is_some() {
                        // A clarifying question about the pending question or the lesson: answer
                        // it inline, in plain text with no tools, and leave the pending question
                        // open rather than jumping ahead to a new one.
                        self.permits.resize(ctx.cap);
                        let engine = Permitted {
                            workspace: h.id().to_owned(),
                            inner: (ctx.engines)(role(&s)),
                            permits: self.permits.clone(),
                        };
                        let root = h.root().join("artifacts");
                        let (etx, erx) = mpsc::channel(EVENT_CAP);
                        let fwd = tokio::spawn(forward(
                            self.pump.clone(),
                            erx,
                            h.id().to_owned(),
                            chat.to_owned(),
                            turn_id.clone(),
                            ctx.shutdown.clone(),
                        ));
                        let clarified = lp_agent::loops::answer_pending_steer(
                            &engine,
                            &root,
                            &mut s,
                            &id,
                            &text,
                            &etx,
                            &ctx.shutdown,
                        )
                        .await;
                        drop(etx);
                        let _ = fwd.await;
                        if let Err(e) = clarified {
                            tracing::warn!(chat, "clarifying answer failed: {e}");
                        }
                        meta.remember(&client_id, &turn_id);
                        let now = self.now();
                        let chats = h.stores().chats.clone();
                        let mut rec = rec;
                        let version = blocking(move || {
                            Ok(save(chats.as_ref(), &mut rec, &s, &mut meta, now)?)
                        })
                        .await?;
                        drop(guard);
                        self.publish(
                            h.id(),
                            Event::ChatRest {
                                chat_id: chat.to_owned(),
                                view_version: version,
                            },
                        );
                        return Ok(Accepted::Recorded(turn_id));
                    }
                    s.messages.push(Msg::Steer { id, text });
                }
                TurnInput::Resume => {}
            }
        }
        meta.remember(&client_id, &turn_id);
        self.begin(
            ctx.clone(),
            chat.to_owned(),
            slot,
            guard,
            rec,
            s,
            meta,
            turn_id.clone(),
            client_id,
            true,
        )
        .await;
        Ok(Accepted::Started(turn_id))
    }

    /// Saves the applied input, writes the turn marker, marks the slot running and spawns the
    /// turn. Errors here end the turn at once as a failed turn.
    #[allow(clippy::too_many_arguments)]
    async fn begin(
        self: &Arc<Self>,
        ctx: TurnCtx<E>,
        chat: String,
        slot: Arc<Slot>,
        guard: OwnedMutexGuard<()>,
        mut rec: ChatRecord,
        s: Session,
        mut meta: ChatMeta,
        turn_id: String,
        client_id: String,
        persist_input: bool,
    ) {
        let now = self.now();
        let cancel = slot.start(&turn_id, &client_id, now);
        let h = ctx.handle.clone();
        let marker = Marker {
            turn_id: turn_id.clone(),
            client_id,
            started_at: now,
        };
        let (chats, root, c, sess) = (
            h.stores().chats.clone(),
            h.root().to_owned(),
            chat.clone(),
            s.clone(),
        );
        let saved = blocking(move || {
            if persist_input {
                save(chats.as_ref(), &mut rec, &sess, &mut meta, now)?;
            }
            write_marker(&root, &c, &marker)
                .map_err(|e| Rejected::Store(StoreError::Backend(e.to_string())))?;
            Ok((rec, meta))
        })
        .await;
        let rt = self.clone();
        match saved {
            Ok((rec, meta)) => {
                tokio::spawn(async move {
                    rt.drive(ctx, chat, slot, guard, rec, s, meta, turn_id, cancel)
                        .await;
                });
            }
            Err(e) => {
                tracing::error!(chat, "turn not started: {e}");
                slot.finish(Some(TurnFailure {
                    code: "internal".into(),
                    message: capped(&format!("Saving the chat failed: {e}"), ERROR_CHARS),
                    retryable: true,
                    retry_after_ms: None,
                    interrupted: false,
                    ai: None,
                }));
                drop(guard);
                self.publish(
                    h.id(),
                    Event::ChatUpdated {
                        chat_id: chat.clone(),
                    },
                );
            }
        }
    }

    /// The spawned turn.
    #[allow(clippy::too_many_arguments)]
    async fn drive(
        self: Arc<Self>,
        ctx: TurnCtx<E>,
        chat: String,
        slot: Arc<Slot>,
        guard: OwnedMutexGuard<()>,
        mut rec: ChatRecord,
        mut s: Session,
        mut meta: ChatMeta,
        turn_id: String,
        cancel: CancellationToken,
    ) {
        let h = ctx.handle.clone();
        let ws = h.id().to_owned();
        let _busy = h.busy();
        let stop = {
            let (c, sd, wsc) = (cancel.clone(), ctx.shutdown.clone(), h.cancel_token());
            tokio::spawn(async move {
                tokio::select! {
                    () = sd.cancelled() => c.cancel(),
                    () = wsc.cancelled() => c.cancel(),
                    () = c.cancelled() => {}
                }
            })
        };
        self.publish(
            &ws,
            Event::ChatUpdated {
                chat_id: chat.clone(),
            },
        );
        let gate = Arc::new(TurnGate {
            workspace: ws.clone(),
            permits: self.permits.clone(),
            faults: self.faults.clone(),
            pool: self.pool.clone(),
            queue_limit: self.config.queue_limit,
            observer: Arc::new(SlotObserver {
                slot: slot.clone(),
                pump: self.pump.clone(),
                ws: ws.clone(),
                chat: chat.clone(),
            }),
            partial: Mutex::default(),
            surfaced_retry_after: Mutex::default(),
        });
        let (tx, rx) = mpsc::channel(EVENT_CAP);
        let fwd = tokio::spawn(forward(
            self.pump.clone(),
            rx,
            ws.clone(),
            chat.clone(),
            turn_id.clone(),
            cancel.clone(),
        ));
        let graph: Arc<dyn lp_core::ports::GraphStore> = h.stores().graph.clone();
        s = s.with_graph(graph);
        let res = match self.bench(&ctx).await {
            Ok(bench) => {
                self.run_loop(&ctx, &bench, &gate, &slot, &mut s, &mut meta, &tx, &cancel)
                    .await
            }
            Err(e) => Err(LoopError::Store(StoreError::Backend(e.to_string()))),
        };
        drop(tx);
        let _ = fwd.await;
        stop.abort();
        let user_cancel = slot.live.lock().user_cancel;
        let shutting = ctx.shutdown.is_cancelled();
        let mut failure = match &res {
            Ok(()) => {
                self.compact(&ctx, &gate, &mut s, &mut meta, &cancel).await;
                None
            }
            Err(e) if e.cancelled() && user_cancel && !shutting => {
                let partial = std::mem::take(&mut *gate.partial.lock());
                if !partial.trim().is_empty() {
                    s.messages.push(Msg::Assistant {
                        text: partial,
                        calls: vec![],
                    });
                    meta.mark_interrupted(s.messages.len() - 1);
                }
                None
            }
            Err(e) if e.cancelled() => Some(interrupted()),
            Err(e) => {
                tracing::warn!(workspace = ws, chat, "turn failed: {e}");
                Some(failure_of(e, &gate))
            }
        };
        if failure.is_some() {
            let partial = std::mem::take(&mut *gate.partial.lock());
            let already_saved =
                matches!(s.messages.last(), Some(Msg::Assistant { text, .. }) if *text == partial);
            if !partial.trim().is_empty() && !already_saved {
                s.messages.push(Msg::Assistant {
                    text: partial,
                    calls: vec![],
                });
                meta.mark_interrupted(s.messages.len() - 1);
            }
        }
        // Steers that found no step to join stay in the transcript for the next one.
        for (id, text) in slot.take_steers() {
            if !s.has_steer_message(&id) {
                let client = id.strip_prefix("steer_user_").map(str::to_owned);
                s.messages.push(Msg::Steer { id, text });
                if let Some(client) = client {
                    meta.remember(&client, &turn_id);
                }
            }
        }
        let now = self.now();
        let keep_marker = failure.is_some();
        let (chats, root, c) = (h.stores().chats.clone(), h.root().to_owned(), chat.clone());
        let saved = blocking(move || {
            let v = save(chats.as_ref(), &mut rec, &s, &mut meta, now)?;
            if !keep_marker {
                remove_marker(&root, &c);
            }
            Ok(v)
        })
        .await;
        let version = match saved {
            Ok(v) => Some(v),
            Err(e) => {
                tracing::error!(workspace = ws, chat, "chat not saved: {e}");
                failure.get_or_insert(TurnFailure {
                    code: "internal".into(),
                    message: capped(&format!("Saving the chat failed: {e}"), ERROR_CHARS),
                    retryable: true,
                    retry_after_ms: None,
                    interrupted: false,
                    ai: None,
                });
                None
            }
        };
        slot.finish(failure.clone());
        drop(guard);
        if let Some(f) = failure {
            self.publish(
                &ws,
                Event::ChatError {
                    chat_id: chat.clone(),
                    turn_id: turn_id.clone(),
                    error: ev::TurnError {
                        code: f.code,
                        message: f.message,
                        retryable: f.retryable,
                    },
                },
            );
        }
        match version {
            Some(v) => self.publish(
                &ws,
                Event::ChatRest {
                    chat_id: chat,
                    view_version: v,
                },
            ),
            None => self.publish(&ws, Event::ChatUpdated { chat_id: chat }),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn run_loop(
        &self,
        ctx: &TurnCtx<E>,
        bench: &Arc<Bench<E>>,
        gate: &Arc<TurnGate>,
        slot: &Slot,
        s: &mut Session,
        meta: &mut ChatMeta,
        tx: &mpsc::Sender<StreamDelta>,
        cancel: &CancellationToken,
    ) -> Result<(), LoopError> {
        let root = ctx.handle.root().join("artifacts");
        for round in 0..MAX_ROUNDS {
            let wb = bench.wb.clone();
            let woke = on_session(s, move |s| wb.deliver(s)).await?;
            let mut steered = false;
            if !s.pending_question() {
                for (id, text) in slot.take_steers() {
                    let client = id.strip_prefix("steer_user_").map(str::to_owned);
                    s.messages.push(Msg::Steer { id, text });
                    if let Some(client) = client {
                        if let Some(turn) = slot.live.lock().turn_id.as_deref() {
                            meta.remember(&client, turn);
                        }
                    }
                    steered = true;
                }
            }
            if round > 0 && !woke && !steered {
                return Ok(());
            }
            if s.pending_question() {
                return Ok(());
            }
            run_turn_on_blocking(
                ctx.engines.clone(),
                gate.clone(),
                root.clone(),
                s,
                tx.clone(),
                cancel.clone(),
            )
            .await?;
            bench.jobs.name_requests(s);
            let wb = bench.wb.clone();
            on_session(s, move |s| wb.launch(s).map(drop)).await?;
            meta.stamp(s, self.now());
        }
        Ok(())
    }

    /// Summarizes the oldest part of a long transcript. A failure only logs.
    async fn compact(
        &self,
        ctx: &TurnCtx<E>,
        gate: &Arc<TurnGate>,
        s: &mut Session,
        meta: &mut ChatMeta,
        cancel: &CancellationToken,
    ) {
        if estimate_tokens(s) <= COMPACT_TRIGGER_TOKENS {
            return;
        }
        let Some(cut) = s.compaction_cut() else {
            return;
        };
        let mut prompt = String::from(
            "Summarize the earlier part of this tutoring conversation for the tutor who continues it: \
             what the learner already knows, what they got wrong, what was taught and what was planned. \
             Plain prose, at most 300 words, no tool calls.\n\nConversation:\n",
        );
        for m in &s.messages[..cut] {
            prompt.push_str(&serde_json::to_string(m).unwrap_or_default());
            prompt.push('\n');
        }
        let engine = Gated {
            inner: (ctx.engines)("teacher"),
            gate: gate.clone(),
        };
        let (tx, mut rx) = mpsc::channel(EVENT_CAP);
        let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
        let req = TurnRequest {
            prompt,
            ..TurnRequest::default()
        };
        let res = engine.stream_turn(req, tx, cancel.clone()).await;
        let _ = drain.await;
        match res {
            Ok(r) if !r.text.trim().is_empty() => {
                s.apply_compaction(cut, r.text.trim().to_owned());
                meta.compacted(cut, self.now());
                tracing::info!(cut, "compacted the transcript");
            }
            Ok(_) => tracing::warn!("compaction returned no summary"),
            Err(e) => tracing::warn!("compaction failed: {e}"),
        }
    }

    /// Cancels the running or queued turn of `chat`; its turn id if there was one.
    pub fn cancel(&self, ws: &str, chat: &str) -> Option<String> {
        let slot = self
            .slots
            .lock()
            .get(&(ws.to_owned(), chat.to_owned()))
            .cloned()?;
        let mut l = slot.live.lock();
        let token = l.cancel.clone()?;
        l.user_cancel = true;
        token.cancel();
        l.turn_id.clone()
    }

    /// After shutdown cancellation, wait until turns have saved their interrupted transcript.
    pub async fn wait_idle(&self) {
        let locks: Vec<_> = self.slots.lock().values().map(|s| s.lock.clone()).collect();
        for lock in locks {
            let _guard = lock.lock_owned().await;
        }
    }

    /// Jobs of the workspace, newest first.
    pub async fn jobs(self: &Arc<Self>, ctx: &TurnCtx<E>) -> Result<Vec<JobView>, Rejected> {
        self.ensure(ctx).await?;
        let jobs = ctx.handle.stores().jobs.clone();
        blocking(move || {
            let (all, skipped) = jobs.all_report()?;
            for s in skipped {
                tracing::warn!("skipped unreadable job file {s}");
            }
            let mut out: Vec<JobView> = all.iter().map(JobView::from).collect();
            out.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.id.cmp(&b.id)));
            Ok(out)
        })
        .await
    }

    /// Stops a running job; its result is never delivered.
    pub async fn abort_job(
        self: &Arc<Self>,
        ctx: &TurnCtx<E>,
        job_id: &str,
    ) -> Result<JobView, Rejected> {
        let job = self
            .jobs(ctx)
            .await?
            .into_iter()
            .find(|j| j.id == job_id)
            .ok_or_else(|| Rejected::NotFound(job_id.to_owned()))?;
        if job.status != JobStatus::Running {
            return Err(Rejected::Conflict(format!("job {job_id} is not running")));
        }
        self.bench(ctx).await?.wb.abort(job_id);
        Ok(job)
    }

    /// Adds an imported note as user-provided source context for the next tutor turn.
    pub async fn attach_note(
        self: &Arc<Self>,
        ctx: &TurnCtx<E>,
        chat: &str,
        artifact_id: &str,
        text: String,
    ) -> Result<(), Rejected> {
        self.ensure(ctx).await?;
        let h = ctx.handle.clone();
        let slot = self.slot(&h, chat);
        let guard = slot
            .lock
            .clone()
            .try_lock_owned()
            .map_err(|_| Rejected::InProgress {
                turn_id: slot.live.lock().turn_id.clone(),
            })?;
        let (chats, id) = (h.stores().chats.clone(), chat.to_owned());
        let now = self.now();
        let source = format!("Imported note (source {artifact_id}):\n\n{text}");
        let version = blocking(move || {
            let (mut rec, mut s, mut meta) = load(chats.as_ref(), &id)?;
            s.messages.push(Msg::User(source));
            Ok(save(chats.as_ref(), &mut rec, &s, &mut meta, now)?)
        })
        .await?;
        drop(guard);
        self.publish(
            h.id(),
            Event::ChatRest {
                chat_id: chat.to_owned(),
                view_version: version,
            },
        );
        Ok(())
    }

    /// Starts a PDF reading job for `path` attached to `chat`; its job id.
    pub async fn attach_pdf(
        self: &Arc<Self>,
        ctx: &TurnCtx<E>,
        chat: &str,
        path: &Path,
        question: &str,
    ) -> Result<String, Rejected> {
        let h = ctx.handle.clone();
        let slot = self.slot(&h, chat);
        let guard = slot
            .lock
            .clone()
            .try_lock_owned()
            .map_err(|_| Rejected::InProgress {
                turn_id: slot.live.lock().turn_id.clone(),
            })?;
        let bench = self.bench(ctx).await?;
        let (chats, id) = (h.stores().chats.clone(), chat.to_owned());
        let (mut rec, mut s, mut meta) = blocking(move || load(chats.as_ref(), &id)).await?;
        let question = match question.trim() {
            "" => "Summarize the key ideas of this document for the current lesson.".to_owned(),
            q => check_text("question", q)?,
        };
        let call = Call {
            id: format!("upload#{}", s.messages.len()),
            name: "pdf".into(),
            args: json!({ "path": path.display().to_string(), "question": question }),
        };
        s.messages.push(Msg::Assistant {
            text: String::new(),
            calls: vec![call.clone()],
        });
        let ack = match lp_agent::workbench::exec(&mut s, &call) {
            Some(Ok(Effect::Ack(text))) => text,
            Some(Err(e)) => return Err(invalid("file", e)),
            _ => return Err(invalid("file", "the PDF job could not be queued")),
        };
        s.messages.push(Msg::ToolResult {
            call_id: call.id.clone(),
            name: "pdf".into(),
            ok: true,
            text: ack,
        });
        let job = s.requests_ids().pop().unwrap_or_default();
        bench.jobs.name_requests(&s);
        let now = self.now();
        let (wb, chats) = (bench.wb.clone(), h.stores().chats.clone());
        let version = blocking(move || {
            wb.launch(&mut s)?;
            Ok(save(chats.as_ref(), &mut rec, &s, &mut meta, now)?)
        })
        .await?;
        drop(guard);
        self.publish(
            h.id(),
            Event::ChatRest {
                chat_id: chat.to_owned(),
                view_version: version,
            },
        );
        Ok(job)
    }
}

/// Runs one agent turn on a blocking-pool thread. The agent loop holds a `Box<dyn Agent>` across
/// awaits, so its future is not `Send` and cannot be spawned; `Handle::block_on` drives it with
/// the server runtime's drivers, and anything it spawns (CLI processes) lands on that runtime.
async fn run_turn_on_blocking<E: LlmEngine + 'static>(
    engines: Engines<E>,
    gate: Arc<TurnGate>,
    root: PathBuf,
    s: &mut Session,
    tx: mpsc::Sender<StreamDelta>,
    cancel: CancellationToken,
) -> Result<(), TurnError> {
    let handle = tokio::runtime::Handle::current();
    let mut taken = std::mem::replace(s, Session::new("", Kind::Teach));
    let joined = tokio::task::spawn_blocking(move || {
        let engine_for = |role: &str| {
            Arc::new(Gated {
                inner: engines(role),
                gate: gate.clone(),
            })
        };
        let out = handle.block_on(run_to_rest(&engine_for, &root, &mut taken, &tx, &cancel));
        (taken, out)
    })
    .await;
    let (back, out) = match joined {
        Ok(v) => v,
        Err(e) => std::panic::resume_unwind(e.into_panic()),
    };
    *s = back;
    out
}

/// Engine stream → `chat.delta` / `chat.toolCall` events.
async fn forward(
    pump: Arc<DeltaPump>,
    mut rx: mpsc::Receiver<StreamDelta>,
    ws: String,
    chat: String,
    turn: String,
    cancel: CancellationToken,
) {
    // Keep tool chips behind their text, and share one delta budget across text and thinking.
    let mut pending = std::collections::VecDeque::<StreamDelta>::new();
    let mut closed = false;
    let mut next = tokio::time::Instant::now();
    loop {
        if closed && pending.is_empty() {
            break;
        }
        tokio::select! {
            biased;
            () = cancel.cancelled() => break,
            d = rx.recv(), if !closed => match d {
                Some(StreamDelta::Text(t)) => match pending.back_mut() {
                    Some(StreamDelta::Text(previous)) => previous.push_str(&t),
                    _ => pending.push_back(StreamDelta::Text(t)),
                },
                Some(StreamDelta::Thinking(t)) => match pending.back_mut() {
                    Some(StreamDelta::Thinking(previous)) => previous.push_str(&t),
                    _ => pending.push_back(StreamDelta::Thinking(t)),
                },
                Some(d) => pending.push_back(d),
                None => closed = true,
            },
            () = tokio::time::sleep_until(next), if !pending.is_empty() => {}
        }
        loop {
            match pending.front() {
                Some(StreamDelta::ToolCall(_)) => {}
                Some(_) if tokio::time::Instant::now() >= next => {}
                _ => break,
            }
            match pending.pop_front() {
                Some(StreamDelta::Text(t)) => {
                    pump.delta(&ws, &chat, &turn, DeltaKind::Text, &t);
                    next = tokio::time::Instant::now() + ev::delta_interval();
                }
                Some(StreamDelta::Thinking(t)) => {
                    pump.delta(&ws, &chat, &turn, DeltaKind::Thinking, &t);
                    next = tokio::time::Instant::now() + ev::delta_interval();
                }
                Some(StreamDelta::ToolCall(c)) => {
                    pump.publish(
                        Scope::Workspace(ws.clone()),
                        Event::ChatToolCall {
                            chat_id: chat.clone(),
                            turn_id: turn.clone(),
                            call: ev::ToolCallInfo {
                                id: c.id,
                                name: c.name,
                            },
                        },
                    );
                }
                None => break,
            }
        }
    }
}

trait SteerLookup {
    fn has_steer_message(&self, id: &str) -> bool;
}

impl SteerLookup for Session {
    fn has_steer_message(&self, id: &str) -> bool {
        self.messages
            .iter()
            .any(|m| matches!(m, Msg::Steer { id: s, .. } if s == id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn call(name: &str, args: Value) -> Call {
        Call {
            id: "q#1".into(),
            name: name.into(),
            args,
        }
    }

    fn quiz(multi: bool) -> Call {
        call(
            "quiz",
            json!({"question": "Q", "options": [{"label": "A", "value": "a"}, {"label": "B", "value": "b"}],
                   "correctAnswer": "a", "explanation": "", "multiSelect": multi}),
        )
    }

    fn ans(choice: &[&str], text: &str) -> AnswerInput {
        AnswerInput {
            choice: choice.iter().map(|c| (*c).to_owned()).collect(),
            text: text.into(),
            confidence: Some(4),
            ..AnswerInput::default()
        }
    }

    #[test]
    fn answers_are_validated_against_the_pending_call() {
        let q = quiz(false);
        assert_eq!(
            render_for(&q, &ans(&["b"], "")).unwrap(),
            "selected: b; confidence: 4/5"
        );
        assert_eq!(
            render_for(&q, &ans(&[IDK], "")).unwrap(),
            "selected: I don't know; confidence: 4/5"
        );
        for bad in [ans(&["z"], ""), ans(&["a", "b"], ""), ans(&[], "my guess")] {
            assert!(
                matches!(
                    render_for(&q, &bad),
                    Err(Rejected::Invalid {
                        field: "answer.choice",
                        ..
                    })
                ),
                "{bad:?}"
            );
        }
        assert!(render_for(&quiz(true), &ans(&["a", "b"], "")).is_ok());
        let mut low = ans(&["a"], "");
        low.confidence = Some(9);
        assert!(render_for(&q, &low).is_err());

        // A probe takes free text; recall and problems take text only.
        let probe = Call {
            name: "probe".into(),
            ..quiz(false)
        };
        assert!(render_for(&probe, &ans(&[], "no idea, guess b")).is_ok());
        let recall = call(
            "recall",
            json!({"question": "State it", "modelAnswer": "x"}),
        );
        assert!(render_for(&recall, &ans(&[], "")).is_err());
        assert!(render_for(&recall, &ans(&["a"], "")).is_err());
        assert!(render_for(&recall, &ans(&[IDK], "")).is_ok());
        let problem = call("problem", json!({"statement": "Reverse", "mode": "recall"}));
        let mut hint = ans(&[], "");
        hint.hint = true;
        assert_eq!(
            render_for(&problem, &hint).unwrap(),
            "answer: (asking for a hint); confidence: 4/5"
        );
    }

    #[test]
    fn resume_needs_the_model_to_be_next() {
        let mut s = Session::new("c", Kind::Teach);
        assert!(!awaiting_model(&s));
        s.push_user("hi");
        assert!(awaiting_model(&s));
        s.messages.push(Msg::Assistant {
            text: "hello".into(),
            calls: vec![],
        });
        assert!(!awaiting_model(&s));
        s.messages.push(Msg::Assistant {
            text: String::new(),
            calls: vec![quiz(false)],
        });
        assert!(!awaiting_model(&s), "a question waits");
    }

    #[test]
    fn markers_round_trip() {
        let d = tempfile::tempdir().unwrap();
        let m = Marker {
            turn_id: "t".into(),
            client_id: "c".into(),
            started_at: 5,
        };
        assert!(read_marker(d.path(), "chat_1").is_none());
        write_marker(d.path(), "chat_1", &m).unwrap();
        assert_eq!(read_marker(d.path(), "chat_1"), Some(m));
        remove_marker(d.path(), "chat_1");
        remove_marker(d.path(), "chat_1");
        assert!(read_marker(d.path(), "chat_1").is_none());
    }

    #[test]
    fn a_torn_or_zero_byte_marker_is_treated_as_none_not_an_error() {
        let d = tempfile::tempdir().unwrap();
        let m = Marker {
            turn_id: "t".into(),
            client_id: "c".into(),
            started_at: 5,
        };
        write_marker(d.path(), "chat_1", &m).unwrap();
        let path = marker_path(d.path(), "chat_1");

        // Zero-byte (crash right after create, before any bytes landed).
        std::fs::write(&path, []).unwrap();
        assert!(read_marker(d.path(), "chat_1").is_none());

        // Torn (crash mid-write): a truncated prefix of valid JSON.
        let full = serde_json::to_vec(&m).unwrap();
        std::fs::write(&path, &full[..full.len() / 2]).unwrap();
        assert!(read_marker(d.path(), "chat_1").is_none());

        // A fresh write still succeeds over the wreckage.
        write_marker(d.path(), "chat_1", &m).unwrap();
        assert_eq!(read_marker(d.path(), "chat_1"), Some(m));
    }
}
