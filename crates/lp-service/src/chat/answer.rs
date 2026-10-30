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

fn letter(i: usize) -> char {
    char::from(b'A' + i as u8)
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

/// The graded answer to a quiz: correct or not, the right option, the call's rationale and the
/// confidence calibration. `None` when the answer carries no selected option.
pub fn verdict(q: &QuizInput, call_id: &str, answer: &str) -> Option<String> {
    let picked = answer.strip_prefix("selected: ")?.split(';').next()?.trim();
    let right = q.correct_answer.values();
    let shown = shuffle_options(&q.options, call_id);
    let label = |v: &str| {
        shown.iter().position(|o| o.value == v).map_or_else(
            || v.to_owned(),
            |i| format!("{}. {}", letter(i), shown[i].label),
        )
    };
    let mut picks: Vec<&str> = picked.split(',').map(str::trim).collect();
    picks.sort_unstable();
    let mut want: Vec<&str> = right.iter().map(String::as_str).collect();
    want.sort_unstable();
    let correct = picks == want;
    let mut out = if correct {
        "**Correct.**".to_owned()
    } else {
        "**Incorrect.**".to_owned()
    };
    if !correct {
        let right: Vec<String> = right.iter().map(|v| label(v)).collect();
        out.push_str(&format!(" The right answer: {}.", right.join(", ")));
    }
    if !q.explanation.trim().is_empty() {
        out.push_str(&format!(" {}", q.explanation.trim()));
    }
    let conf = answer
        .rsplit("confidence: ")
        .next()
        .and_then(|c| c.split('/').next())
        .and_then(|c| c.trim().parse::<u8>().ok());
    if let Some(c) = conf {
        let note = match (correct, c) {
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
