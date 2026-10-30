//! What the classroom shows of a `Session`: transcript lines, plan statuses and the pending prompt.

use std::collections::HashSet;

use lp_agent::loops::{Call, Kind, Msg, Session};
use lp_core::plan::{derive_plan_statuses, Phase, PlanStatuses, PlanTree, QuizInput, ToolCall};

use super::answer::verdict;

/// What the teacher is waiting on. `Quiz` covers `probe` and `quiz`; `Free` covers `explain`,
/// `recall`, `ask_user_question` and `problem`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Prompt {
    Quiz { call_id: String, input: QuizInput },
    Free { label: String, question: String },
}

/// The classroom of one chat: phase, transcript, plan with derived statuses, pending prompt.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClassroomView {
    pub phase: Phase,
    pub practice: bool,
    /// One markdown string per message.
    pub transcript: Vec<String>,
    pub plan: Option<PlanTree>,
    pub statuses: PlanStatuses,
    pub prompt: Option<Prompt>,
}

impl Default for ClassroomView {
    fn default() -> Self {
        Self {
            phase: Phase::Probe,
            practice: false,
            transcript: Vec::new(),
            plan: None,
            statuses: PlanStatuses::default(),
            prompt: None,
        }
    }
}

fn is_question(name: &str) -> bool {
    matches!(
        name,
        "probe" | "recall" | "quiz" | "explain" | "ask_user_question" | "problem"
    )
}

fn calls(s: &Session) -> impl Iterator<Item = &Call> {
    s.messages.iter().flat_map(|m| match m {
        Msg::Assistant { calls, .. } => calls.as_slice(),
        _ => &[],
    })
}

/// The latest question call the learner has not answered yet.
pub fn pending_call(s: &Session) -> Option<&Call> {
    let resolved: HashSet<&str> = s
        .messages
        .iter()
        .filter_map(|m| match m {
            Msg::Answer { call_id, .. } | Msg::ToolResult { call_id, .. } => Some(call_id.as_str()),
            _ => None,
        })
        .collect();
    calls(s)
        .filter(|c| is_question(&c.name) && !resolved.contains(c.id.as_str()))
        .last()
}

fn prompt(c: &Call) -> Prompt {
    if matches!(c.name.as_str(), "probe" | "quiz") {
        if let Ok(input) = serde_json::from_value::<QuizInput>(c.args.clone()) {
            return Prompt::Quiz {
                call_id: c.id.clone(),
                input,
            };
        }
    }
    let field = |k: &str| c.args.get(k).and_then(|v| v.as_str());
    Prompt::Free {
        label: c.name.clone(),
        question: field("question")
            .or_else(|| field("statement"))
            .or_else(|| field("content"))
            .unwrap_or_default()
            .to_owned(),
    }
}

/// One transcript line per message; `None` for messages the learner does not see.
fn transcript_item(m: &Msg) -> Option<String> {
    match m {
        Msg::User(t) => Some(format!("**You:** {t}")),
        Msg::Assistant { text, .. } => Some(text.clone()),
        Msg::Answer { text, .. } => Some(format!("**You answered:** {text}")),
        Msg::Compaction(t) => Some(format!("_Earlier:_ {t}")),
        Msg::Steer { text, .. } if text.starts_with(lp_agent::workbench::VISUAL_STEER) => {
            let (_, body) = text.split_once('\n')?;
            Some(format!("**Diagram:**\n```\n{body}\n```"))
        }
        // Job notices are their own line, never part of a question.
        Msg::ToolResult { name, text, .. } if text.starts_with("dispatched as ") => {
            let job = text["dispatched as ".len()..].split_whitespace().next()?;
            Some(format!("_{name} running in the background ({job})_"))
        }
        Msg::ToolResult { .. } | Msg::Steer { .. } => None,
    }
}

/// What the classroom shows of `s`; `streaming` is the in-flight assistant text.
pub fn classroom_view(s: &Session, streaming: &str) -> ClassroomView {
    let mut transcript: Vec<String> = s
        .messages
        .iter()
        .flat_map(|m| {
            let verdict = match m {
                Msg::Answer { call_id, text } => calls(s)
                    .find(|c| c.id == *call_id)
                    .and_then(|c| serde_json::from_value::<QuizInput>(c.args.clone()).ok())
                    .and_then(|q| verdict(&q, call_id, text)),
                _ => None,
            };
            transcript_item(m).into_iter().chain(verdict)
        })
        .filter(|t| !t.trim().is_empty())
        .collect();
    if !streaming.is_empty() {
        transcript.push(streaming.to_owned());
    }
    let statuses = s.plan.as_ref().map_or_else(PlanStatuses::default, |tree| {
        let made: Vec<ToolCall> = calls(s)
            .map(|c| ToolCall {
                name: c.name.clone(),
                args: c.args.clone(),
            })
            .collect();
        derive_plan_statuses(tree, &made)
    });
    ClassroomView {
        phase: s.phase(),
        practice: s.kind == Kind::Practice,
        transcript,
        plan: s.plan.clone(),
        statuses,
        prompt: pending_call(s).map(prompt),
    }
}
