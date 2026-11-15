//! F14: the mastery bar. Per key fact: `open`, `taught`, `mc-passed`, `confirmed` or `parked`.
//! Confirmed needs a right multiple choice and a right free recall, on separate turns with
//! teaching between; a miss always returns the fact to `open`. `Predicted` events never touch
//! this fold.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::course::{Course, CourseEvent, ItemKind, Lesson};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FactStatus {
    #[default]
    Open,
    Taught,
    McPassed,
    Confirmed,
    Parked,
}

/// Key is `"{lesson}#{fact index}"`, where the fact index is into the lesson's `key_facts`.
pub type FactKey = String;

pub fn fact_key(lesson: &str, fact_index: usize) -> FactKey {
    format!("{lesson}#{fact_index}")
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MasteryState {
    pub facts: BTreeMap<FactKey, FactStatus>,
    /// Facts at `McPassed` that have been taught again since the multiple choice pass, so a
    /// right `Recall` now confirms them. `next_step` serves the recap before the recall.
    pub recall_ready: BTreeSet<FactKey>,
}

impl MasteryState {
    pub fn status(&self, lesson: &str, fact_index: usize) -> FactStatus {
        self.facts
            .get(&fact_key(lesson, fact_index))
            .copied()
            .unwrap_or_default()
    }

    /// True when a `Recall` answered now would confirm this fact.
    pub fn recall_ready(&self, lesson: &str, fact_index: usize) -> bool {
        self.recall_ready.contains(&fact_key(lesson, fact_index))
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct StatusCounts {
    pub open: u32,
    pub taught: u32,
    pub mc_passed: u32,
    pub confirmed: u32,
    pub parked: u32,
}

/// Counts for one lesson's own facts (not the whole course).
pub fn counts_for_lesson(state: &MasteryState, lesson: &Lesson) -> StatusCounts {
    let mut c = StatusCounts::default();
    for i in 0..lesson.key_facts.len() {
        match state.status(&lesson.id, i) {
            FactStatus::Open => c.open += 1,
            FactStatus::Taught => c.taught += 1,
            FactStatus::McPassed => c.mc_passed += 1,
            FactStatus::Confirmed => c.confirmed += 1,
            FactStatus::Parked => c.parked += 1,
        }
    }
    c
}

/// Renders e.g. "11 of 14 confirmed, 3 open, 1 parked". The denominator excludes parked facts,
/// which are exempt from F14's advance check.
pub fn count_line(c: &StatusCounts) -> String {
    let non_parked = c.open + c.taught + c.mc_passed + c.confirmed;
    let mut parts = vec![format!("{} of {non_parked} confirmed", c.confirmed)];
    for (n, label) in [
        (c.taught, "taught"),
        (c.mc_passed, "mc-passed"),
        (c.open, "open"),
        (c.parked, "parked"),
    ] {
        if n > 0 {
            parts.push(format!("{n} {label}"));
        }
    }
    parts.join(", ")
}

#[derive(Default)]
struct Track {
    status: FactStatus,
    mc_pass_seq: Option<usize>,
    taught_since_mc: bool,
}

fn apply_outcome(t: &mut Track, kind: ItemKind, correct: bool, seq: usize) {
    if !correct {
        *t = Track::default();
        return;
    }
    match kind {
        ItemKind::Mcq => {
            if matches!(t.status, FactStatus::Open | FactStatus::Taught) {
                t.status = FactStatus::McPassed;
                t.mc_pass_seq = Some(seq);
                t.taught_since_mc = false;
            }
        }
        ItemKind::Recall => {
            if t.status == FactStatus::McPassed && t.taught_since_mc {
                t.status = FactStatus::Confirmed;
            }
        }
    }
}

struct ItemMeta<'a> {
    lesson: &'a str,
    kind: ItemKind,
    fact: Option<usize>,
}

/// Rebuild mastery from the event log. Pure and prefix-safe.
pub fn fold_mastery(course: &Course, events: &[CourseEvent]) -> MasteryState {
    let mut meta: BTreeMap<&str, ItemMeta> = BTreeMap::new();
    for l in &course.lessons {
        for it in &l.items {
            meta.insert(
                it.id.as_str(),
                ItemMeta {
                    lesson: l.id.as_str(),
                    kind: it.kind,
                    fact: it.tests,
                },
            );
        }
    }
    let mut tracks: BTreeMap<FactKey, Track> = BTreeMap::new();

    for (seq, ev) in events.iter().enumerate() {
        match ev {
            CourseEvent::Shown { lesson, .. } => {
                let Some(l) = course.lessons.iter().find(|l| &l.id == lesson) else {
                    continue;
                };
                for i in 0..l.key_facts.len() {
                    let t = tracks.entry(fact_key(lesson, i)).or_default();
                    if t.status == FactStatus::Open {
                        t.status = FactStatus::Taught;
                    }
                    if t.mc_pass_seq.is_some_and(|mc| seq > mc) {
                        t.taught_since_mc = true;
                    }
                }
            }
            CourseEvent::Answered {
                item, correct, gap, ..
            } => {
                if *gap {
                    continue;
                }
                let Some(m) = meta.get(item.as_str()) else {
                    continue;
                };
                let Some(fact) = m.fact else { continue };
                let t = tracks.entry(fact_key(m.lesson, fact)).or_default();
                apply_outcome(t, m.kind, *correct, seq);
            }
            CourseEvent::RecallGraded { item, correct, .. } => {
                let Some(m) = meta.get(item.as_str()) else {
                    continue;
                };
                let Some(fact) = m.fact else { continue };
                let t = tracks.entry(fact_key(m.lesson, fact)).or_default();
                apply_outcome(t, ItemKind::Recall, *correct, seq);
            }
            CourseEvent::ItemParked { item, .. } => {
                if let Some(m) = meta.get(item.as_str()) {
                    if let Some(fact) = m.fact {
                        tracks.entry(fact_key(m.lesson, fact)).or_default().status =
                            FactStatus::Parked;
                    }
                }
            }
            // Predictions (and every other event) never affect mastery.
            _ => {}
        }
    }
    let recall_ready = tracks
        .iter()
        .filter(|(_, t)| t.status == FactStatus::McPassed && t.taught_since_mc)
        .map(|(k, _)| k.clone())
        .collect();
    MasteryState {
        facts: tracks.into_iter().map(|(k, t)| (k, t.status)).collect(),
        recall_ready,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::{Course, Item, ItemKind, Lesson};
    use proptest::prelude::*;

    fn course_with_one_fact() -> Course {
        Course {
            id: "c".into(),
            lessons: vec![Lesson {
                id: "l00".into(),
                key_facts: vec!["fact 0".into()],
                items: vec![
                    Item {
                        id: "l00-mcq".into(),
                        kind: ItemKind::Mcq,
                        q: "mc?".into(),
                        answer: "a".into(),
                        options: vec!["a".into(), "b".into(), "c".into(), "d".into()],
                        tests: Some(0),
                        ..Item::default()
                    },
                    Item {
                        id: "l00-recall".into(),
                        kind: ItemKind::Recall,
                        q: "recall?".into(),
                        answer: "a".into(),
                        tests: Some(0),
                        ..Item::default()
                    },
                ],
                ..Lesson::default()
            }],
            ..Course::default()
        }
    }

    fn mc(correct: bool) -> CourseEvent {
        CourseEvent::Answered {
            ts: 0,
            item: "l00-mcq".into(),
            correct,
            chosen: String::new(),
            confidence: None,
            ms: None,
            gap: false,
            reasoning: String::new(),
        }
    }

    fn recall(correct: bool) -> CourseEvent {
        CourseEvent::RecallGraded {
            ts: 0,
            item: "l00-recall".into(),
            correct,
            source: "model".into(),
        }
    }

    fn shown() -> CourseEvent {
        CourseEvent::Shown {
            ts: 0,
            lesson: "l00".into(),
        }
    }

    fn predicted() -> CourseEvent {
        CourseEvent::Predicted {
            ts: 0,
            lesson: "l00".into(),
            guess: "whatever".into(),
        }
    }

    #[test]
    fn mc_alone_never_confirms() {
        let c = course_with_one_fact();
        let st = fold_mastery(&c, &[mc(true)]);
        assert_eq!(st.status("l00", 0), FactStatus::McPassed);
    }

    #[test]
    fn mc_and_recall_on_the_same_turn_never_confirms() {
        let c = course_with_one_fact();
        // No `Shown` (teaching) between the two correct answers.
        let st = fold_mastery(&c, &[mc(true), recall(true)]);
        assert_eq!(st.status("l00", 0), FactStatus::McPassed);
    }

    #[test]
    fn mc_then_teaching_then_recall_confirms() {
        let c = course_with_one_fact();
        let st = fold_mastery(&c, &[mc(true), shown(), recall(true)]);
        assert_eq!(st.status("l00", 0), FactStatus::Confirmed);
    }

    #[test]
    fn recall_before_any_mc_pass_does_not_confirm() {
        let c = course_with_one_fact();
        let st = fold_mastery(&c, &[shown(), recall(true)]);
        assert_eq!(st.status("l00", 0), FactStatus::Taught);
    }

    #[test]
    fn a_miss_always_returns_to_open() {
        let c = course_with_one_fact();
        for setup in [
            vec![mc(true)],
            vec![mc(true), shown(), recall(true)],
            vec![shown()],
        ] {
            let mut events = setup;
            events.push(mc(false));
            let st = fold_mastery(&c, &events);
            assert_eq!(st.status("l00", 0), FactStatus::Open, "{events:?}");
        }
        // A missed recall resets it too.
        let st = fold_mastery(&c, &[mc(true), shown(), recall(false)]);
        assert_eq!(st.status("l00", 0), FactStatus::Open);
    }

    #[test]
    fn a_prediction_never_affects_mastery() {
        let c = course_with_one_fact();
        let without = fold_mastery(&c, &[mc(true), shown(), recall(true)]);
        let with = fold_mastery(
            &c,
            &[
                predicted(),
                mc(true),
                predicted(),
                shown(),
                predicted(),
                recall(true),
            ],
        );
        assert_eq!(without, with);
    }

    #[test]
    fn item_parked_sets_parked_and_is_exempt_from_counts_denominator() {
        let c = course_with_one_fact();
        let st = fold_mastery(
            &c,
            &[CourseEvent::ItemParked {
                ts: 0,
                item: "l00-mcq".into(),
            }],
        );
        assert_eq!(st.status("l00", 0), FactStatus::Parked);
        let counts = counts_for_lesson(&st, &c.lessons[0]);
        assert_eq!(counts.parked, 1);
        assert_eq!(count_line(&counts), "0 of 0 confirmed, 1 parked");
    }

    #[test]
    fn count_line_matches_the_spec_example() {
        let c = StatusCounts {
            open: 3,
            taught: 0,
            mc_passed: 0,
            confirmed: 11,
            parked: 1,
        };
        assert_eq!(count_line(&c), "11 of 14 confirmed, 3 open, 1 parked");
    }

    proptest! {
        #[test]
        fn fold_deterministic_and_prefix_safe(
            which in prop::collection::vec(0u8..5, 0..60),
            correct in prop::collection::vec(any::<bool>(), 0..60),
        ) {
            let c = course_with_one_fact();
            let n = which.len().min(correct.len());
            let events: Vec<CourseEvent> = (0..n)
                .map(|i| match which[i] {
                    0 => mc(correct[i]),
                    1 => recall(correct[i]),
                    2 => shown(),
                    3 => predicted(),
                    _ => CourseEvent::ItemParked { ts: 0, item: "l00-mcq".into() },
                })
                .collect();
            prop_assert_eq!(fold_mastery(&c, &events), fold_mastery(&c, &events));
            for k in 0..=events.len() {
                let _ = fold_mastery(&c, &events[..k]);
            }
        }
    }
}
