//! `GET …/chats/{id}/export.md`: the chat as a Markdown transcript. Questions are listed the way
//! the learner saw them (shuffled options); answers carry their verdict. A pending question never
//! shows its answer.

use lp_agent::loops::{Call, Msg, Session};
use lp_core::plan::{shuffle_options, QuizInput};
use lp_core::ports::ChatRecord;

use super::answer::{verdict, IDK};
use super::meta::ChatMeta;

fn when(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .map(|t| t.format("%Y-%m-%d %H:%M UTC").to_string())
        .unwrap_or_default()
}

fn quote(text: &str) -> String {
    text.lines()
        .map(|l| format!("> {l}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn question(c: &Call) -> Option<String> {
    let label = match c.name.as_str() {
        "probe" => "Check",
        "quiz" => "Quiz",
        "recall" => "Recall",
        "explain" => "Explain",
        "ask_user_question" => "Question",
        "problem" => "Problem",
        _ => return None,
    };
    let field = |k: &str| c.args.get(k).and_then(|v| v.as_str()).unwrap_or_default();
    let body = match c.name.as_str() {
        "problem" => field("statement"),
        "explain" => field("content"),
        _ => field("question"),
    };
    let mut out = format!("**{label}:** {}", body.trim());
    if let Ok(q) = serde_json::from_value::<QuizInput>(c.args.clone()) {
        for (i, o) in shuffle_options(&q.options, &c.id).iter().enumerate() {
            out.push_str(&format!("\n- {}. {}", char::from(b'A' + i as u8), o.label));
        }
    }
    Some(quote(&out))
}

fn answer_line(s: &Session, call_id: &str, text: &str) -> String {
    let call = s.messages.iter().find_map(|m| match m {
        Msg::Assistant { calls, .. } => calls.iter().find(|c| c.id == call_id),
        _ => None,
    });
    let quiz = call.and_then(|c| serde_json::from_value::<QuizInput>(c.args.clone()).ok());
    let shown = text.replace(&format!("selected: {IDK}"), "selected: I don't know");
    let mut out = format!("**You answered:** {shown}");
    if let Some(v) = quiz.and_then(|q| verdict(&q, call_id, text)) {
        out.push_str(&format!("\n\n{v}"));
    }
    out
}

/// The whole chat as Markdown.
pub fn export_markdown(rec: &ChatRecord, s: &Session) -> String {
    let meta = ChatMeta::of(&rec.body);
    let kind = match s.kind {
        lp_agent::loops::Kind::Teach => "Teach",
        lp_agent::loops::Kind::Practice => "Practice",
    };
    let mut out = format!(
        "# {}\n\n_{kind} chat · started {} · last activity {}_\n",
        rec.title.trim(),
        when(rec.created_at),
        when(rec.updated_at)
    );
    for (i, m) in s.messages.iter().enumerate() {
        let block = match m {
            Msg::User(t) => Some(format!("**You:** {}", t.trim())),
            Msg::Assistant { text, calls } => {
                let mut parts: Vec<String> = Vec::new();
                if !text.trim().is_empty() {
                    let mut t = format!("**Tutor:** {}", text.trim());
                    if meta.is_interrupted(i) {
                        t.push_str(" _(interrupted)_");
                    }
                    parts.push(t);
                }
                parts.extend(calls.iter().filter_map(question));
                (!parts.is_empty()).then(|| parts.join("\n\n"))
            }
            Msg::Answer { call_id, text } => Some(answer_line(s, call_id, text)),
            Msg::ToolResult {
                name,
                ok: true,
                text,
                ..
            } if name == "conclude" => Some(format!("_{}_", text.trim())),
            Msg::ToolResult { name, text, .. } if text.starts_with("dispatched as ") => {
                let job = text["dispatched as ".len()..]
                    .split_whitespace()
                    .next()
                    .unwrap_or_default();
                Some(format!("_{name} running in the background ({job})_"))
            }
            Msg::ToolResult { .. } => None,
            Msg::Steer { text, .. } => text
                .strip_prefix(lp_agent::workbench::VISUAL_STEER)
                .and_then(|t| t.split_once('\n'))
                .map(|(_, body)| format!("**Diagram:**\n\n```\n{body}\n```"))
                .or_else(|| {
                    (!text.starts_with("Background ") && !text.trim().is_empty())
                        .then(|| format!("**Guidance:** {}", text.trim()))
                }),
            Msg::Compaction(t) => Some(format!(
                "---\n\n_Earlier conversation, summarized:_ {}\n\n---",
                t.trim()
            )),
        };
        if let Some(b) = block {
            out.push('\n');
            if let Some(t) = meta.time(i) {
                out.push_str(&format!("<!-- {} -->\n", when(t)));
            }
            out.push_str(&b);
            out.push('\n');
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_agent::loops::Kind;
    use serde_json::json;

    fn rec(s: &Session) -> ChatRecord {
        let mut meta = ChatMeta::default();
        meta.stamp(s, 1_700_000_000_000);
        ChatRecord {
            id: s.id.as_str().into(),
            title: "Ownership".into(),
            created_at: 1_700_000_000_000,
            updated_at: 1_700_000_100_000,
            body: meta.body(s).unwrap(),
            turns: vec![],
            extractions: vec![],
        }
    }

    #[test]
    fn exports_questions_answers_and_verdicts_without_leaking_pending_answers() {
        let mut s = Session::new("c1", Kind::Teach);
        s.push_user("teach me ownership");
        let quiz = json!({"question": "Who owns it?", "options": [{"label": "One", "value": "a"}, {"label": "Two", "value": "b"}],
                          "correctAnswer": "a", "explanation": "SECRET one owner"});
        s.messages.push(Msg::Assistant {
            text: "Let's check.".into(),
            calls: vec![Call {
                id: "q#1".into(),
                name: "probe".into(),
                args: quiz.clone(),
            }],
        });
        let pending = export_markdown(&rec(&s), &s);
        assert!(pending.starts_with("# Ownership\n"));
        assert!(pending.contains("**You:** teach me ownership"));
        assert!(pending.contains("> **Check:** Who owns it?"));
        assert!(pending.contains(". One") && pending.contains(". Two"));
        assert!(!pending.contains("SECRET"), "{pending}");
        s.answer("q#1", "selected: a; confidence: 5/5");
        let done = export_markdown(&rec(&s), &s);
        assert!(done.contains("**You answered:** selected: a"));
        assert!(done.contains("**Correct.** SECRET one owner"));
        assert!(done.contains("<!-- 2023-11-14 22:13 UTC -->"));
    }
}
