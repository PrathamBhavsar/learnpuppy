//! Stateless per-turn agent loops: every step rebuilds the agent and its prompt from the transcript.

mod practice;
mod prober;
mod teacher;
mod tools;

#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::path::Path;

use lp_cli_driver::engine::{DriverError, LlmEngine, StreamDelta, TurnRequest};
use lp_core::fsrs::{compaction_cut, CompactMsg, MsgKind};
use lp_core::plan::{heal_encoded_arguments, phase_at_3, Phase, PlanTree, ToolOutcome};
use lp_core::ports::{ChatRecord, ChatStore, StoreError, StoreResult};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::workbench::{GraphHandle, JobRequest};

pub use practice::PracticeAgent;
pub use prober::ProberAgent;
pub use teacher::TeacherAgent;

const SHARED_PROMPT: &str = include_str!("prompts/shared.md");

/// Tool calls the model may chain in one turn before we give up.
const MAX_STEPS: usize = 16;
/// Consecutive invalid tool calls tolerated before the turn fails.
const MAX_INVALID: usize = 3;
pub const HANDOFF_TEXT: &str = "The probe is finished. Judge the learner from the probe transcript, call plan_init to author the plan, then begin teaching.";
pub const COMPACTED_HISTORY_HEADER: &str = "Summary of earlier conversation:";

/// Tools that put a question to the learner; the answer arrives as the next message.
const QUESTION_TOOLS: [&str; 6] = [
    "probe",
    "recall",
    "quiz",
    "explain",
    "ask_user_question",
    "problem",
];

fn is_question(name: &str) -> bool {
    QUESTION_TOOLS.contains(&name)
}

