//! F07/F20: the single entry point for every learner action on a `method_version >= 1` course.
//! `apply` calls `method::check_action` before it ever touches the store, so no event this
//! service writes on behalf of a learner can skip the gate; `confirm_plan` and `insert_prereq`
//! are the two actions that also change the course itself, not just append an event.

use lp_cli_driver::engine::LlmEngine;
use lp_core::course::CourseEvent;
use lp_core::method::dag::{self, PrereqError};
use lp_core::method::mastery::fold_mastery;
use lp_core::method::{check_action, Action, GateError, Settings};
use lp_core::ports::StoreError;
use lp_store::FsCourseStore;

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
    let course = store.load_course(id)?;
    let events = store.events(id)?;
    let mastery = fold_mastery(&course, &events);
    let target = course
        .lessons
        .iter()
        .find(|l| l.id == before)
        .ok_or_else(|| PlanError::NotFound(before.to_string()))?;

    let mut prereq = lp_agent::course::prereq(engine, target, missing).await?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use lp_cli_driver::engine::FauxEngine;
    use lp_core::course::{Course, Item, Lesson};

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
}
