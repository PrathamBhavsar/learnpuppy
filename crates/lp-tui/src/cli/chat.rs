//! `learnpuppy chat new | step`.

use std::io::Write;
use std::path::Path;
use std::sync::Arc;

use anyhow::{bail, Context};
use clap::{Args, Subcommand};
use lp_agent::loops::{Kind, Session};
use lp_cli_driver::engine::LlmEngine;
use lp_core::plan::Phase;
use lp_core::ports::ChatStore;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::session::{classroom_view, pending_call, render_answer, run_to_rest};
use crate::state::now_ms;
use crate::views::classroom::{Answer, Prompt};

#[derive(Subcommand)]
pub enum Cmd {
    /// Start a session on a topic and run its first turn.
    New {
        topic: String,
        #[command(flatten)]
        mode: Mode,
    },
    /// Answer the pending question of a session and run the next turn.
    Step {
        id: String,
        answer: String,
        /// Confidence 1..5.
        #[arg(long, default_value_t = 3)]
        confidence: u8,
    },
}

#[derive(Args)]
pub struct Mode {
    #[arg(long, conflicts_with = "practice")]
    teach: bool,
    #[arg(long)]
    practice: bool,
}

const STREAM_DELTA_CAP: usize = 512;

pub async fn run<E: LlmEngine>(
    chats: &dyn ChatStore,
    engine_for: &dyn Fn(&str) -> Arc<E>,
    root: &Path,
    cmd: Cmd,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let mut s = match &cmd {
        Cmd::New { topic, mode } => {
            let kind = if mode.practice {
                Kind::Practice
            } else {
                Kind::Teach
            };
            let mut s = Session::new(format!("c_{:x}", now_ms()), kind);
            s.push_user(topic);
            s.save(chats, now_ms() / 1000)?;
            let mut rec = chats.load_chat(&s.id)?;
            rec.title.clone_from(topic);
            chats.save_chat(&rec)?;
            s
        }
        Cmd::Step {
            id,
            answer,
            confidence,
        } => {
            let mut s = Session::load(chats, id).with_context(|| format!("chat {id}"))?;
            let Some(call_id) = pending_call(&s).map(|c| c.id.clone()) else {
                bail!("chat {id} has no pending question");
            };
            let text = render_answer(&Answer {
                value: String::new(),
                confidence: (*confidence).clamp(1, 5),
                reading_thinking: String::new(),
                thinking: answer.clone(),
            });
            s.answer(&call_id, text);
            s
        }
    };
    let (tx, mut rx) = mpsc::channel(STREAM_DELTA_CAP);
    // Deltas are not printed live; draining keeps the bounded channel from stalling the turn.
    let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
    let res = run_to_rest(engine_for, root, &mut s, &tx, &CancellationToken::new()).await;
    drop(tx);
    let _ = drain.await;
    s.save(chats, now_ms() / 1000)?;
    res?;

    let view = classroom_view(&s, "");
    writeln!(out, "chat {}", s.id)?;
    write!(out, "phase {:?}", s.phase())?;
    if s.phase() == Phase::Probe {
        let max = lp_agent::loops::MAX_PROBE_QUESTIONS;
        write!(out, ", probe question {}/{max}", s.probe_questions())?;
    }
    writeln!(out)?;
    if let Some(last) = view.transcript.last() {
        writeln!(out, "{last}")?;
    }
    match view.prompt {
        Some(Prompt::Quiz { input, .. }) => {
            writeln!(out, "\n? {}", input.question)?;
            for o in input.options {
                writeln!(out, "  {} ({})", o.label, o.value)?;
            }
        }
        Some(Prompt::Free { question, .. }) => writeln!(out, "\n? {question}")?,
        None => {}
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_cli_driver::engine::FauxEngine;
    use lp_store::FsChatStore;
    use serde_json::json;

    #[tokio::test]
    async fn new_then_step_persists_and_prints_the_question() {
        let d = tempfile::tempdir().unwrap();
        let chats = FsChatStore::open(d.path()).unwrap();
        let faux = Arc::new(FauxEngine::new());
        let engine_for = {
            let faux = faux.clone();
            move |_: &str| faux.clone()
        };
        faux.faux_tool_call(
            "probe",
            json!({
                "question": "Who owns it?",
                "options": [{"label": "One", "value": "a"}, {"label": "Two", "value": "b"}],
                "correctAnswer": "a",
                "explanation": "One owner."
            }),
        );
        let mut out = Vec::new();
        let new = Cmd::New {
            topic: "ownership".into(),
            mode: Mode {
                teach: true,
                practice: false,
            },
        };
        run(&chats, &engine_for, d.path(), new, &mut out)
            .await
            .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("? Who owns it?") && text.contains("One (a)"));
        assert!(text.contains("phase Probe, probe question 1/8"), "{text}");
        let id = chats.list_chats().unwrap()[0].id.to_string();
        assert_eq!(chats.load_chat(&id).unwrap().title, "ownership");

        faux.faux_text("Good.");
        let step = Cmd::Step {
            id: id.clone(),
            answer: "a".into(),
            confidence: 9,
        };
        run(&chats, &engine_for, d.path(), step, &mut Vec::new())
            .await
            .unwrap();
        let s = Session::load(&chats, &id).unwrap();
        assert!(pending_call(&s).is_none());
        let again = Cmd::Step {
            id,
            answer: "x".into(),
            confidence: 3,
        };
        assert!(run(&chats, &engine_for, d.path(), again, &mut Vec::new())
            .await
            .is_err());
    }
}
