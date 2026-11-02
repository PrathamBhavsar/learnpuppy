//! Chat wire types: summaries and the `ClassroomView` the Learn screen renders
//! (docs/web/03 §Learn).
//!
//! Pending questions are built field by field from the question call. The correct answer of a
//! quiz, a recall's model answer and a problem's model solution are never copied: there is no
//! field they could travel in.

use lp_agent::loops::{Call, Kind, Msg, Session, MAX_PROBE_QUESTIONS};
use lp_core::plan::{shuffle_options, Phase, ProblemInput, QuizInput, QuizOption};
use lp_core::ports::{ChatRecord, JobRecord, JobStatus};
use lp_service::chat::{self, ChatMeta, Prompt, IDK};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChatKind {
    Teach,
    Practice,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ChatPhase {
    Probe,
    Plan,
    Teach,
    Practice,
    Done,
}

/// The session stored in a chat record. A record that has no session yet (or an unreadable
/// one) reads as an empty teach session.
pub fn session_of(rec: &ChatRecord) -> Session {
    serde_json::from_value(rec.body.clone()).unwrap_or_else(|_| {
        let practice = rec
            .body
            .get("kind")
            .and_then(|k| k.as_str())
            .is_some_and(|k| k == "Practice");
        Session::new(
            rec.id.to_string(),
            if practice {
                Kind::Practice
            } else {
                Kind::Teach
            },
        )
    })
}

fn phase_of(s: &Session, view: &chat::ClassroomView) -> ChatPhase {
    let concluded = s
        .outcomes()
        .iter()
        .any(|o| o.succeeded && o.name == "conclude");
    let at_end =
        view.plan.is_some() && view.statuses.current.is_some() && view.statuses.upcoming.is_empty();
    match (s.kind, s.phase()) {
        (Kind::Practice, _) if concluded && view.prompt.is_none() => ChatPhase::Done,
        (Kind::Practice, _) => ChatPhase::Practice,
        (Kind::Teach, Phase::Probe) => ChatPhase::Probe,
        (Kind::Teach, Phase::Plan) => ChatPhase::Plan,
        (Kind::Teach, Phase::Teach) if at_end && view.prompt.is_none() => ChatPhase::Done,
        (Kind::Teach, Phase::Teach) => ChatPhase::Teach,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatSummaryDto {
    pub id: String,
    pub kind: ChatKind,
    pub title: String,
    pub phase: ChatPhase,
    pub updated_at: i64,
    pub message_count: usize,
    /// A question waits for the learner.
    pub pending: bool,
    /// Workbench jobs still running for this chat.
    pub jobs: usize,
}

impl ChatSummaryDto {
    pub fn new(rec: &ChatRecord, jobs: &[JobRecord]) -> Self {
        let s = session_of(rec);
        let view = chat::classroom_view(&s, "");
        Self {
            id: rec.id.to_string(),
            kind: kind_of(&s),
            title: rec.title.clone(),
            phase: phase_of(&s, &view),
            updated_at: rec.updated_at,
            message_count: s.messages.len(),
            pending: s.pending_question(),
            jobs: jobs
                .iter()
                .filter(|j| j.chat_id == rec.id && j.status == JobStatus::Running)
                .count(),
        }
    }
}

fn kind_of(s: &Session) -> ChatKind {
    match s.kind {
        Kind::Teach => ChatKind::Teach,
        Kind::Practice => ChatKind::Practice,
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatPageDto {
    /// Most recently updated first.
    pub items: Vec<ChatSummaryDto>,
    pub next_cursor: Option<String>,
    pub total: usize,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ChatsQuery {
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct RenameChat {
    pub title: String,
}

// ---------------------------------------------------------------- the classroom

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
    /// A notice from a tool the learner should see (background job started, model solution).
    Tool,
    Steer,
    Compaction,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct CallDto {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MessageDto {
    /// `m<index>` in the stored transcript.
    pub id: String,
    pub role: MessageRole,
    /// Markdown.
    pub text: String,
    pub thinking: Option<String>,
    pub interrupted: Option<bool>,
    /// Tool calls the assistant made in this message (names only, never arguments).
    pub calls: Vec<CallDto>,
    /// For an answer: the question call it answers.
    pub answers_call_id: Option<String>,
    /// Epoch ms; null for messages the transcript did not time-stamp.
    pub created_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct OptionDto {
    /// What to send back as the answer.
    pub key: String,
    pub text: String,
}

fn option_dtos(opts: Vec<QuizOption>) -> Vec<OptionDto> {
    opts.into_iter()
        .map(|o| OptionDto {
            key: o.value,
            text: o.label,
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProblemDto {
    /// `recall`, `discrimination` or `composition`.
    pub mode: String,
    pub statement: String,
    /// Hint rungs, when the mode has a ladder.
    pub rungs: Option<Vec<String>>,
}

/// The question waiting for the learner. Has no field for the correct answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PendingDto {
    pub call_id: String,
    /// `probe`, `recall`, `quiz`, `explain`, `ask_user_question` or `problem`.
    pub tool: String,
    pub prompt: String,
    /// Already shuffled for this call.
    pub options: Option<Vec<OptionDto>>,
    pub allow_free_text: bool,
    pub multi_select: bool,
    /// Confidence slider `1..=n`; absent when the tool takes no confidence.
    pub confidence_scale: Option<u8>,
    /// The answer value that means "I don't know" (options only).
    pub idk_key: Option<String>,
    pub problem: Option<ProblemDto>,
}

fn pending(call: &Call, prompt: Prompt) -> PendingDto {
    let mut p = PendingDto {
        call_id: call.id.clone(),
        tool: call.name.clone(),
        prompt: String::new(),
        options: None,
        allow_free_text: true,
        multi_select: false,
        confidence_scale: None,
        idk_key: None,
        problem: None,
    };
    match prompt {
        Prompt::Quiz { call_id, input } => {
            p.prompt = input.question.clone();
            p.options = Some(option_dtos(shuffle_options(&input.options, &call_id)));
            p.multi_select = input.multi_select;
            p.allow_free_text = call.name == "probe";
            p.confidence_scale = Some(5);
            p.idk_key = Some(IDK.to_owned());
        }
        Prompt::Free { question, .. } => {
            p.prompt = question;
            match call.name.as_str() {
                "ask_user_question" => {
                    let opts: Option<Vec<QuizOption>> = call
                        .args
                        .get("options")
                        .and_then(|o| serde_json::from_value(o.clone()).ok());
                    if let Some(o) = opts.filter(|o| !o.is_empty()) {
                        p.options = Some(option_dtos(o));
                    }
                }
                "problem" => {
                    if let Ok(pr) = serde_json::from_value::<ProblemInput>(call.args.clone()) {
                        let mode = serde_json::to_value(pr.mode)
                            .ok()
                            .and_then(|v| v.as_str().map(str::to_owned))
                            .unwrap_or_default();
                        p.problem = Some(ProblemDto {
                            mode,
                            statement: pr.statement,
                            rungs: None,
                        });
                    }
                }
                _ => {}
            }
        }
    }
    p
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum PlanStatus {
    Current,
    Visited,
    Upcoming,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PlanNodeDto {
    pub id: String,
    pub title: String,
    pub summary: String,
    pub status: PlanStatus,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct PlanEdgeDto {
    pub from: String,
    pub to: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanDto {
    pub nodes: Vec<PlanNodeDto>,
    pub edges: Vec<PlanEdgeDto>,
    pub current_id: Option<String>,
}

fn plan_dto(view: &chat::ClassroomView) -> Option<PlanDto> {
    let tree = view.plan.as_ref()?;
    let status = |id: &str| {
        if view.statuses.current.as_deref() == Some(id) {
            PlanStatus::Current
        } else if view.statuses.visited.iter().any(|v| v == id) {
            PlanStatus::Visited
        } else {
            PlanStatus::Upcoming
        }
    };
    Some(PlanDto {
        nodes: tree
            .nodes
            .iter()
            .map(|n| PlanNodeDto {
                id: n.id.clone(),
                title: n.title.clone(),
                summary: n.summary.clone(),
                status: status(&n.id),
            })
            .collect(),
        edges: tree
            .edges
            .iter()
            .map(|(a, b)| PlanEdgeDto {
                from: a.clone(),
                to: b.clone(),
            })
            .collect(),
        current_id: view.statuses.current.clone(),
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProbeProgressDto {
    pub asked: usize,
    pub max: usize,
}

/// The grade of an answered quiz or probe. Exists only after the answer.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerdictDto {
    pub call_id: String,
    pub correct: bool,
    pub explanation: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum JobStatusDto {
    Running,
    Done,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobDto {
    pub id: String,
    /// `research`, `pdf`, `visual` or `demo`.
    pub kind: String,
    pub status: JobStatusDto,
    pub title: String,
    pub artifact_id: Option<String>,
}

impl From<&JobRecord> for JobDto {
    fn from(j: &JobRecord) -> Self {
        Self {
            id: j.id.to_string(),
            kind: chat::kind_name(j.kind).to_owned(),
            status: j.status.into(),
            title: chat::job_title(j),
            artifact_id: chat::job_artifact(j),
        }
    }
}

impl From<JobStatus> for JobStatusDto {
    fn from(s: JobStatus) -> Self {
        match s {
            JobStatus::Running => JobStatusDto::Running,
            JobStatus::Done => JobStatusDto::Done,
            JobStatus::Failed => JobStatusDto::Failed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum TurnState {
    Idle,
    Running,
    Queued,
    Error,
}

/// Why the last turn stopped short.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnErrorDto {
    /// AI failures: `OFFLINE`, `NOT_CONNECTED`, `NO_MODEL`, `RATE_LIMITED`, `TIMED_OUT`,
    /// `NO_RESPONSE`, `UNKNOWN`. Others: `engine_unavailable`, `rate_limited`, `internal`,
    /// `storage_full`, or `interrupted` when a server restart cut the turn off.
    pub code: String,
    pub message: String,
    pub retryable: bool,
    /// For `rate_limited`: how long until an account is free again.
    pub retry_after_ms: Option<u64>,
    /// The server stopped mid-turn; offer Resume.
    pub interrupted: bool,
    /// `RATE_LIMITED`: epoch ms when the soonest limit lifts.
    #[serde(default)]
    pub resets_at: Option<i64>,
    /// The provider the failure is about (`claude`, `codex`, `agy`, `kiro`, `opencode`).
    #[serde(default)]
    pub provider: Option<String>,
    #[serde(default)]
    pub model: Option<String>,
    /// What to do next: a command to run (`kiro-cli login`) or a screen (`Settings`).
    #[serde(default)]
    pub action: Option<String>,
    /// `OFFLINE`: seconds until the automatic retry.
    #[serde(default)]
    pub retry_in_secs: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnDto {
    pub state: TurnState,
    pub turn_id: Option<String>,
    pub started_at: Option<i64>,
    /// The request that started the turn; post it again to retry after an error.
    #[serde(default)]
    pub client_id: Option<String>,
    /// While `queued` on a rate limit: when the next attempt starts (epoch ms).
    #[serde(default)]
    pub retry_at: Option<i64>,
    #[serde(default)]
    pub error: Option<TurnErrorDto>,
}

impl TurnDto {
    pub fn idle() -> Self {
        Self::from(&chat::TurnStatus::default())
    }
}

impl From<&chat::TurnStatus> for TurnDto {
    fn from(t: &chat::TurnStatus) -> Self {
        Self {
            state: match t.state {
                chat::TurnState::Idle => TurnState::Idle,
                chat::TurnState::Running => TurnState::Running,
                chat::TurnState::Queued => TurnState::Queued,
                chat::TurnState::Error => TurnState::Error,
            },
            turn_id: t.turn_id.clone(),
            started_at: t.started_at,
            client_id: t.client_id.clone(),
            retry_at: t.retry_at,
            error: t.error.as_ref().map(|e| TurnErrorDto {
                code: e.code.clone(),
                message: e.message.clone(),
                retryable: e.retryable,
                retry_after_ms: e.retry_after_ms,
                interrupted: e.interrupted,
                resets_at: e
                    .ai
                    .as_ref()
                    .and_then(|a| a.resets_at)
                    .and_then(|r| i64::try_from(r).ok()),
                provider: e.ai.as_ref().and_then(|a| a.provider.clone()),
                model: e.ai.as_ref().and_then(|a| a.model.clone()),
                action: e.ai.as_ref().and_then(|a| a.action.clone()),
                retry_in_secs: e.ai.as_ref().and_then(|a| a.retry_in_secs),
            }),
        }
    }
}

/// Everything the Classroom renders, derived server-side from the stored session.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClassroomViewDto {
    pub id: String,
    pub kind: ChatKind,
    pub title: String,
    pub phase: ChatPhase,
    /// Changes with every saved turn; `chat.rest` carries the version to expect.
    pub view_version: u64,
    pub messages: Vec<MessageDto>,
    pub pending: Option<PendingDto>,
    pub plan: Option<PlanDto>,
    pub probe_progress: ProbeProgressDto,
    pub verdicts: Vec<VerdictDto>,
    pub problem_result: Option<ProblemResultDto>,
    pub jobs: Vec<JobDto>,
    pub turn: TurnDto,
}

/// Feedback committed by a successful Practice conclude call.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProblemResultDto {
    pub call_id: String,
    pub outcome: String,
    pub verdict: String,
    pub rung: Option<String>,
    pub miss_class: Option<String>,
    pub retest: bool,
}

fn problem_result(s: &Session) -> Option<ProblemResultDto> {
    s.messages.iter().rev().find_map(|m| {
        let Msg::ToolResult {
            call_id,
            name,
            ok: true,
            ..
        } = m
        else {
            return None;
        };
        if name != "conclude" {
            return None;
        }
        let call = s.messages.iter().find_map(|m| match m {
            Msg::Assistant { calls, .. } => calls.iter().find(|c| c.id == *call_id),
            _ => None,
        })?;
        let input: lp_core::plan::ConcludeInput = serde_json::from_value(call.args.clone()).ok()?;
        let wire = |v| {
            serde_json::to_value(v)
                .ok()
                .and_then(|v| v.as_str().map(str::to_owned))
        };
        Some(ProblemResultDto {
            call_id: call_id.clone(),
            outcome: wire(input.outcome)?,
            verdict: input.verdict,
            rung: input
                .rung
                .and_then(|r| serde_json::to_value(r).ok())
                .and_then(|v| v.as_str().map(str::to_owned)),
            miss_class: input
                .miss_class
                .and_then(|r| serde_json::to_value(r).ok())
                .and_then(|v| v.as_str().map(str::to_owned)),
            retest: input.retest,
        })
    })
}

/// Longest tool notice sent to the UI.
const NOTICE_CHARS: usize = 600;

fn capped(s: &str) -> String {
    if s.chars().count() <= NOTICE_CHARS {
        return s.to_owned();
    }
    let cut: String = s.chars().take(NOTICE_CHARS).collect();
    format!("{cut}…")
}

/// How an answer reads in the transcript: the picked option's letter and label instead of its
/// internal value.
fn friendly_answer(call: Option<&Call>, text: &str) -> String {
    let Some(rest) = text.strip_prefix("selected: ") else {
        return text.to_owned();
    };
    let (picked, tail) = rest.split_once(';').unwrap_or((rest, ""));
    let picked = picked.trim();
    let label = call
        .and_then(|c| {
            let q = serde_json::from_value::<QuizInput>(c.args.clone()).ok()?;
            let shown = shuffle_options(&q.options, &c.id);
            let names: Vec<String> = picked
                .split(',')
                .map(|v| {
                    let v = v.trim();
                    if v == IDK {
                        return "I don't know".to_owned();
                    }
                    shown.iter().position(|o| o.value == v).map_or_else(
                        || v.to_owned(),
                        |i| format!("{}. {}", char::from(b'A' + i as u8), shown[i].label),
                    )
                })
                .collect();
            Some(names.join(", "))
        })
        .unwrap_or_else(|| picked.to_owned());
    let label = if label == IDK {
        "I don't know".to_owned()
    } else {
        label
    };
    if tail.trim().is_empty() {
        label
    } else {
        format!("{label};{tail}")
    }
}

fn messages(s: &Session, meta: &ChatMeta) -> Vec<MessageDto> {
    let call = |id: &str| -> Option<&Call> {
        s.messages.iter().find_map(|m| match m {
            Msg::Assistant { calls, .. } => calls.iter().find(|c| c.id == id),
            _ => None,
        })
    };
    let msg = |i: usize, role: MessageRole, text: String| MessageDto {
        id: format!("m{i}"),
        role,
        text,
        thinking: None,
        interrupted: meta.is_interrupted(i).then_some(true),
        calls: vec![],
        answers_call_id: None,
        created_at: meta.time(i),
    };
    let mut out = Vec::new();
    for (i, m) in s.messages.iter().enumerate() {
        match m {
            Msg::User(t) => out.push(msg(i, MessageRole::User, t.clone())),
            Msg::Assistant { text, calls } => {
                if text.trim().is_empty() && calls.is_empty() {
                    continue;
                }
                let mut d = msg(i, MessageRole::Assistant, text.clone());
                d.calls = calls
                    .iter()
                    .map(|c| CallDto {
                        id: c.id.clone(),
                        name: c.name.clone(),
                    })
                    .collect();
                out.push(d);
            }
            Msg::Answer { call_id, text } => {
                let mut d = msg(i, MessageRole::User, friendly_answer(call(call_id), text));
                d.answers_call_id = Some(call_id.clone());
                out.push(d);
            }
            Msg::ToolResult { name, text, .. } if text.starts_with("dispatched as ") => {
                let job = text["dispatched as ".len()..]
                    .split_whitespace()
                    .next()
                    .unwrap_or_default();
                out.push(msg(
                    i,
                    MessageRole::Tool,
                    format!("{name} running in the background ({job})"),
                ));
            }
            Msg::ToolResult {
                name,
                ok: true,
                text,
                ..
            } if name == "conclude" => {
                out.push(msg(i, MessageRole::Tool, capped(text)));
            }
            Msg::ToolResult { .. } => {}
            Msg::Steer { text, .. } => {
                if let Some(body) = text
                    .strip_prefix(lp_agent::workbench::VISUAL_STEER)
                    .and_then(|t| t.split_once('\n'))
                    .map(|(_, b)| b)
                {
                    out.push(msg(
                        i,
                        MessageRole::Steer,
                        format!("**Diagram:**\n```\n{body}\n```"),
                    ));
                } else if !text.starts_with("Background ") && !text.trim().is_empty() {
                    out.push(msg(i, MessageRole::Steer, capped(text)));
                }
            }
            Msg::Compaction(t) => out.push(msg(i, MessageRole::Compaction, t.clone())),
        }
    }
    out
}

fn verdicts(s: &Session) -> Vec<VerdictDto> {
    s.messages
        .iter()
        .filter_map(|m| {
            let Msg::Answer { call_id, text } = m else {
                return None;
            };
            let call = s.messages.iter().find_map(|m| match m {
                Msg::Assistant { calls, .. } => calls
                    .iter()
                    .find(|c| c.id == *call_id && matches!(c.name.as_str(), "probe" | "quiz")),
                _ => None,
            })?;
            let q = serde_json::from_value::<QuizInput>(call.args.clone()).ok()?;
            let graded = chat::verdict(&q, call_id, text)?;
            Some(VerdictDto {
                call_id: call_id.clone(),
                correct: graded.starts_with("**Correct.**"),
                explanation: Some(q.explanation.trim().to_owned()).filter(|e| !e.is_empty()),
            })
        })
        .collect()
}

impl ClassroomViewDto {
    /// `turn` is the live turn state; the stored session knows nothing of it.
    pub fn new(rec: &ChatRecord, jobs: &[JobRecord], turn: TurnDto) -> Self {
        let s = session_of(rec);
        let meta = ChatMeta::of(&rec.body);
        let view = chat::classroom_view(&s, "");
        let pending = chat::pending_call(&s)
            .zip(view.prompt.clone())
            .map(|(call, prompt)| pending(call, prompt));
        Self {
            id: rec.id.to_string(),
            kind: kind_of(&s),
            title: rec.title.clone(),
            phase: phase_of(&s, &view),
            view_version: u64::try_from(rec.updated_at).unwrap_or(0),
            messages: messages(&s, &meta),
            pending,
            plan: plan_dto(&view),
            probe_progress: ProbeProgressDto {
                asked: s.probe_questions(),
                max: MAX_PROBE_QUESTIONS,
            },
            verdicts: verdicts(&s),
            problem_result: problem_result(&s),
            jobs: jobs
                .iter()
                .filter(|j| j.chat_id == rec.id)
                .map(JobDto::from)
                .collect(),
            turn,
        }
    }
}

// ---------------------------------------------------------------- chat commands

/// `POST …/chats`.
#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreateChat {
    pub kind: ChatKind,
    /// What to learn (teach; required) or an optional focus (practice).
    #[serde(default)]
    pub topic: String,
    /// Graph propositions the chat starts from (practice needs a topic or at least one).
    #[serde(default)]
    pub seed_node_ids: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CreatedChatDto {
    pub chat_id: String,
    /// The first turn, already running.
    pub turn_id: String,
}

/// The learner's answer to the pending question.
#[derive(Clone, Debug, Default, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnswerDto {
    /// Picked option keys (`pending.options[].key`, or `pending.idkKey`); several only when
    /// `pending.multiSelect`.
    #[serde(default)]
    pub choice: Vec<String>,
    /// Free text: a probe's own answer, a recall, an explanation, a problem attempt.
    #[serde(default)]
    pub text: String,
    #[serde(default)]
    pub reading_notes: String,
    /// `1..=5`, when `pending.confidenceScale` is set (default 3).
    #[serde(default)]
    pub confidence: Option<u8>,
    /// Problems only: ask for the next hint rung.
    #[serde(default)]
    pub hint: bool,
}

/// `POST …/chats/{id}/turn`. `clientId` makes retries safe: a repeated id returns the original
/// turn, and after an `error` it re-runs the turn.
#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum TurnBody {
    /// A learner message (not while a question is pending).
    User {
        text: String,
        #[serde(rename = "clientId")]
        client_id: String,
    },
    /// Answers the pending question `callId`.
    Answer {
        #[serde(rename = "callId")]
        call_id: String,
        answer: AnswerDto,
        #[serde(rename = "clientId")]
        client_id: String,
    },
    /// Guidance; joins the running turn between model steps, or starts a turn when idle.
    Steer {
        text: String,
        #[serde(rename = "clientId")]
        client_id: String,
    },
    /// Re-runs the last step after an error or a restart.
    Resume {
        #[serde(rename = "clientId")]
        client_id: String,
    },
}

impl TurnBody {
    pub fn into_input(self) -> (String, chat::TurnInput) {
        match self {
            TurnBody::User { text, client_id } => (client_id, chat::TurnInput::User(text)),
            TurnBody::Steer { text, client_id } => (client_id, chat::TurnInput::Steer(text)),
            TurnBody::Resume { client_id } => (client_id, chat::TurnInput::Resume),
            TurnBody::Answer {
                call_id,
                answer,
                client_id,
            } => (
                client_id,
                chat::TurnInput::Answer {
                    call_id,
                    answer: chat::AnswerInput {
                        choice: answer.choice,
                        text: answer.text,
                        reading_notes: answer.reading_notes,
                        confidence: answer.confidence,
                        hint: answer.hint,
                    },
                },
            ),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub enum TurnAcceptance {
    /// A new turn is running.
    Started,
    /// This `clientId` was already applied; `turnId` is its turn.
    Duplicate,
    /// A steer for the running turn `turnId`.
    SteerQueued,
    /// Saved; the model runs once the pending question is answered.
    Recorded,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct TurnAcceptedDto {
    pub turn_id: String,
    pub status: TurnAcceptance,
}

impl From<chat::Accepted> for TurnAcceptedDto {
    fn from(a: chat::Accepted) -> Self {
        let status = match &a {
            chat::Accepted::Started(_) => TurnAcceptance::Started,
            chat::Accepted::Duplicate(_) => TurnAcceptance::Duplicate,
            chat::Accepted::SteerQueued(_) => TurnAcceptance::SteerQueued,
            chat::Accepted::Recorded(_) => TurnAcceptance::Recorded,
        };
        Self {
            turn_id: a.turn_id().to_owned(),
            status,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CancelDto {
    /// False when no turn was running.
    pub cancelled: bool,
    pub turn_id: Option<String>,
}

/// One workbench job in the workspace's job list.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct JobItemDto {
    pub id: String,
    pub chat_id: String,
    /// `research`, `pdf`, `visual` or `demo`.
    pub kind: String,
    pub status: JobStatusDto,
    pub title: String,
    pub artifact_id: Option<String>,
    /// Why a finished job has no result.
    pub error: Option<String>,
    /// PDF jobs: pages read so far.
    pub pages_read: Option<u64>,
    /// PDF jobs: the document's page count.
    pub total_pages: Option<u64>,
    pub created_at: i64,
}

impl From<chat::JobView> for JobItemDto {
    fn from(j: chat::JobView) -> Self {
        Self {
            id: j.id,
            chat_id: j.chat_id,
            kind: j.kind.to_owned(),
            status: j.status.into(),
            title: j.title,
            artifact_id: j.artifact_id,
            error: j.error,
            pages_read: j.pages_read,
            total_pages: j.total_pages,
            created_at: j.created_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct JobListDto {
    /// Newest first.
    pub items: Vec<JobItemDto>,
}

/// UTF-8 Markdown or plain text, at most 1 MB.
#[derive(ToSchema)]
#[allow(dead_code)]
pub struct NoteUploadForm {
    #[schema(value_type = String, format = Binary)]
    pub file: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NoteUploadDto {
    pub artifact_id: String,
    pub chat_id: String,
    pub bytes: u64,
}

/// `POST …/sources/pdf` form: the PDF and an optional question for the reading job.
#[derive(ToSchema)]
#[allow(dead_code)]
pub struct PdfUploadForm {
    #[schema(value_type = String, format = Binary)]
    pub file: Vec<u8>,
    pub question: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PdfUploadDto {
    pub job_id: String,
    /// The stored PDF (`GET …/artifacts/{id}`).
    pub artifact_id: String,
    pub chat_id: String,
    pub bytes: u64,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PdfQuery {
    /// The chat the reading job reports to.
    pub chat: String,
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use lp_core::plan::{PlanNode, PlanTree};
    use lp_core::ports::JobKind;
    use serde_json::json;

    pub(crate) fn call(id: &str, name: &str, args: serde_json::Value) -> Msg {
        Msg::Assistant {
            text: String::new(),
            calls: vec![Call {
                id: id.into(),
                name: name.into(),
                args,
            }],
        }
    }

    pub(crate) fn record(s: &Session) -> ChatRecord {
        ChatRecord {
            id: s.id.as_str().into(),
            title: "Ownership".into(),
            created_at: 1,
            updated_at: 42,
            body: serde_json::to_value(s).unwrap(),
            turns: vec![],
            extractions: vec![],
        }
    }

    fn secret_quiz() -> serde_json::Value {
        json!({
            "question": "Who owns the value?",
            "options": [
                {"label": "The binding", "value": "opt_secret_right"},
                {"label": "The heap", "value": "opt_b"},
                {"label": "The stack", "value": "opt_c"}
            ],
            "correctAnswer": "opt_secret_right",
            "explanation": "SECRET-EXPLANATION: one owner at a time.",
            "type": "check",
            "nodeId": "n1"
        })
    }

    fn idle() -> TurnDto {
        TurnDto::idle()
    }

    #[test]
    fn pending_questions_never_carry_the_answer() {
        let mut quiz = Session::new("c1", Kind::Teach);
        quiz.messages.push(Msg::User("teach me".into()));
        quiz.messages.push(call("q#1", "quiz", secret_quiz()));

        let mut recall = Session::new("c2", Kind::Teach);
        recall.messages.push(call(
            "r#1",
            "recall",
            json!({"question": "State the rule", "modelAnswer": "SECRET-MODEL-ANSWER"}),
        ));

        let mut problem = Session::new("c3", Kind::Practice);
        problem.messages.push(call(
            "p#1",
            "problem",
            json!({"statement": "Reverse a list", "mode": "recall", "target": [],
                   "modelSolution": "SECRET-SOLUTION: iterate backwards"}),
        ));

        for (s, secrets) in [
            (&quiz, ["correctAnswer", "SECRET-EXPLANATION"]),
            (&recall, ["SECRET-MODEL-ANSWER", "modelAnswer"]),
            (&problem, ["SECRET-SOLUTION", "modelSolution"]),
        ] {
            let dto = ClassroomViewDto::new(&record(s), &[], idle());
            assert!(dto.pending.is_some(), "{}", s.id);
            let wire = serde_json::to_string(&dto).unwrap();
            for secret in secrets {
                assert!(!wire.contains(secret), "{secret} leaked: {wire}");
            }
            assert!(!wire.contains("correctAnswer") && !wire.contains("correct_answer"));
            assert!(dto.verdicts.is_empty(), "no verdict before the answer");
        }

        // The options are the shuffled ones the dock shows; their keys are plain values.
        let p = ClassroomViewDto::new(&record(&quiz), &[], idle())
            .pending
            .unwrap();
        assert_eq!((p.tool.as_str(), p.call_id.as_str()), ("quiz", "q#1"));
        assert_eq!(p.prompt, "Who owns the value?");
        let mut keys: Vec<_> = p.options.unwrap().into_iter().map(|o| o.key).collect();
        keys.sort();
        assert_eq!(keys, ["opt_b", "opt_c", "opt_secret_right"]);
        assert_eq!((p.confidence_scale, p.allow_free_text), (Some(5), false));
        assert_eq!(p.idk_key.as_deref(), Some(IDK));
    }

    #[test]
    fn answering_reveals_the_verdict_and_reads_naturally() {
        let mut s = Session::new("c1", Kind::Teach);
        s.messages.push(call("q#1", "quiz", secret_quiz()));
        s.answer("q#1", "selected: opt_secret_right; confidence: 5/5");
        s.messages.push(call("q#2", "quiz", secret_quiz()));
        s.answer("q#2", "selected: opt_b; confidence: 1/5");
        let dto = ClassroomViewDto::new(&record(&s), &[], idle());
        assert!(dto.pending.is_none());
        let v: Vec<_> = dto
            .verdicts
            .iter()
            .map(|v| (v.call_id.as_str(), v.correct))
            .collect();
        assert_eq!(v, [("q#1", true), ("q#2", false)]);
        assert!(dto.verdicts[0]
            .explanation
            .as_deref()
            .unwrap()
            .starts_with("SECRET-EXPLANATION"));
        let answers: Vec<_> = dto
            .messages
            .iter()
            .filter(|m| m.answers_call_id.is_some())
            .collect();
        assert_eq!(answers.len(), 2);
        assert_eq!(answers[0].role, MessageRole::User);
        assert!(
            answers[0].text.ends_with(". The binding; confidence: 5/5"),
            "{}",
            answers[0].text
        );
        assert!(!answers[0].text.contains("opt_secret_right"));
    }

    #[test]
    fn messages_map_roles_and_hide_model_facing_noise() {
        let mut s = Session::new("c1", Kind::Teach);
        s.messages.push(Msg::User("hi".into()));
        s.messages.push(Msg::Assistant {
            text: "Welcome".into(),
            calls: vec![Call {
                id: "v#0".into(),
                name: "visual".into(),
                args: json!({"secret": "args"}),
            }],
        });
        s.messages.push(Msg::Assistant {
            text: " ".into(),
            calls: vec![],
        });
        s.messages.push(Msg::ToolResult {
            call_id: "v#0".into(),
            name: "visual".into(),
            ok: true,
            text: "dispatched as job_1 \u{2014} continue".into(),
        });
        s.messages.push(Msg::ToolResult {
            call_id: "x".into(),
            name: "probe_done".into(),
            ok: true,
            text: "Probe complete. Handoff queued.".into(),
        });
        s.messages.push(Msg::Steer {
            id: "steer_j2".into(),
            text: "Background Research job j2 finished: {}".into(),
        });
        s.messages.push(Msg::Steer {
            id: "s1".into(),
            text: "focus on borrowing".into(),
        });
        s.messages.push(Msg::Compaction("earlier we met".into()));
        let dto = ClassroomViewDto::new(&record(&s), &[], idle());
        let roles: Vec<_> = dto
            .messages
            .iter()
            .map(|m| (m.id.as_str(), m.role))
            .collect();
        assert_eq!(
            roles,
            [
                ("m0", MessageRole::User),
                ("m1", MessageRole::Assistant),
                ("m3", MessageRole::Tool),
                ("m6", MessageRole::Steer),
                ("m7", MessageRole::Compaction),
            ]
        );
        assert_eq!(
            dto.messages[1].calls,
            [CallDto {
                id: "v#0".into(),
                name: "visual".into()
            }]
        );
        assert!(!serde_json::to_string(&dto).unwrap().contains("secret"));
        assert_eq!(
            dto.messages[2].text,
            "visual running in the background (job_1)"
        );
    }

    #[test]
    fn phases_plan_probe_progress_and_jobs() {
        let mut s = Session::new("c1", Kind::Teach);
        let rec = |s: &Session| ClassroomViewDto::new(&record(s), &[], idle());
        assert_eq!(rec(&s).phase, ChatPhase::Probe);
        assert_eq!(
            rec(&s).probe_progress,
            ProbeProgressDto { asked: 0, max: 8 }
        );
        s.messages.push(call("p#1", "probe", secret_quiz()));
        s.answer("p#1", "selected: opt_b; confidence: 3/5");
        assert_eq!(rec(&s).probe_progress.asked, 1);
        s.messages.push(Msg::ToolResult {
            call_id: "d".into(),
            name: "probe_done".into(),
            ok: true,
            text: "ok".into(),
        });
        assert_eq!(rec(&s).phase, ChatPhase::Plan);

        s.plan = Some(PlanTree {
            nodes: ["n1", "n2"]
                .iter()
                .map(|id| PlanNode {
                    id: (*id).into(),
                    title: format!("Title {id}"),
                    summary: String::new(),
                })
                .collect(),
            edges: vec![("n1".into(), "n2".into())],
            version: 1,
        });
        s.messages.push(Msg::ToolResult {
            call_id: "pl".into(),
            name: "plan_init".into(),
            ok: true,
            text: "ok".into(),
        });
        let mut q = secret_quiz();
        q["nodeId"] = json!("n1");
        s.messages.push(call("q#1", "quiz", q.clone()));
        let dto = rec(&s);
        assert_eq!(dto.phase, ChatPhase::Teach);
        let plan = dto.plan.unwrap();
        assert_eq!(plan.current_id.as_deref(), Some("n1"));
        let st: Vec<_> = plan.nodes.iter().map(|n| n.status).collect();
        assert_eq!(st, [PlanStatus::Current, PlanStatus::Upcoming]);
        assert_eq!(
            plan.edges,
            [PlanEdgeDto {
                from: "n1".into(),
                to: "n2".into()
            }]
        );

        // Reaching the last node with nothing pending ends the chat.
        s.answer("q#1", "selected: opt_b; confidence: 3/5");
        q["nodeId"] = json!("n2");
        s.messages.push(call("q#2", "quiz", q));
        s.answer("q#2", "selected: opt_b; confidence: 3/5");
        assert_eq!(rec(&s).phase, ChatPhase::Done);

        let job = JobRecord {
            id: "j1".into(),
            chat_id: "c1".into(),
            kind: JobKind::Visual,
            status: JobStatus::Done,
            delivery: lp_core::ports::DeliveryStatus::Claimed,
            created_at: 1,
            result: Some(json!({"title": "Borrow diagram", "artifactId": "a1"})),
            usage: vec![],
        };
        let other = JobRecord {
            chat_id: "c9".into(),
            id: "j2".into(),
            ..job.clone()
        };
        let dto = ClassroomViewDto::new(&record(&s), &[job, other], idle());
        assert_eq!(
            dto.jobs,
            [JobDto {
                id: "j1".into(),
                kind: "visual".into(),
                status: JobStatusDto::Done,
                title: "Borrow diagram".into(),
                artifact_id: Some("a1".into()),
            }]
        );
    }

    #[test]
    fn practice_chats_and_empty_records() {
        let mut s = Session::new("c1", Kind::Practice);
        let dto = ClassroomViewDto::new(&record(&s), &[], idle());
        assert_eq!(
            (dto.kind, dto.phase),
            (ChatKind::Practice, ChatPhase::Practice)
        );
        s.messages.push(call(
            "p#1",
            "problem",
            json!({"statement": "Reverse a list", "mode": "composition", "modelSolution": "x"}),
        ));
        let p = ClassroomViewDto::new(&record(&s), &[], idle())
            .pending
            .unwrap();
        let pr = p.problem.unwrap();
        assert_eq!(
            (pr.mode.as_str(), pr.statement.as_str()),
            ("composition", "Reverse a list")
        );
        assert!(p.options.is_none() && p.allow_free_text);
        s.messages.push(Msg::Answer {
            call_id: "p#1".into(),
            text: "done".into(),
        });
        s.messages.push(Msg::ToolResult {
            call_id: "c#1".into(),
            name: "conclude".into(),
            ok: true,
            text: "Model solution: iterate backwards".into(),
        });
        let dto = ClassroomViewDto::new(&record(&s), &[], idle());
        assert_eq!(dto.phase, ChatPhase::Done);
        assert_eq!(dto.messages.last().unwrap().role, MessageRole::Tool);

        // A record with no session yet reads as an empty teach chat.
        let mut empty = record(&Session::new("c2", Kind::Teach));
        empty.body = serde_json::Value::Null;
        let dto = ClassroomViewDto::new(&empty, &[], idle());
        assert_eq!(
            (dto.kind, dto.phase, dto.messages.len()),
            (ChatKind::Teach, ChatPhase::Probe, 0)
        );
        assert_eq!(dto.view_version, 42);
        let sum = ChatSummaryDto::new(&empty, &[]);
        assert_eq!((sum.message_count, sum.pending, sum.jobs), (0, false, 0));
        let j = serde_json::to_value(&dto).unwrap();
        assert_eq!(j["turn"]["state"], "idle");
        assert_eq!(j["probeProgress"]["max"], 8);
        assert_eq!(serde_json::from_value::<ClassroomViewDto>(j).unwrap(), dto);
    }

    #[test]
    fn summary_counts_pending_and_running_jobs() {
        let mut s = Session::new("c1", Kind::Teach);
        s.messages.push(call("q#1", "quiz", secret_quiz()));
        let running = JobRecord {
            id: "j1".into(),
            chat_id: "c1".into(),
            kind: JobKind::Research,
            status: JobStatus::Running,
            delivery: lp_core::ports::DeliveryStatus::Pending,
            created_at: 1,
            result: None,
            usage: vec![],
        };
        let done = JobRecord {
            id: "j2".into(),
            status: JobStatus::Done,
            ..running.clone()
        };
        let sum = ChatSummaryDto::new(&record(&s), &[running.clone(), done]);
        assert!(sum.pending);
        assert_eq!((sum.jobs, sum.message_count, sum.updated_at), (1, 1, 42));
        assert_eq!(JobDto::from(&running).title, "Research job");
    }
}
