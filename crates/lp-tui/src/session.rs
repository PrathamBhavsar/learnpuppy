//! Glue between a `lp_agent` teaching `Session` and what the UI and CLI show of it.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use lp_agent::loops::{run_turn, Call, Kind, Msg, Session, TurnError};
use lp_cli_driver::engine::{LlmEngine, StreamDelta};
use lp_core::plan::{derive_plan_statuses, Phase, PlanStatuses, QuizInput, ToolCall};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::views::classroom::{Answer, Classroom, Prompt, IDK};

/// Model slot that serves the next turn of `s`.
pub fn role(s: &Session) -> &'static str {
    match (s.kind, s.phase()) {
        (Kind::Practice, _) => "practice",
        (Kind::Teach, Phase::Probe) => "prober",
        (Kind::Teach, _) => "teacher",
    }
}

/// Runs turns until the session waits for the learner, delivering queued steers in between.
pub async fn run_to_rest<E: LlmEngine>(
    engine_for: &dyn Fn(&str) -> Arc<E>,
    root: &Path,
    s: &mut Session,
    events: &mpsc::Sender<StreamDelta>,
    cancel: &CancellationToken,
) -> Result<(), TurnError> {
    loop {
        let engine = engine_for(role(s));
        run_turn(engine.as_ref(), root, s, events, cancel).await?;
        if !s.wake() {
            return Ok(());
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

/// The text the model sees for a learner's answer.
pub fn render_answer(a: &Answer) -> String {
    let mut out = match a.value.as_str() {
        "" => String::new(),
        IDK => "selected: I don't know".to_owned(),
        v => format!("selected: {v}"),
    };
    for (label, text) in [
        ("reading notes", &a.reading_thinking),
        ("answer", &a.thinking),
    ] {
        if !text.trim().is_empty() {
            if !out.is_empty() {
                out.push_str("; ");
            }
            out.push_str(&format!("{label}: {}", text.trim()));
        }
    }
    format!("{out}; confidence: {}/5", a.confidence)
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

/// What the Classroom view shows; `streaming` is the in-flight assistant text.
pub fn classroom_view(s: &Session, streaming: &str) -> Classroom {
    let mut transcript: Vec<String> = s
        .messages
        .iter()
        .filter_map(|m| match m {
            Msg::User(t) => Some(format!("**You:** {t}")),
            Msg::Assistant { text, .. } => Some(text.clone()),
            Msg::Answer { text, .. } => Some(format!("**You answered:** {text}")),
            Msg::Compaction(t) => Some(format!("_Earlier:_ {t}")),
            Msg::Steer { text, .. } if text.starts_with(lp_agent::workbench::VISUAL_STEER) => {
                let (_, body) = text.split_once('\n')?;
                Some(format!("**Diagram:**\n```\n{body}\n```"))
            }
            Msg::ToolResult { .. } | Msg::Steer { .. } => None,
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
    Classroom {
        phase: s.phase(),
        practice: s.kind == Kind::Practice,
        transcript,
        plan: s.plan.clone(),
        statuses,
        prompt: pending_call(s).map(prompt),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_cli_driver::engine::FauxEngine;
    use serde_json::json;

    fn quiz() -> serde_json::Value {
        json!({
            "question": "Who owns it?",
            "options": [{"label": "One", "value": "a"}, {"label": "Two", "value": "b"}],
            "correctAnswer": "a",
            "explanation": "One owner."
        })
    }

    #[test]
    fn finished_visual_job_shows_its_diagram_in_the_transcript() {
        let mut s = Session::new("c1", Kind::Teach);
        s.messages.push(Msg::Steer {
            id: "steer_j1".into(),
            text: format!(
                "{}j1 finished:\n[a]-[b]\n\nread left to right",
                lp_agent::workbench::VISUAL_STEER
            ),
        });
        s.messages.push(Msg::Steer {
            id: "steer_j2".into(),
            text: "Background Research job j2 finished: {}".into(),
        });
        let t = classroom_view(&s, "").transcript;
        assert_eq!(t.len(), 1);
        assert!(t[0].contains("[a]-[b]") && t[0].contains("```"));
    }

    #[tokio::test]
    async fn turn_to_prompt_then_answer_clears_it() {
        let faux = Arc::new(FauxEngine::new());
        faux.faux_tool_call("probe", quiz());
        let engines = {
            let faux = faux.clone();
            move |_: &str| faux.clone()
        };
        let (tx, _rx) = mpsc::channel(512);
        let mut s = Session::new("c1", Kind::Teach);
        s.push_user("teach me ownership");
        run_to_rest(
            &engines,
            Path::new("."),
            &mut s,
            &tx,
            &CancellationToken::new(),
        )
        .await
        .unwrap();
        let cl = classroom_view(&s, "");
        assert_eq!(cl.phase, Phase::Probe);
        assert!(
            matches!(&cl.prompt, Some(Prompt::Quiz { input, .. }) if input.question == "Who owns it?")
        );
        assert!(cl.transcript[0].contains("teach me ownership"));

        let id = pending_call(&s).unwrap().id.clone();
        s.answer(
            &id,
            render_answer(&Answer {
                value: "a".into(),
                confidence: 4,
                reading_thinking: String::new(),
                thinking: "owner".into(),
            }),
        );
        assert!(classroom_view(&s, "").prompt.is_none());
        assert!(matches!(
            s.messages.last(),
            Some(Msg::Answer { text, .. }) if text == "selected: a; answer: owner; confidence: 4/5"
        ));
    }

    #[test]
    fn free_prompt_and_idk_rendering() {
        let mut s = Session::new("c", Kind::Practice);
        s.messages.push(Msg::Assistant {
            text: String::new(),
            calls: vec![Call {
                id: "p#0".into(),
                name: "problem".into(),
                args: json!({"statement": "Reverse a list"}),
            }],
        });
        assert_eq!(
            classroom_view(&s, "typing").prompt,
            Some(Prompt::Free {
                label: "problem".into(),
                question: "Reverse a list".into()
            })
        );
        let a = Answer {
            value: IDK.into(),
            confidence: 1,
            reading_thinking: String::new(),
            thinking: String::new(),
        };
        assert_eq!(render_answer(&a), "selected: I don't know; confidence: 1/5");
    }
}
