//! F07/F20: the single entry point for every learner action on a `method_version >= 1` course.
//! `apply` calls `method::check_action` before it ever touches the store, so no event this
//! service writes on behalf of a learner can skip the gate; `confirm_plan` and `insert_prereq`
//! are the two actions that also change the course itself, not just append an event.

use lp_cli_driver::engine::LlmEngine;
use lp_core::course::{find_item, rating_for, Course, CourseEvent, ItemKind, Lesson};
use lp_core::method::dag::{self, PrereqError};
use lp_core::method::mastery::{fact_key, fold_mastery};
use lp_core::method::{
    check_action, fold_runner, next_step, Action, AskKind, GateError, Settings, Step,
};
use lp_core::ports::{StoreError, TrackerStore};
use lp_store::FsCourseStore;

use super::{card_id, grading};
use crate::review;

#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    #[error("{0}")]
    Gate(#[from] GateError),
    #[error("{0}")]
    Prereq(#[from] PrereqError),
    #[error("storage failure: {0}")]
    Store(#[from] StoreError),
    #[error("no AI could write the prerequisite lesson: {0}")]
    Agent(#[from] lp_agent::course::CourseAgentError),
    #[error("lesson not found: {0}")]
    NotFound(String),
    #[error("{0:?} changes the course itself; use confirm_plan or insert_prereq instead")]
    NeedsDedicatedCall(Action),
}

/// Validates `action` against `check_action`, then appends `event` — the caller's job is to
/// make sure `event` is the event that `action` describes; this is the only function in the
/// method course service that writes a learner-driven event, so every action passes the gate.
/// `ConfirmPlan` and `InsertPrereq` change the course itself, not just the event log: use
/// `confirm_plan` and `insert_prereq` for those instead.
pub fn apply(
    store: &FsCourseStore,
    id: &str,
    settings: &Settings,
    due: &[String],
    start: bool,
    action: &Action,
    event: &CourseEvent,
) -> Result<(), PlanError> {
    if matches!(action, Action::ConfirmPlan | Action::InsertPrereq { .. }) {
        return Err(PlanError::NeedsDedicatedCall(action.clone()));
    }
    let course = store.load_course(id)?;
    let events = store.events(id)?;
    let mastery = fold_mastery(&course, &events);
    check_action(&course, &events, &mastery, settings, due, start, action)?;
    store.append_event(id, event)?;
    Ok(())
}

/// F07: confirms the plan map. Rejected if the plan has a cycle; a no-op once already confirmed
/// (confirming again still passes the gate, since a confirmed plan has no cycle either).
pub fn confirm_plan(store: &FsCourseStore, id: &str, now: i64) -> Result<(), PlanError> {
    let mut course = store.load_course(id)?;
    let events = store.events(id)?;
    let mastery = fold_mastery(&course, &events);
    let settings = Settings::default();
    check_action(
        &course,
        &events,
        &mastery,
        &settings,
        &[],
        false,
        &Action::ConfirmPlan,
    )?;
    course.plan_confirmed_at = Some(now);
    store.save_course(&course)?;
    Ok(())
}

/// The lesson a prerequisite would precede, for building the agent's prompt. Split out of
/// [`insert_prereq`] so a caller on an async runtime can run this (blocking file IO) in
/// `spawn_blocking`, await the agent call unblocked, then run [`commit_prereq`] the same way —
/// without ever holding a blocking call across an `.await`.
pub fn prereq_target(store: &FsCourseStore, id: &str, before: &str) -> Result<Lesson, PlanError> {
    let course = store.load_course(id)?;
    course
        .lessons
        .iter()
        .find(|l| l.id == before)
        .cloned()
        .ok_or_else(|| PlanError::NotFound(before.to_string()))
}

/// Checks the gate and splices a freshly written prerequisite lesson into the frozen plan ahead
/// of `before`, assigning its final ids. Rejected if it would cycle or reorder a lesson already
/// in `visited`. The other half of [`insert_prereq`]; see [`prereq_target`].
pub fn commit_prereq(
    store: &FsCourseStore,
    id: &str,
    settings: &Settings,
    mut prereq: Lesson,
    before: &str,
    visited: &[String],
    now: i64,
) -> Result<(), PlanError> {
    let course = store.load_course(id)?;
    let events = store.events(id)?;
    let mastery = fold_mastery(&course, &events);
    let n = course.lessons.len();
    prereq.id = format!("l{n:02}p");
    for (ii, it) in prereq.items.iter_mut().enumerate() {
        it.id = format!("{}-i{ii}", prereq.id);
    }

    let action = Action::InsertPrereq {
        lesson: Box::new(prereq.clone()),
        before: before.to_string(),
        visited: visited.to_vec(),
    };
    check_action(&course, &events, &mastery, settings, &[], false, &action)?;

    let new_lessons = dag::insert_prereq(&course.lessons, prereq.clone(), before, visited)?;
    let mut course = course;
    course.lessons = new_lessons;
    store.save_course(&course)?;
    store.append_event(
        id,
        &CourseEvent::PrereqInserted {
            ts: now,
            lesson: prereq.id,
            before: before.to_string(),
        },
    )?;
    Ok(())
}

/// F20: repeated failure on `before` writes one short prerequisite lesson (one cheap call) and
/// splices it into the frozen plan ahead of `before`. Rejected if it would cycle or reorder a
/// lesson already in `visited`.
#[allow(clippy::too_many_arguments)]
pub async fn insert_prereq<E: LlmEngine>(
    store: &FsCourseStore,
    engine: &E,
    id: &str,
    settings: &Settings,
    before: &str,
    missing: &str,
    visited: &[String],
    now: i64,
) -> Result<(), PlanError> {
    let target = prereq_target(store, id, before)?;
    let prereq = lp_agent::course::prereq(engine, &target, missing).await?;
    commit_prereq(store, id, settings, prereq, before, visited, now)
}

/// Finds a lesson by id, cloned.
pub fn lesson(store: &FsCourseStore, id: &str, lesson_id: &str) -> Result<Lesson, PlanError> {
    let course = store.load_course(id)?;
    course
        .lessons
        .iter()
        .find(|l| l.id == lesson_id)
        .cloned()
        .ok_or_else(|| PlanError::NotFound(lesson_id.to_string()))
}

/// F18: records a submitted Feynman explanation through the gate. `correct` never blocks the
/// gate (`grading::grade_feynman` always lets it through); it is recorded for the register only.
#[allow(clippy::too_many_arguments)]
pub fn commit_feynman(
    store: &FsCourseStore,
    id: &str,
    settings: &Settings,
    due: &[String],
    lesson: &str,
    correct: bool,
    explanation: String,
    now: i64,
) -> Result<(), PlanError> {
    let action = Action::Feynman {
        lesson: lesson.to_string(),
    };
    let event = CourseEvent::FeynmanGraded {
        ts: now,
        lesson: lesson.to_string(),
        correct,
        explanation,
    };
    apply(store, id, settings, due, true, &action, &event)
}

/// What's next for the learner (`lp_core::method::next_step`), due reviews served first. Purely
/// read-only: `Shown`, `Predicted` and the rest are logged only when the learner acts on the step
/// through its own endpoint (`predict`, `explain_seen`, ...), never here.
pub fn step(
    store: &FsCourseStore,
    tracker: &dyn TrackerStore,
    id: &str,
    settings: &Settings,
    now: i64,
) -> Result<(Course, Step), PlanError> {
    let course = store.load_course(id)?;
    let events = store.events(id)?;
    let mastery = fold_mastery(&course, &events);
    let due = due_items(tracker, id, now)?;
    let step = next_step(&course, &events, &mastery, settings, &due, true);
    Ok((course, step))
}

/// Items of this course due for review at `now`, oldest first: the tracker's due card ids with
/// the `c:{id}:` prefix stripped back to the item id it names.
pub fn due_items(tracker: &dyn TrackerStore, id: &str, now: i64) -> Result<Vec<String>, PlanError> {
    let prefix = format!("c:{id}:");
    Ok(review::due_cards(tracker, now)?
        .into_iter()
        .filter_map(|c| c.id.strip_prefix(&prefix).map(String::from))
        .collect())
}

/// A learner's answer to a quiz item.
#[derive(Clone, Debug)]
pub struct AnswerInput {
    pub item: String,
    pub lesson: String,
    pub text: String,
    pub confidence: Option<u8>,
    pub ms: Option<u64>,
    pub reasoning: String,
    /// "I don't know" (F12): a gap, never a miss.
    pub gap: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnswerOutcome {
    Graded {
        correct: bool,
    },
    /// A recall miss the rubric did not match: waits for a later grader run (F15).
    Pending,
}

/// F12/F13/F15/F19/F21/F27: grades a quiz answer with code (never a model) and appends the
/// matching event through the single gate in [`apply`]; multiple choice is exact comparison
/// (`grading::grade_mcq`), recall tries the rubric first (`grading::submit_recall`), and a gap
/// ("I don't know") is never a miss. Parks the item once its fact's consecutive misses reach
/// `settings.attempts_before_parking` (the caller contract `next_step` documents). When the
/// gated step was a due review (`AskKind::Review`), also reviews its FSRS card — every other
/// kind has no card yet; one is made at close (`close::ensure_cards`).
pub fn quiz_answer(
    store: &FsCourseStore,
    tracker: &dyn TrackerStore,
    device: &str,
    id: &str,
    settings: &Settings,
    input: &AnswerInput,
    now: i64,
) -> Result<AnswerOutcome, PlanError> {
    let course = store.load_course(id)?;
    let events = store.events(id)?;
    let mastery = fold_mastery(&course, &events);
    let due = due_items(tracker, id, now)?;
    let (_, it) =
        find_item(&course, &input.item).ok_or_else(|| PlanError::NotFound(input.item.clone()))?;
    let it = it.clone();
    let kind = match next_step(&course, &events, &mastery, settings, &due, true) {
        Step::Ask {
            item, lesson, kind, ..
        } if item == input.item && lesson == input.lesson => Some(kind),
        _ => None,
    };
    let (event, outcome) = if input.gap {
        (
            CourseEvent::Answered {
                ts: now,
                item: input.item.clone(),
                correct: false,
                chosen: input.text.clone(),
                confidence: input.confidence,
                ms: input.ms,
                gap: true,
                reasoning: input.reasoning.clone(),
            },
            AnswerOutcome::Graded { correct: false },
        )
    } else if it.kind == ItemKind::Mcq {
        let correct = grading::grade_mcq(&it, &input.text);
        (
            CourseEvent::Answered {
                ts: now,
                item: input.item.clone(),
                correct,
                chosen: input.text.clone(),
                confidence: input.confidence,
                ms: input.ms,
                gap: false,
                reasoning: input.reasoning.clone(),
            },
            AnswerOutcome::Graded { correct },
        )
    } else {
        let ev = grading::submit_recall(&input.lesson, &it, &input.text, now);
        let outcome = match &ev {
            CourseEvent::RecallGraded { correct, .. } => {
                AnswerOutcome::Graded { correct: *correct }
            }
            _ => AnswerOutcome::Pending,
        };
        (ev, outcome)
    };
    let action = Action::Answer {
        item: input.item.clone(),
        lesson: input.lesson.clone(),
    };
    apply(store, id, settings, &due, true, &action, &event)?;

    if let Some(fact) = it.tests {
        let mut events = events;
        events.push(event);
        let runner = fold_runner(&course, &events);
        let fk = fact_key(&input.lesson, fact);
        if runner.fact_attempts.get(&fk).copied() == Some(settings.attempts_before_parking) {
            store.append_event(
                id,
                &CourseEvent::ItemParked {
                    ts: now,
                    item: input.item.clone(),
                },
            )?;
        }
    }

    if kind == Some(AskKind::Review) {
        if let AnswerOutcome::Graded { correct } = outcome {
            let cid = card_id(id, &input.item);
            match review::grade(
                tracker,
                device,
                &cid,
                rating_for(correct, input.confidence),
                now,
            ) {
                Ok(()) | Err(StoreError::NotFound(_)) => {}
                Err(e) => return Err(e.into()),
            }
        }
    }

    Ok(outcome)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_cli_driver::engine::FauxEngine;
    use lp_core::course::{Item, ItemKind, Lesson};
    use lp_core::ports::FlashCard;
    use lp_store::SqliteTrackerStore;

    fn course(method_version: u32) -> Course {
        Course {
            id: "c".into(),
            method_version,
            lessons: vec![
                Lesson {
                    id: "l00".into(),
                    title: "Intro".into(),
                    key_facts: vec!["f0".into()],
                    items: vec![Item {
                        id: "l00-i0".into(),
                        q: "q?".into(),
                        answer: "a".into(),
                        options: vec!["a".into(), "b".into(), "c".into(), "d".into()],
                        tests: Some(0),
                        ..Item::default()
                    }],
                    ..Lesson::default()
                },
                Lesson {
                    id: "l01".into(),
                    depends_on: vec!["l00".into()],
                    ..Lesson::default()
                },
            ],
            ..Course::default()
        }
    }

    fn store() -> (tempfile::TempDir, FsCourseStore) {
        let d = tempfile::tempdir().unwrap();
        let s = FsCourseStore::new(d.path());
        (d, s)
    }

    #[test]
    fn apply_rejects_an_out_of_order_action_and_writes_nothing() {
        let (_d, s) = store();
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        s.save_course(&c).unwrap();
        let action = Action::WriteNote {
            lesson: "l00".into(),
        };
        let event = CourseEvent::NoteWritten {
            ts: 0,
            lesson: "l00".into(),
            text: String::new(),
        };
        let settings = Settings::default();
        let err = apply(&s, "c", &settings, &[], false, &action, &event).unwrap_err();
        assert!(matches!(err, PlanError::Gate(GateError::OutOfOrder { .. })));
        assert!(s.events("c").unwrap().is_empty());
    }

    #[test]
    fn apply_accepts_the_expected_action_and_persists_the_event() {
        let (_d, s) = store();
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        s.save_course(&c).unwrap();
        let action = Action::Explain {
            lesson: "l00".into(),
        };
        let event = CourseEvent::Shown {
            ts: 5,
            lesson: "l00".into(),
        };
        let settings = Settings::default();
        apply(&s, "c", &settings, &[], false, &action, &event).unwrap();
        assert_eq!(s.events("c").unwrap(), vec![event]);
    }

    #[test]
    fn confirm_plan_sets_plan_confirmed_at() {
        let (_d, s) = store();
        s.save_course(&course(1)).unwrap();
        confirm_plan(&s, "c", 42).unwrap();
        assert_eq!(s.load_course("c").unwrap().plan_confirmed_at, Some(42));
    }

    #[test]
    fn confirm_plan_rejects_a_cyclic_plan() {
        let (_d, s) = store();
        let mut c = course(1);
        c.lessons[0].depends_on = vec!["l01".into()];
        c.lessons[1].depends_on = vec!["l00".into()];
        s.save_course(&c).unwrap();
        assert!(matches!(
            confirm_plan(&s, "c", 0),
            Err(PlanError::Gate(GateError::PlanHasCycle))
        ));
    }

    #[tokio::test]
    async fn insert_prereq_splices_a_written_lesson_before_the_target() {
        let (_d, s) = store();
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        s.save_course(&c).unwrap();
        let engine = FauxEngine::new();
        engine.faux_text(
            r#"{"title":"Basics","objective":"o","explain":"e","key_facts":["f"],
               "items":[{"type":"mcq","q":"q?","options":["a","b","c","d"],"answer":"a"}]}"#,
        );
        let settings = Settings::default();
        insert_prereq(&s, &engine, "c", &settings, "l00", "missing basics", &[], 7)
            .await
            .unwrap();
        let saved = s.load_course("c").unwrap();
        assert_eq!(saved.lessons.len(), 3);
        let prereq = saved.lessons.iter().find(|l| l.title == "Basics").unwrap();
        assert!(saved
            .lessons
            .iter()
            .find(|l| l.id == "l00")
            .unwrap()
            .depends_on
            .contains(&prereq.id));
        let events = s.events("c").unwrap();
        assert!(matches!(
            events.as_slice(),
            [CourseEvent::PrereqInserted { before, .. }] if before == "l00"
        ));
    }

    #[test]
    fn apply_refuses_confirm_plan_and_insert_prereq_as_a_plain_event() {
        let (_d, s) = store();
        s.save_course(&course(1)).unwrap();
        let settings = Settings::default();
        let err = apply(
            &s,
            "c",
            &settings,
            &[],
            false,
            &Action::ConfirmPlan,
            &CourseEvent::Shown {
                ts: 0,
                lesson: "l00".into(),
            },
        )
        .unwrap_err();
        assert!(matches!(err, PlanError::NeedsDedicatedCall(_)));
    }

    fn flash_card(id: &str, due: i64) -> FlashCard {
        FlashCard {
            id: id.into(),
            front: "q?".into(),
            options: ["a".into(), "b".into(), "c".into(), "d".into()],
            correct_index: 0,
            rationale: String::new(),
            kind: Default::default(),
            must_name: Vec::new(),
            card: lp_core::fsrs::Card::new(due),
        }
    }

    #[test]
    fn step_is_read_only_and_returns_the_gate_s_next_step() {
        let (_d, s) = store();
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        s.save_course(&c).unwrap();
        let td = tempfile::tempdir().unwrap();
        let t = SqliteTrackerStore::open(td.path()).unwrap();
        let settings = Settings::default();
        let (_, st) = step(&s, &t, "c", &settings, 0).unwrap();
        assert_eq!(
            st,
            Step::Explain {
                lesson: "l00".into(),
                rung: None
            }
        );
        assert!(s.events("c").unwrap().is_empty());
    }

    #[test]
    fn due_items_strips_the_card_prefix_and_only_this_course() {
        let td = tempfile::tempdir().unwrap();
        let t = SqliteTrackerStore::open(td.path()).unwrap();
        t.upsert_card(&flash_card("c:c:l00-i0", 0)).unwrap();
        t.upsert_card(&flash_card("c:other:l00-i0", 0)).unwrap();
        assert_eq!(due_items(&t, "c", 100).unwrap(), vec!["l00-i0".to_string()]);
    }

    fn answer_input(item: &str, lesson: &str, text: &str, gap: bool) -> AnswerInput {
        AnswerInput {
            item: item.into(),
            lesson: lesson.into(),
            text: text.into(),
            confidence: None,
            ms: None,
            reasoning: String::new(),
            gap,
        }
    }

    fn ready_course() -> (tempfile::TempDir, FsCourseStore, SqliteTrackerStore) {
        let (d, s) = store();
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        s.save_course(&c).unwrap();
        s.append_event(
            "c",
            &CourseEvent::Shown {
                ts: 0,
                lesson: "l00".into(),
            },
        )
        .unwrap();
        s.append_event(
            "c",
            &CourseEvent::NoteWritten {
                ts: 0,
                lesson: "l00".into(),
                text: String::new(),
            },
        )
        .unwrap();
        let td = tempfile::tempdir().unwrap();
        let t = SqliteTrackerStore::open(td.path()).unwrap();
        (d, s, t)
    }

    #[test]
    fn quiz_answer_grades_mcq_by_exact_text() {
        let (_d, s, t) = ready_course();
        let settings = Settings::default();
        let input = answer_input("l00-i0", "l00", "a", false);
        let out = quiz_answer(&s, &t, "dev", "c", &settings, &input, 10).unwrap();
        assert_eq!(out, AnswerOutcome::Graded { correct: true });
        assert!(matches!(
            s.events("c").unwrap().last().unwrap(),
            CourseEvent::Answered {
                correct: true,
                gap: false,
                ..
            }
        ));
    }

    #[test]
    fn quiz_answer_a_gap_is_never_a_miss_but_never_correct_either() {
        let (_d, s, t) = ready_course();
        let settings = Settings::default();
        let input = answer_input("l00-i0", "l00", "", true);
        let out = quiz_answer(&s, &t, "dev", "c", &settings, &input, 10).unwrap();
        assert_eq!(out, AnswerOutcome::Graded { correct: false });
        assert!(matches!(
            s.events("c").unwrap().last().unwrap(),
            CourseEvent::Answered {
                correct: false,
                gap: true,
                ..
            }
        ));
    }

    #[test]
    fn quiz_answer_parks_the_item_at_the_miss_threshold() {
        let (_d, s, t) = ready_course();
        let settings = Settings::default();
        assert_eq!(settings.attempts_before_parking, 3);
        for text in ["b", "b"] {
            let input = answer_input("l00-i0", "l00", text, false);
            quiz_answer(&s, &t, "dev", "c", &settings, &input, 10).unwrap();
        }
        // Two straight misses offer a break (F21); the test skips past it without the gate (the
        // offer itself is not what is under test here).
        s.append_event(
            "c",
            &CourseEvent::BreakOffered {
                ts: 0,
                lesson: "l00".into(),
            },
        )
        .unwrap();
        let input = answer_input("l00-i0", "l00", "b", false);
        quiz_answer(&s, &t, "dev", "c", &settings, &input, 10).unwrap();
        let events = s.events("c").unwrap();
        let last_two = &events[events.len() - 2..];
        assert!(matches!(
            last_two[0],
            CourseEvent::Answered { correct: false, .. }
        ));
        assert!(matches!(&last_two[1], CourseEvent::ItemParked { item, .. } if item == "l00-i0"));
    }

    #[test]
    fn quiz_answer_reviews_the_fsrs_card_for_a_due_review_item() {
        let (_d, s) = store();
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        s.save_course(&c).unwrap();
        let td = tempfile::tempdir().unwrap();
        let t = SqliteTrackerStore::open(td.path()).unwrap();
        t.upsert_card(&flash_card("c:c:l00-i0", 0)).unwrap();
        let settings = Settings::default();
        let input = answer_input("l00-i0", "l00", "a", false);
        let out = quiz_answer(&s, &t, "dev1", "c", &settings, &input, 1_000).unwrap();
        assert_eq!(out, AnswerOutcome::Graded { correct: true });
        assert!(t.card("c:c:l00-i0").unwrap().unwrap().card.reps > 0);
        assert_eq!(t.reviews().unwrap().len(), 1);
    }

    #[test]
    fn quiz_answer_grades_recall_by_rubric_then_leaves_a_miss_pending() {
        let (_d, s) = store();
        let c = Course {
            id: "c".into(),
            method_version: 1,
            plan_confirmed_at: Some(1),
            lessons: vec![Lesson {
                id: "l00".into(),
                key_facts: vec!["f0".into()],
                items: vec![Item {
                    id: "l00-i0".into(),
                    kind: ItemKind::Recall,
                    q: "q?".into(),
                    answer: "a".into(),
                    must_name: vec!["mitochondria".into()],
                    tests: Some(0),
                    ..Item::default()
                }],
                ..Lesson::default()
            }],
            ..Course::default()
        };
        s.save_course(&c).unwrap();
        s.append_event(
            "c",
            &CourseEvent::Shown {
                ts: 0,
                lesson: "l00".into(),
            },
        )
        .unwrap();
        s.append_event(
            "c",
            &CourseEvent::NoteWritten {
                ts: 0,
                lesson: "l00".into(),
                text: String::new(),
            },
        )
        .unwrap();
        let td = tempfile::tempdir().unwrap();
        let t = SqliteTrackerStore::open(td.path()).unwrap();
        let settings = Settings::default();
        let out = quiz_answer(
            &s,
            &t,
            "dev",
            "c",
            &settings,
            &answer_input("l00-i0", "l00", "something else", false),
            10,
        )
        .unwrap();
        assert_eq!(out, AnswerOutcome::Pending);
        assert!(matches!(
            s.events("c").unwrap().last().unwrap(),
            CourseEvent::RecallPending { .. }
        ));
    }

    #[test]
    fn commit_feynman_appends_through_the_gate_regardless_of_correctness() {
        let (_d, s) = store();
        let c = Course {
            id: "c".into(),
            method_version: 1,
            plan_confirmed_at: Some(1),
            lessons: vec![Lesson {
                id: "l00".into(),
                key_facts: vec!["f0".into()],
                items: vec![Item {
                    id: "l00-i0".into(),
                    kind: ItemKind::Mcq,
                    q: "q".into(),
                    answer: "a".into(),
                    options: vec!["a".into(), "b".into(), "c".into(), "d".into()],
                    tests: Some(0),
                    ..Item::default()
                }],
                ..Lesson::default()
            }],
            ..Course::default()
        };
        s.save_course(&c).unwrap();
        s.append_event(
            "c",
            &CourseEvent::Answered {
                ts: 0,
                item: "l00-i0".into(),
                correct: true,
                chosen: "a".into(),
                confidence: None,
                ms: None,
                gap: false,
                reasoning: String::new(),
            },
        )
        .unwrap();
        let settings = Settings::default();
        commit_feynman(
            &s,
            "c",
            &settings,
            &[],
            "l00",
            false,
            "my explanation".into(),
            5,
        )
        .unwrap();
        assert!(matches!(
            s.events("c").unwrap().last().unwrap(),
            CourseEvent::FeynmanGraded { correct: false, .. }
        ));
    }
}
