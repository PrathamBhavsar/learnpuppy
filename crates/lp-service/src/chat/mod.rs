//! The classroom: turn loop, the view of a session, and learner answers.

pub mod answer;
pub mod turn;
pub mod view;

pub use answer::{option_lines, render_answer, resolve_option, verdict, Answer, IDK};
pub use turn::{role, run_to_rest};
pub use view::{classroom_view, pending_call, ClassroomView, Prompt};

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Arc;

    use lp_agent::loops::{Call, Kind, Msg, Session};
    use lp_cli_driver::engine::FauxEngine;
    use lp_core::plan::{Phase, QuizInput};
    use serde_json::json;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

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

    #[test]
    fn job_notice_is_its_own_transcript_line() {
        let mut s = Session::new("c1", Kind::Teach);
        s.messages.push(Msg::ToolResult {
            call_id: "v#0".into(),
            name: "visual".into(),
            ok: true,
            text: "dispatched as job_1 \u{2014} continue your turn".into(),
        });
        s.messages.push(Msg::Assistant {
            text: String::new(),
            calls: vec![Call {
                id: "q#1".into(),
                name: "quiz".into(),
                args: json!({"question": "Who owns it?"}),
            }],
        });
        let cl = classroom_view(&s, "");
        assert_eq!(
            cl.transcript,
            ["_visual running in the background (job_1)_"]
        );
        assert!(
            matches!(&cl.prompt, Some(Prompt::Free { question, .. }) if question == "Who owns it?")
        );
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
        let t = classroom_view(&s, "").transcript;
        assert_eq!(
            t.last().unwrap(),
            "**Correct.** One owner. Confidence 4/5: well calibrated."
        );
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

    #[test]
    fn wrong_answer_gets_its_own_incorrect_line_after_the_answer() {
        let mut s = Session::new("c1", Kind::Teach);
        s.messages.push(Msg::Assistant {
            text: String::new(),
            calls: vec![Call {
                id: "q#1".into(),
                name: "quiz".into(),
                args: json!({
                    "question": "Who owns it?",
                    "options": [{"label": "One", "value": "a"}, {"label": "Two", "value": "b"}],
                    "correctAnswer": "a",
                    "explanation": ""
                }),
            }],
        });
        s.answer("q#1", "selected: b; confidence: 5/5");
        let t = classroom_view(&s, "").transcript;
        assert_eq!(t.len(), 2);
        assert!(t[0].starts_with("**You answered:**"));
        assert_eq!(
            t[1],
            "**Incorrect.** The right answer: B. One. Confidence 5/5: overconfident."
        );
    }

    #[test]
    fn typed_answers_resolve_by_letter_number_label_and_prefix() {
        let q: QuizInput = serde_json::from_value(quiz()).unwrap();
        let lines = option_lines(&q, "q#1");
        assert!(lines.ends_with("  I. I don't know\n"));
        let first = lines.lines().next().unwrap().trim();
        let first = if first == "A. One" { "a" } else { "b" };
        for typed in ["a", "A)", "a.", "1"] {
            assert_eq!(
                resolve_option(&q, "q#1", typed).as_deref(),
                Some(first),
                "{typed}"
            );
        }
        assert_eq!(resolve_option(&q, "q#1", "One").as_deref(), Some("a"));
        assert_eq!(resolve_option(&q, "q#1", "tw").as_deref(), Some("b"));
        assert_eq!(resolve_option(&q, "q#1", "i").as_deref(), Some(IDK));
        assert_eq!(
            resolve_option(&q, "q#1", "I don't know").as_deref(),
            Some(IDK)
        );
        for bad in ["x", "3", "", "zebra"] {
            assert_eq!(resolve_option(&q, "q#1", bad), None, "{bad}");
        }
    }

    #[test]
    fn role_follows_kind_and_phase() {
        assert_eq!(role(&Session::new("c", Kind::Teach)), "prober");
        assert_eq!(role(&Session::new("c", Kind::Practice)), "practice");
    }
}
