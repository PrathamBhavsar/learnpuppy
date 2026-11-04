//! Learner answers: how they read to the model, how typed input names an option, and the verdict.

use lp_core::plan::{shuffle_options, QuizInput, QuizOption};

/// Value sent as the answer when the learner picks "I don't know".
pub const IDK: &str = "__idk__";

/// What the learner submitted for the pending question.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    /// Picked option value, `IDK`, or empty for free-form prompts.
    pub value: String,
    pub confidence: u8,
    pub reading_thinking: String,
    pub thinking: String,
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

/// A, B, C… for option `i`, the letter the dock and the transcript recap show.
pub fn letter(i: usize) -> char {
    char::from(b'A' + i as u8)
}

/// How one option value reads to the learner on the wire: `B. the label`, `I don't know`, or the
/// raw value when the call no longer names it.
pub fn option_label(shown: &[QuizOption], value: &str) -> String {
    if value == IDK || value == "I don't know" {
        return "I don't know".to_owned();
    }
    shown.iter().position(|o| o.value == value).map_or_else(
        || value.to_owned(),
        |i| format!("{}. {}", letter(i), shown[i].label),
    )
}

/// The confidence the learner attached to an answer, `N` of `N/5`.
pub fn confidence_in(answer: &str) -> Option<u8> {
    answer
        .rsplit("confidence: ")
        .next()
        .and_then(|c| c.split('/').next())
        .and_then(|c| c.trim().parse::<u8>().ok())
}

/// The options as the learner sees them (shuffled per call, as in the dock), one `A. label` per line.
pub fn option_lines(q: &QuizInput, call_id: &str) -> String {
    let mut out: String = shuffle_options(&q.options, call_id)
        .iter()
        .enumerate()
        .map(|(i, o)| format!("  {}. {}\n", letter(i), o.label))
        .collect();
    out.push_str("  I. I don't know\n");
    out
}

/// The option value (or `IDK`) a typed answer names: its letter (`b`, `B)`, `b.`), 1-based number,
/// exact value, label, or a unique prefix of a label or value. `None` when it names none or several.
pub fn resolve_option(q: &QuizInput, call_id: &str, input: &str) -> Option<String> {
    let t = input.trim();
    let key = t.strip_suffix([')', '.']).unwrap_or(t).trim();
    let shown = shuffle_options(&q.options, call_id);
    let n = shown.len();
    let by_index = |i: usize| (i < n).then(|| shown[i].value.clone());
    if let [c] = key.as_bytes() {
        let i = usize::from(c.to_ascii_uppercase().wrapping_sub(b'A'));
        if i < n {
            return by_index(i);
        }
        if c.eq_ignore_ascii_case(&b'i') {
            return Some(IDK.to_owned());
        }
    }
    if let Some(i) = key.parse::<usize>().ok().and_then(|k| k.checked_sub(1)) {
        return by_index(i);
    }
    if key.eq_ignore_ascii_case("i don't know") {
        return Some(IDK.to_owned());
    }
    let exact = |f: fn(&QuizOption) -> &str| shown.iter().find(|o| f(o).eq_ignore_ascii_case(t));
    if let Some(o) = exact(|o| &o.value).or_else(|| exact(|o| &o.label)) {
        return Some(o.value.clone());
    }
    let low = t.to_ascii_lowercase();
    let mut hits = shown.iter().filter(|o| {
        !low.is_empty()
            && (o.label.to_ascii_lowercase().starts_with(&low)
                || o.value.to_ascii_lowercase().starts_with(&low))
    });
    match (hits.next(), hits.next()) {
        (Some(o), None) => Some(o.value.clone()),
        _ => None,
    }
}

/// The machine-readable grade behind [`verdict`]: what the learner picked, what was right and how
/// sure they were. The transcript recap (docs/web/03 `answers`) renders from this, so option
/// shuffling and grading stay in Rust (CLAUDE.md).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Grade {
    pub correct: bool,
    /// Option values the learner picked, in the order they were sent.
    pub picked: Vec<String>,
    /// The option values that are right.
    pub right: Vec<String>,
    pub confidence: Option<u8>,
}

/// Grade a quiz answer. `None` when the answer carries no selected option (free-form probes).
pub fn grade(q: &QuizInput, answer: &str) -> Option<Grade> {
    let picked = answer.strip_prefix("selected: ")?.split(';').next()?.trim();
    let picks: Vec<String> = picked
        .split(',')
        .map(str::trim)
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .collect();
    let right: Vec<String> = q.correct_answer.values().to_vec();
    let sorted = |mut v: Vec<String>| {
        v.sort_unstable();
        v
    };
    let correct = sorted(picks.clone()) == sorted(right.clone());
    Some(Grade {
        correct,
        picked: picks,
        right,
        confidence: confidence_in(answer),
    })
}

/// The graded answer to a quiz: correct or not, the right option, the call's rationale and the
/// confidence calibration. `None` when the answer carries no selected option.
pub fn verdict(q: &QuizInput, call_id: &str, answer: &str) -> Option<String> {
    let g = grade(q, answer)?;
    let shown = shuffle_options(&q.options, call_id);
    let mut out = if g.correct {
        "**Correct.**".to_owned()
    } else {
        "**Incorrect.**".to_owned()
    };
    if !g.correct {
        let right: Vec<String> = g.right.iter().map(|v| option_label(&shown, v)).collect();
        out.push_str(&format!(" The right answer: {}.", right.join(", ")));
    }
    if !q.explanation.trim().is_empty() {
        out.push_str(&format!(" {}", q.explanation.trim()));
    }
    if let Some(c) = g.confidence {
        let note = match (g.correct, c) {
            (true, 4..) => "well calibrated",
            (true, ..=2) => "underconfident",
            (false, 4..) => "overconfident",
            (false, ..=2) => "well calibrated",
            _ => "moderate",
        };
        out.push_str(&format!(" Confidence {c}/5: {note}."));
    }
    Some(out)
}
