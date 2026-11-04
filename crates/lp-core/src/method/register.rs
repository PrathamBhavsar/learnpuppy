//! F19: the misconception register. Each wrong multiple-choice option stores the belief it
//! implies, so code writes the row the moment the learner picks it, with no model call. "I
//! don't know" is a gap (F12), not a miss, and never writes a row.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::course::{find_item, Course, CourseEvent, ItemKind};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RegisterStatus {
    #[default]
    Open,
    RetestedOk,
    Recurring,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegisterRow {
    pub item: String,
    pub belief: String,
    pub status: RegisterStatus,
    pub misses: u32,
}

/// Rebuild the misconception register from the event log. Pure and prefix-safe; keyed by item
/// id, in item-id order.
pub fn fold_register(course: &Course, events: &[CourseEvent]) -> Vec<RegisterRow> {
    let mut by_item: BTreeMap<String, RegisterRow> = BTreeMap::new();
    for ev in events {
        let CourseEvent::Answered {
            item,
            correct,
            chosen,
            gap,
            ..
        } = ev
        else {
            continue;
        };
        if *gap {
            continue;
        }
        if *correct {
            if let Some(row) = by_item.get_mut(item) {
                row.status = RegisterStatus::RetestedOk;
            }
            continue;
        }
        let Some((_, it)) = find_item(course, item) else {
            continue;
        };
        if it.kind != ItemKind::Mcq {
            continue;
        }
        let Some(idx) = it.options.iter().position(|o| o == chosen) else {
            continue;
        };
        let Some(belief) = it.misconceptions.get(idx).filter(|b| !b.trim().is_empty()) else {
            continue;
        };
        match by_item.get_mut(item) {
            Some(row) => {
                if row.status == RegisterStatus::RetestedOk {
                    row.status = RegisterStatus::Recurring;
                }
                row.misses += 1;
                row.belief = belief.clone();
            }
            None => {
                by_item.insert(
                    item.clone(),
                    RegisterRow {
                        item: item.clone(),
                        belief: belief.clone(),
                        status: RegisterStatus::Open,
                        misses: 1,
                    },
                );
            }
        }
    }
    by_item.into_values().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::{Course, Item, ItemKind, Lesson};
    use proptest::prelude::*;

    fn course_with_mcq() -> Course {
        Course {
            id: "c".into(),
            lessons: vec![Lesson {
                id: "l00".into(),
                items: vec![Item {
                    id: "l00-i0".into(),
                    kind: ItemKind::Mcq,
                    q: "q".into(),
                    options: vec!["a".into(), "b".into(), "c".into(), "d".into()],
                    answer: "a".into(),
                    misconceptions: vec![
                        String::new(),
                        "thinks b".into(),
                        String::new(),
                        String::new(),
                    ],
                    ..Item::default()
                }],
                ..Lesson::default()
            }],
            ..Course::default()
        }
    }

    fn ans(item: &str, correct: bool, chosen: &str, gap: bool) -> CourseEvent {
        CourseEvent::Answered {
            ts: 0,
            item: item.into(),
            correct,
            chosen: chosen.into(),
            confidence: None,
            ms: None,
            gap,
            reasoning: String::new(),
        }
    }

    #[test]
    fn miss_with_misconception_writes_a_row() {
        let c = course_with_mcq();
        let rows = fold_register(&c, &[ans("l00-i0", false, "b", false)]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].belief, "thinks b");
        assert_eq!(rows[0].status, RegisterStatus::Open);
        assert_eq!(rows[0].misses, 1);
    }

    #[test]
    fn i_dont_know_adds_no_row() {
        let c = course_with_mcq();
        let rows = fold_register(&c, &[ans("l00-i0", false, "", true)]);
        assert!(rows.is_empty());
    }

    #[test]
    fn miss_without_a_stored_misconception_adds_no_row() {
        let c = course_with_mcq();
        // "c" has an empty misconceptions entry.
        let rows = fold_register(&c, &[ans("l00-i0", false, "c", false)]);
        assert!(rows.is_empty());
    }

    #[test]
    fn correct_after_miss_is_retested_ok_then_recurring_on_relapse() {
        let c = course_with_mcq();
        let events = [
            ans("l00-i0", false, "b", false),
            ans("l00-i0", true, "a", false),
        ];
        let rows = fold_register(&c, &events);
        assert_eq!(rows[0].status, RegisterStatus::RetestedOk);

        let events = [
            ans("l00-i0", false, "b", false),
            ans("l00-i0", true, "a", false),
            ans("l00-i0", false, "b", false),
        ];
        let rows = fold_register(&c, &events);
        assert_eq!(rows[0].status, RegisterStatus::Recurring);
        assert_eq!(rows[0].misses, 2);
    }

    proptest! {
        #[test]
        fn fold_deterministic_and_prefix_safe(
            correct in prop::collection::vec(any::<bool>(), 0..40),
            chosen in prop::collection::vec(prop::sample::select(vec!["a", "b", "c", "d", "ghost"]), 0..40),
            gap in prop::collection::vec(any::<bool>(), 0..40),
        ) {
            let c = course_with_mcq();
            let n = correct.len().min(chosen.len()).min(gap.len());
            let events: Vec<CourseEvent> = (0..n)
                .map(|i| ans("l00-i0", correct[i], chosen[i], gap[i]))
                .collect();
            prop_assert_eq!(fold_register(&c, &events), fold_register(&c, &events));
            for k in 0..=events.len() {
                let _ = fold_register(&c, &events[..k]);
            }
        }
    }
}
