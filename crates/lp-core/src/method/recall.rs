//! F15: recall grading. Exact match is handled by `course::match_answer`; this is the rubric
//! step after it — a stored list of terms the answer must name, checked by code with no model
//! call. A match confirms the fact directly. Rubric false negatives (right answer, different
//! words) are accepted; the rubric must never produce a false positive.

use crate::course::{fold_accents, normalise, CourseEvent, Item};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RubricResult {
    /// Every `must_name` term was found in the answer.
    Match,
    /// No rubric is stored, or a term is missing: falls to the model, then to a pending
    /// self-grade if no model is available.
    NoMatch,
}

fn folded(s: &str) -> String {
    fold_accents(&normalise(s))
}

/// Checks that every term in `item.must_name` appears in `text` as a whole word or phrase,
/// ignoring case, accents and punctuation. Never matches when `must_name` is empty, so a lesson
/// with no stored rubric always falls through instead of confirming on nothing.
pub fn rubric_check(item: &Item, text: &str) -> RubricResult {
    if item.must_name.is_empty() {
        return RubricResult::NoMatch;
    }
    let given = format!(" {} ", folded(text));
    let hit = item.must_name.iter().all(|term| {
        let t = folded(term);
        !t.is_empty() && given.contains(&format!(" {t} "))
    });
    if hit {
        RubricResult::Match
    } else {
        RubricResult::NoMatch
    }
}

/// Items with a `RecallPending` not yet followed by a `RecallGraded` or `Answered` on the same
/// item: `(item, lesson, text)`, in first-pending order, each with its most recent pending text.
/// A later grader run batches these into one cheap call (F15's fallback when the rubric does
/// not match); this is how it finds what still needs grading.
pub fn unresolved_pending(events: &[CourseEvent]) -> Vec<(String, String, String)> {
    let mut open: Vec<(String, String, String)> = Vec::new();
    for ev in events {
        match ev {
            CourseEvent::RecallPending {
                item, lesson, text, ..
            } => match open.iter_mut().find(|(i, ..)| i == item) {
                Some(e) => {
                    e.1 = lesson.clone();
                    e.2 = text.clone();
                }
                None => open.push((item.clone(), lesson.clone(), text.clone())),
            },
            CourseEvent::RecallGraded { item, .. } | CourseEvent::Answered { item, .. } => {
                open.retain(|(i, ..)| i != item);
            }
            _ => {}
        }
    }
    open
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::ItemKind;

    fn item(must_name: &[&str]) -> Item {
        Item {
            kind: ItemKind::Recall,
            q: "q".into(),
            answer: "a".into(),
            must_name: must_name.iter().map(|s| s.to_string()).collect(),
            ..Item::default()
        }
    }

    #[test]
    fn empty_rubric_never_matches() {
        assert_eq!(
            rubric_check(&item(&[]), "anything at all"),
            RubricResult::NoMatch
        );
    }

    #[test]
    fn all_terms_present_matches() {
        let it = item(&["mitochondria", "energy"]);
        assert_eq!(
            rubric_check(&it, "The mitochondria makes energy for the cell."),
            RubricResult::Match
        );
    }

    #[test]
    fn missing_term_falls_through() {
        let it = item(&["mitochondria", "energy"]);
        assert_eq!(
            rubric_check(&it, "The mitochondria is round."),
            RubricResult::NoMatch
        );
    }

    #[test]
    fn accent_insensitive() {
        let it = item(&["mañana"]);
        assert_eq!(rubric_check(&it, "see you manana"), RubricResult::Match);
    }

    #[test]
    fn whole_word_only_no_substring_false_positive() {
        // "art" must not match inside "party": a false positive the rubric must never produce.
        let it = item(&["art"]);
        assert_eq!(
            rubric_check(&it, "we went to a party"),
            RubricResult::NoMatch
        );
        assert_eq!(rubric_check(&it, "it is modern art"), RubricResult::Match);
    }

    #[test]
    fn multi_word_phrase_matches() {
        let it = item(&["my name is"]);
        assert_eq!(
            rubric_check(&it, "well, my name is Alex"),
            RubricResult::Match
        );
    }

    fn pending(item: &str, lesson: &str, text: &str) -> CourseEvent {
        CourseEvent::RecallPending {
            ts: 0,
            item: item.into(),
            lesson: lesson.into(),
            text: text.into(),
        }
    }

    fn graded(item: &str) -> CourseEvent {
        CourseEvent::RecallGraded {
            ts: 0,
            item: item.into(),
            correct: true,
            source: "model".into(),
        }
    }

    #[test]
    fn unresolved_pending_tracks_open_items_in_first_seen_order() {
        let events = [pending("l00-i0", "l00", "a"), pending("l00-i1", "l00", "b")];
        assert_eq!(
            unresolved_pending(&events),
            vec![
                ("l00-i0".to_string(), "l00".to_string(), "a".to_string()),
                ("l00-i1".to_string(), "l00".to_string(), "b".to_string()),
            ]
        );
    }

    #[test]
    fn a_graded_event_resolves_its_pending_item() {
        let events = [pending("l00-i0", "l00", "a"), graded("l00-i0")];
        assert!(unresolved_pending(&events).is_empty());
    }

    #[test]
    fn a_later_pending_on_the_same_item_keeps_the_newest_text() {
        let events = [
            pending("l00-i0", "l00", "first try"),
            pending("l00-i0", "l00", "second try"),
        ];
        assert_eq!(
            unresolved_pending(&events),
            vec![(
                "l00-i0".to_string(),
                "l00".to_string(),
                "second try".to_string()
            )]
        );
    }
}
