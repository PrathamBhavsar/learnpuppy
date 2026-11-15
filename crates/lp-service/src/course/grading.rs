//! F15/F18: turning an answer into correct-or-not. Multiple choice is compared by code; a
//! recall answer tries the stored rubric first (code, no model), then one cheap batched call —
//! per lesson normally, or per bout on a non-Claude fallback, where calls are scarcer — and
//! finally stays `RecallPending` when no provider can be reached at all, for a later grader run
//! to resolve. A Feynman pass (F18) is graded the same way, but submitting it always passes the
//! gate: the model only decides whether it was recorded correct, never whether study continues.

use lp_agent::course::{feynman, grade_batch, Check};
use lp_cli_driver::engine::LlmEngine;
use lp_core::course::{Course, CourseEvent, Item, Lesson};
use lp_core::method::recall::{rubric_check, unresolved_pending, RubricResult};

/// A multiple-choice answer is graded by exact comparison with the stored option text: the
/// learner picks an option verbatim, never retypes the answer.
pub fn grade_mcq(item: &Item, chosen: &str) -> bool {
    chosen == item.answer
}

/// The event to append for a freshly submitted recall answer: an instant rubric match grades it
/// at once (zero model calls); anything else becomes pending until a batched grader run
/// resolves it.
pub fn submit_recall(lesson: &str, item: &Item, text: &str, now: i64) -> CourseEvent {
    match rubric_check(item, text) {
        RubricResult::Match => CourseEvent::RecallGraded {
            ts: now,
            item: item.id.clone(),
            correct: true,
            source: "rubric".into(),
        },
        RubricResult::NoMatch => CourseEvent::RecallPending {
            ts: now,
            item: item.id.clone(),
            lesson: lesson.to_string(),
            text: text.to_string(),
        },
    }
}

/// How many pending recalls one grader run batches into a single cheap call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BatchScope<'a> {
    /// One lesson's pending recalls.
    Lesson(&'a str),
    /// Every pending recall in the course, regardless of lesson — the non-Claude fallback,
    /// where calls are scarcer than lessons.
    Bout,
}

/// One grader run: batches every unresolved `RecallPending` in `scope` into a single cheap
/// call, then returns the `RecallGraded` events to append. With no engine (every provider
/// limited) or a failed call, nothing is returned: every pending item stays pending rather than
/// guessing, and the same run can be retried later with no loss.
pub async fn run_grader<E: LlmEngine>(
    engine: Option<&E>,
    course: &Course,
    events: &[CourseEvent],
    scope: BatchScope<'_>,
    now: i64,
) -> Vec<CourseEvent> {
    let Some(engine) = engine else {
        return Vec::new();
    };
    let pending: Vec<(String, String, String)> = unresolved_pending(events)
        .into_iter()
        .filter(|(_, lesson, _)| match scope {
            BatchScope::Lesson(l) => lesson == l,
            BatchScope::Bout => true,
        })
        .collect();
    if pending.is_empty() {
        return Vec::new();
    }
    let pairs: Vec<(Item, String)> = pending
        .iter()
        .filter_map(|(item_id, _, text)| {
            lp_core::course::find_item(course, item_id).map(|(_, it)| (it.clone(), text.clone()))
        })
        .collect();
    let Ok(graded) = grade_batch(engine, &pairs).await else {
        return Vec::new();
    };
    graded
        .into_iter()
        .map(|g| CourseEvent::RecallGraded {
            ts: now,
            item: g.item_id,
            correct: g.correct,
            source: "model".into(),
        })
        .collect()
}