/// A turn stops at the first tool that waits for the learner or concludes a problem.
fn ends_turn(name: &str) -> bool {
    is_question(name) || name == "conclude"
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Call {
    pub id: String,
    pub name: String,
    pub args: Value,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Msg {
    User(String),
    Assistant {
        text: String,
        calls: Vec<Call>,
    },
    ToolResult {
        call_id: String,
        name: String,
        ok: bool,
        text: String,
    },
    Answer {
        call_id: String,
        text: String,
    },
    Steer {
        id: String,
        text: String,
    },
    Compaction(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Kind {
    Teach,
    Practice,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Session {
    pub id: String,
    pub kind: Kind,
    pub messages: Vec<Msg>,
    pub plan: Option<PlanTree>,
    /// Handoff steers queued by tools, delivered by `wake`.
    steers: Vec<(String, String)>,
    problem_active: bool,
    /// Workbench dispatches queued by tools, started by `Workbench::launch`.
    #[serde(default)]
    pub(crate) requests: Vec<JobRequest>,
    /// Graph behind the `list_*` tools; attached at runtime, never persisted.
    #[serde(skip)]
    pub(crate) graph: Option<GraphHandle>,
}

impl Session {
    pub fn new(id: impl Into<String>, kind: Kind) -> Self {
        Self {
            id: id.into(),
            kind,
            messages: Vec::new(),
            plan: None,
            steers: Vec::new(),
            problem_active: false,
            requests: Vec::new(),
            graph: None,
        }
    }

    pub(crate) fn queue_steer(&mut self, id: String, text: String) {
        self.steers.push((id, text));
    }

    pub fn push_user(&mut self, text: impl Into<String>) {
        self.messages.push(Msg::User(text.into()));
    }

    /// Record the learner's answer (selection, confidence and thinking, pre-rendered) to a question call.
    pub fn answer(&mut self, call_id: &str, text: impl Into<String>) {
        self.messages.push(Msg::Answer {
            call_id: call_id.into(),
            text: text.into(),
        });
    }

    pub fn outcomes(&self) -> Vec<ToolOutcome> {
        self.messages
            .iter()
            .filter_map(|m| match m {
                Msg::ToolResult { name, ok, .. } => Some(ToolOutcome {
                    name: name.clone(),
                    succeeded: *ok,
                }),
                _ => None,
            })
            .collect()
    }

    pub fn phase(&self) -> Phase {
        phase_at_3(&self.outcomes())
    }

    fn calls(&self) -> impl Iterator<Item = &Call> {
        self.messages.iter().flat_map(|m| match m {
            Msg::Assistant { calls, .. } => calls.as_slice(),
            _ => &[],
        })
    }

    /// A question call that has neither an answer nor a (failed) tool result.
    pub fn pending_question(&self) -> bool {
        let resolved: HashSet<&str> = self
            .messages
            .iter()
            .filter_map(|m| match m {
                Msg::Answer { call_id, .. } | Msg::ToolResult { call_id, .. } => {
                    Some(call_id.as_str())
                }
                _ => None,
            })
            .collect();
        self.calls()
            .any(|c| is_question(&c.name) && !resolved.contains(c.id.as_str()))
    }

    /// Deliver queued steers once no question is waiting; true if the transcript changed.
    pub fn wake(&mut self) -> bool {
        if self.pending_question() || self.steers.is_empty() {
            return false;
        }
        for (id, text) in std::mem::take(&mut self.steers) {
            self.messages.push(Msg::Steer { id, text });
        }
        true
    }

    /// Build this turn's agent from the transcript alone.
    pub fn agent(&self, root: &Path) -> Box<dyn Agent> {
        let root = root.to_path_buf();
        match (self.kind, self.phase()) {
            (Kind::Practice, _) => Box::new(PracticeAgent { root }),
            (Kind::Teach, Phase::Probe) => Box::new(ProberAgent { root }),
            (Kind::Teach, _) => Box::new(TeacherAgent { root }),
        }
    }

    /// Start of the retained tail when the active context is over the compaction threshold.
    pub fn compaction_cut(&self) -> Option<usize> {
        let msgs: Vec<CompactMsg> = self
            .messages
            .iter()
            .map(|m| CompactMsg {
                kind: match m {
                    Msg::Assistant { calls, .. } if calls.iter().any(|c| is_question(&c.name)) => {
                        MsgKind::QuizCall
                    }
                    Msg::Answer { .. } => MsgKind::QuizAnswer,
                    _ => MsgKind::Other,
                },
                tokens: render_msg(m).chars().count() as u64 / 4 + 1,
            })
            .collect();
        let cut = compaction_cut(&msgs);
        // A lone earlier summary is not worth summarizing again.
        self.messages[..cut]
            .iter()
            .any(|m| !matches!(m, Msg::Compaction(_)))
            .then_some(cut)
    }

    /// Replace `messages[..cut]` with the summary produced for them.
    pub fn apply_compaction(&mut self, cut: usize, summary: String) {
        self.messages.splice(..cut, [Msg::Compaction(summary)]);
    }

    pub fn save(&self, store: &dyn ChatStore, now: i64) -> StoreResult<()> {
        let body = serde_json::to_value(self).map_err(|e| StoreError::Backend(e.to_string()))?;
        let mut rec = match store.load_chat(&self.id) {
            Ok(rec) => rec,
            Err(StoreError::NotFound(_)) => ChatRecord {
                id: self.id.as_str().into(),
                title: self.id.clone(),
                created_at: now,
                updated_at: now,
                body: Value::Null,
                turns: Vec::new(),
                extractions: Vec::new(),
            },
            Err(e) => return Err(e),
        };
        rec.body = body;
        rec.updated_at = now;
        store.save_chat(&rec)
    }

    pub fn load(store: &dyn ChatStore, id: &str) -> StoreResult<Self> {
        serde_json::from_value(store.load_chat(id)?.body)
            .map_err(|e| StoreError::Backend(e.to_string()))
    }
}

pub struct ToolSpec {
    pub name: &'static str,
    pub usage: &'static str,
}

/// What a tool did. `Err` carries validation problems back to the model for a retry.
pub enum Effect {
    /// Result text for the model; the turn continues.
    Ack(String),
    /// Question put to the learner; the turn ends and no result is recorded until they answer.
    Await,
    /// Result text; the turn ends.
    End(String),
}

pub trait Agent {
    fn role_prompt(&self, s: &Session) -> String;
    fn tools(&self) -> Vec<&'static ToolSpec>;
    fn exec(&self, s: &mut Session, call: &Call) -> Result<Effect, String>;
}

#[derive(Debug, thiserror::Error)]
pub enum TurnError {
    #[error(transparent)]
    Driver(#[from] DriverError),
    #[error("model made {MAX_INVALID} invalid tool calls in a row")]
    TooManyInvalid,
    #[error("turn exceeded {MAX_STEPS} steps")]
    StepLimit,
}

#[derive(Debug, PartialEq)]
pub struct TurnOutcome {
    /// Last assistant text.
    pub text: String,
    pub pending_question: bool,
    /// Set when the caller should start a background compaction of `messages[..cut]`.
    pub compact_at: Option<usize>,
}

fn render_msg(m: &Msg) -> String {
    match m {
        Msg::User(t) => format!("[learner] {t}"),
        Msg::Assistant { text, calls } => {
            let mut out = format!("[assistant] {text}");
            for c in calls {
                out.push_str(&format!("\n  call {} {} {}", c.name, c.id, c.args));
            }
            out
        }
        Msg::ToolResult {
            call_id,
            name,
            ok,
            text,
        } => format!(
            "[tool result {name} {call_id} {}] {text}",
            if *ok { "ok" } else { "error" }
        ),
        Msg::Answer { call_id, text } => format!("[learner answer {call_id}] {text}"),
        Msg::Steer { id, text } => format!("[steer {id}] {text}"),
        Msg::Compaction(t) => format!("[{COMPACTED_HISTORY_HEADER}] {t}"),
    }
}

fn render_prompt(agent: &dyn Agent, s: &Session) -> String {
    let mut out = format!("{SHARED_PROMPT}\n{}\n\nTools:\n", agent.role_prompt(s));
    for t in agent.tools() {
        out.push_str(&format!("- {}: {}\n", t.name, t.usage));
    }
    out.push_str("\nTranscript:\n");
    for m in &s.messages {
        out.push_str(&render_msg(m));
        out.push('\n');
    }
    out
}

/// Run one learner-visible turn: model steps and tool calls until a question, a conclusion or plain text.
pub async fn run_turn<E: LlmEngine>(
    engine: &E,
    root: &Path,
    s: &mut Session,
    events: &mpsc::Sender<StreamDelta>,
    cancel: &CancellationToken,
) -> Result<TurnOutcome, TurnError> {
    let mut invalid = 0;
    for _ in 0..MAX_STEPS {
        let agent = s.agent(root);
        let req = TurnRequest {
            prompt: render_prompt(agent.as_ref(), s),
            model: None,
            tools: agent
                .tools()
                .iter()
                .map(|t| lp_cli_driver::engine::ToolDecl {
                    name: t.name.into(),
                    usage: t.usage.into(),
                })
                .collect(),
        };
        let resp = engine
            .stream_turn(req, events.clone(), cancel.clone())
            .await?;
        // Engine ids may repeat across steps; the index keeps them unique for answer matching.
        let mut calls: Vec<Call> = resp
            .tool_calls
            .into_iter()
            .map(|c| Call {
                id: format!("{}#{}", c.id, s.messages.len()),
                name: c.name,
                args: heal_encoded_arguments(c.input),
            })
            .collect();
        if let Some(i) = calls.iter().position(|c| ends_turn(&c.name)) {
            calls.truncate(i + 1);
        }
        s.messages.push(Msg::Assistant {
            text: resp.text.clone(),
            calls: calls.clone(),
        });
        let mut stop = calls.is_empty();
        for c in &calls {
            let (ok, text) = match agent.exec(s, c) {
                Ok(Effect::Await) => {
                    invalid = 0;
                    stop = true;
                    continue;
                }
                Ok(Effect::Ack(t)) => (true, t),
                Ok(Effect::End(t)) => {
                    stop = true;
                    (true, t)
                }
                Err(e) => (false, e),
            };
            invalid = if ok { 0 } else { invalid + 1 };
            s.messages.push(Msg::ToolResult {
                call_id: c.id.clone(),
                name: c.name.clone(),
                ok,
                text,
            });
            if invalid >= MAX_INVALID {
                return Err(TurnError::TooManyInvalid);
            }
        }
        if stop {
            return Ok(TurnOutcome {
                text: resp.text,
                pending_question: s.pending_question(),
                compact_at: s.compaction_cut(),
            });
        }
    }
    Err(TurnError::StepLimit)
}