/// F18: never blocks the gate — submitting a Feynman pass always moves the lesson on; whether a
/// model could judge it is a separate, best-effort question. With no engine available (or a
/// failed call) it counts as correct provisionally, since study must not wait on a provider.
pub async fn grade_feynman<E: LlmEngine>(
    engine: Option<&E>,
    lesson: &Lesson,
    explanation: &str,
) -> Check {
    let default = Check {
        correct: true,
        note: String::new(),
    };
    let Some(engine) = engine else {
        return default;
    };
    feynman(engine, lesson, explanation)
        .await
        .unwrap_or(default)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_cli_driver::engine::FauxEngine;
    use lp_core::course::ItemKind;

    fn recall_item(id: &str, must_name: &[&str]) -> Item {
        Item {
            id: id.into(),
            kind: ItemKind::Recall,
            q: "q?".into(),
            answer: "the answer".into(),
            must_name: must_name.iter().map(|s| s.to_string()).collect(),
            ..Item::default()
        }
    }

    fn course_with(lessons: Vec<Lesson>) -> Course {
        Course {
            id: "c".into(),
            lessons,
            ..Course::default()
        }
    }

    fn lesson(id: &str, items: Vec<Item>) -> Lesson {
        Lesson {
            id: id.into(),
            items,
            ..Lesson::default()
        }
    }

    #[test]
    fn mcq_grading_is_exact_comparison() {
        let it = Item {
            answer: "paris".into(),
            ..Item::default()
        };
        assert!(grade_mcq(&it, "paris"));
        assert!(!grade_mcq(&it, "Paris"));
    }

    #[test]
    fn a_rubric_match_grades_instantly_with_no_model() {
        let it = recall_item("l00-i0", &["mitochondria"]);
        let ev = submit_recall("l00", &it, "the mitochondria does it", 10);
        assert_eq!(
            ev,
            CourseEvent::RecallGraded {
                ts: 10,
                item: "l00-i0".into(),
                correct: true,
                source: "rubric".into(),
            }
        );
    }

    #[test]
    fn a_rubric_miss_becomes_pending() {
        let it = recall_item("l00-i0", &["mitochondria"]);
        let ev = submit_recall("l00", &it, "something else entirely", 10);
        assert_eq!(
            ev,
            CourseEvent::RecallPending {
                ts: 10,
                item: "l00-i0".into(),
                lesson: "l00".into(),
                text: "something else entirely".into(),
            }
        );
    }

    #[tokio::test]
    async fn with_every_provider_limited_recall_stays_pending() {
        let it = recall_item("l00-i0", &["mitochondria"]);
        let course = course_with(vec![lesson("l00", vec![it])]);
        let events = vec![CourseEvent::RecallPending {
            ts: 0,
            item: "l00-i0".into(),
            lesson: "l00".into(),
            text: "something else".into(),
        }];
        let out =
            run_grader::<FauxEngine>(None, &course, &events, BatchScope::Lesson("l00"), 20).await;
        assert!(out.is_empty());
        // Nothing resolved it: it is still open for the next grader run.
        assert_eq!(unresolved_pending(&events).len(), 1);
    }

    #[tokio::test]
    async fn a_later_grader_run_with_a_provider_confirms_a_pending_recall() {
        let it = recall_item("l00-i0", &["mitochondria"]);
        let course = course_with(vec![lesson("l00", vec![it])]);
        let events = vec![CourseEvent::RecallPending {
            ts: 0,
            item: "l00-i0".into(),
            lesson: "l00".into(),
            text: "something else".into(),
        }];
        let engine = FauxEngine::new();
        engine.faux_text(r#"{"grades":[{"id":"l00-i0","correct":true,"note":"close enough"}]}"#);
        let out = run_grader(
            Some(&engine),
            &course,
            &events,
            BatchScope::Lesson("l00"),
            20,
        )
        .await;
        assert_eq!(
            out,
            vec![CourseEvent::RecallGraded {
                ts: 20,
                item: "l00-i0".into(),
                correct: true,
                source: "model".into(),
            }]
        );
    }

    #[tokio::test]
    async fn non_claude_fallback_batches_every_lesson_s_pending_recalls_into_one_call() {
        let a = recall_item("l00-i0", &["x"]);
        let b = recall_item("l01-i0", &["y"]);
        let course = course_with(vec![lesson("l00", vec![a]), lesson("l01", vec![b])]);
        let events = vec![
            CourseEvent::RecallPending {
                ts: 0,
                item: "l00-i0".into(),
                lesson: "l00".into(),
                text: "t1".into(),
            },
            CourseEvent::RecallPending {
                ts: 1,
                item: "l01-i0".into(),
                lesson: "l01".into(),
                text: "t2".into(),
            },
        ];
        let engine = FauxEngine::new();
        // Exactly one queued response: a `Bout` run must cover both lessons in that single call.
        engine.faux_text(
            r#"{"grades":[{"id":"l00-i0","correct":true,"note":""},
                          {"id":"l01-i0","correct":false,"note":""}]}"#,
        );
        let out = run_grader(Some(&engine), &course, &events, BatchScope::Bout, 5).await;
        assert_eq!(out.len(), 2);
        assert!(out.iter().any(
            |e| matches!(e, CourseEvent::RecallGraded { item, correct, .. } if item == "l00-i0" && *correct)
        ));
        assert!(out.iter().any(
            |e| matches!(e, CourseEvent::RecallGraded { item, correct, .. } if item == "l01-i0" && !*correct)
        ));
    }

    #[tokio::test]
    async fn lesson_scope_only_batches_that_lesson_s_pending_recalls() {
        let a = recall_item("l00-i0", &["x"]);
        let b = recall_item("l01-i0", &["y"]);
        let course = course_with(vec![lesson("l00", vec![a]), lesson("l01", vec![b])]);
        let events = vec![
            CourseEvent::RecallPending {
                ts: 0,
                item: "l00-i0".into(),
                lesson: "l00".into(),
                text: "t1".into(),
            },
            CourseEvent::RecallPending {
                ts: 1,
                item: "l01-i0".into(),
                lesson: "l01".into(),
                text: "t2".into(),
            },
        ];
        let engine = FauxEngine::new();
        engine.faux_text(r#"{"grades":[{"id":"l00-i0","correct":true,"note":""}]}"#);
        let out = run_grader(
            Some(&engine),
            &course,
            &events,
            BatchScope::Lesson("l00"),
            5,
        )
        .await;
        assert_eq!(out.len(), 1);
        assert!(matches!(&out[0], CourseEvent::RecallGraded { item, .. } if item == "l00-i0"));
    }

    #[tokio::test]
    async fn feynman_with_no_engine_passes_provisionally() {
        let lesson = Lesson::default();
        let check = grade_feynman::<FauxEngine>(None, &lesson, "it just does").await;
        assert!(check.correct);
    }

    #[tokio::test]
    async fn feynman_uses_the_model_s_verdict_when_available() {
        let lesson = Lesson {
            key_facts: vec!["mitochondria makes energy".into()],
            ..Lesson::default()
        };
        let engine = FauxEngine::new();
        engine.faux_text(r#"{"correct":false,"note":"missed the mechanism"}"#);
        let check = grade_feynman(Some(&engine), &lesson, "it just does").await;
        assert!(!check.correct);
        assert_eq!(check.note, "missed the mechanism");
    }
}
