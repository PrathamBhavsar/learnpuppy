//! The learning method: pure, synchronous rules on top of the course domain in `course.rs`.
//! No IO, no async, no clock — every function here takes `now` and any randomness as plain
//! arguments from its caller.

pub mod dag;
pub mod mastery;
pub mod probe;
pub mod recall;
pub mod register;

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

use crate::course::{ClaimVerdict, Course, CourseEvent, Lesson, LessonKind};
use mastery::{fact_key, FactKey, FactStatus, MasteryState};

/// Starting values from the plan (`brain/learning-method/architecture.md` section 6); every
/// field is a tunable setting, not a fact, and can be changed per workspace.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Settings {
    pub bout_limit_minutes: u32,
    pub gap_between_bouts_minutes: u32,
    pub daily_limit_minutes: u32,
    pub idle_rewind_minutes: u32,
    pub gap_pause_seconds: u32,
    pub gap_pause_every_minutes: u32,
    pub mixed_pass_every: u32,
    pub target_error_rate_pct: u32,
    pub error_band_low_pct: u32,
    pub error_band_high_pct: u32,
    pub attempts_before_parking: u32,
    pub probe_strands: u32,
    pub probe_questions_per_strand: u32,
    pub probe_questions_max: u32,
    pub strong_calls_per_hour: u32,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            bout_limit_minutes: 90,
            gap_between_bouts_minutes: 120,
            daily_limit_minutes: 270,
            idle_rewind_minutes: 10,
            gap_pause_seconds: 10,
            gap_pause_every_minutes: 5,
            mixed_pass_every: 3,
            target_error_rate_pct: 15,
            error_band_low_pct: 8,
            error_band_high_pct: 25,
            attempts_before_parking: 3,
            probe_strands: 3,
            probe_questions_per_strand: 2,
            probe_questions_max: 6,
            strong_calls_per_hour: 2,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum GateError {
    /// F14: the next lesson requires every non-parked fact in this one to reach `mc-passed`.
    #[error("lesson {lesson} still has open facts: {open_facts:?}")]
    MasteryBarOpen {
        lesson: String,
        open_facts: Vec<String>,
    },
    /// F07: no teaching can start before the plan is confirmed.
    #[error("the plan is not confirmed yet")]
    PlanNotConfirmed,
    /// F07: a confirmed plan can never contain a dependency cycle.
    #[error("the plan has a cycle")]
    PlanHasCycle,
    /// F20: inserting this prerequisite would create a cycle in the plan.
    #[error("inserting this prerequisite would create a cycle")]
    PrereqCycle,
    /// F20: inserting this prerequisite would reorder a lesson the learner already visited.
    #[error("inserting this prerequisite would reorder an already visited lesson")]
    PrereqReordersVisited,
    /// Generic reject for any action that does not match what `next_step` would currently
    /// allow (F09, F11, F12, F13, F16, F17, F18, F21, F27 all surface through this).
    #[error("action is out of order, expected: {expected}")]
    OutOfOrder { expected: String },
}

/// What comes next, as seen by the UI (`brain/learning-method/architecture.md` section 1). This
/// covers the teaching-order gates owned by M2 (F05, F07, F09-F13, F16-F18, F20, F21, F27); the
/// bout clock and gap pauses (F23, F24, `Ready`, `Gap`, `Locked`) are a separate layer, owned by
/// `bout.rs`/`pacing.rs` (M3), that wraps around this.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum Step {
    /// F07: the plan map needs the learner's confirmation before any teaching.
    ConfirmPlan,
    /// F09: an ungraded prediction before a conceptual explanation.
    Predict { lesson: String },
    /// F10/F16: the lesson's explanation, or one rung of a procedure's faded example.
    Explain { lesson: String, rung: Option<usize> },
    /// F11: the notes gate between explanation and quiz.
    Notes { lesson: String },
    /// F12/F13/F17/F27: a quiz item. `angle` is set when this follows a miss and should show a
    /// stored alternate explanation first; `kind` says why this item was chosen.
    Ask {
        item: String,
        lesson: String,
        kind: AskKind,
        angle: Option<usize>,
    },
    /// F21: offered once per lesson, after two straight misses, instead of a third angle.
    Break { lesson: String },
    /// F18: explain it to a ten-year-old, required on load-bearing and goal lessons.
    Feynman { lesson: String },
    /// Every non-parked fact in every lesson has reached at least `mc-passed`.
    Close,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AskKind {
    /// F05: a due review, served before new material at the start of a session.
    Review,
    /// A fresh item, never attempted.
    New,
    /// F27: a different item on the same fact, right after a miss.
    Retry,
    /// F17: every `mixed_pass_every`-th quiz, an item from an earlier, already-taught lesson.
    Mixed,
}

/// A learner action proposed to the server, to be validated by `check_action` before it is
/// appended to the event log.
#[derive(Clone, Debug, PartialEq)]
pub enum Action {
    /// F14: leave a lesson behind (checked at M1; kept here as part of the one gate entry point).
    AdvanceLesson {
        lesson: String,
    },
    /// F07: confirm the plan map.
    ConfirmPlan,
    Predict {
        lesson: String,
    },
    Explain {
        lesson: String,
    },
    WriteNote {
        lesson: String,
    },
    Answer {
        item: String,
        lesson: String,
    },
    Feynman {
        lesson: String,
    },
    TakeBreak {
        lesson: String,
    },
    /// F20: insert a short prerequisite lesson before `before` into a frozen plan.
    InsertPrereq {
        lesson: Box<Lesson>,
        before: String,
        visited: Vec<String>,
    },
    Close,
}

/// Folded state for the M2 runner: everything `next_step` needs beyond the mastery bar (which
/// `method::mastery` already folds). Pure and prefix-safe, like every other fold in this crate.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct RunnerState {
    pub predicted: HashSet<String>,
    pub notes_written: HashSet<String>,
    pub feynman_done: HashSet<String>,
    pub break_offered: HashSet<String>,
    /// The rung index that was current the last time this lesson's explanation (or rung) was
    /// shown; `None` if never shown. A mismatch with the current rung means re-explain (F16).
    pub shown_at_rung: HashMap<String, usize>,
    /// Consecutive misses per fact, reset to 0 on a correct answer (F13/F21/F27).
    pub fact_attempts: HashMap<FactKey, u32>,
    /// The item most recently used to test each fact, so a retry prefers a different one.
    pub fact_last_item: HashMap<FactKey, String>,
    pub parked: HashSet<String>,
    /// Procedural lessons only: which rung (0 = full, up to `rungs.len() - 1`) is current.
    pub rung_progress: HashMap<String, usize>,
    /// Count of resolved quiz turns across the whole course, for F17's every-Nth check.
    pub quiz_count: u32,
}

fn advance_rung(progress: usize, correct: bool, len: usize) -> usize {
    if len == 0 {
        return 0;
    }
    if correct {
        (progress + 1).min(len - 1)
    } else {
        progress.saturating_sub(1)
    }
}

/// Rebuild the M2 runner state from the event log.
pub fn fold_runner(course: &Course, events: &[CourseEvent]) -> RunnerState {
    struct ItemMeta<'a> {
        lesson: &'a str,
        fact: Option<usize>,
    }
    let mut meta: HashMap<&str, ItemMeta> = HashMap::new();
    let mut rung_len: HashMap<&str, usize> = HashMap::new();
    for l in &course.lessons {
        if l.kind == LessonKind::Procedural {
            rung_len.insert(l.id.as_str(), l.rungs.len());
        }
        for it in &l.items {
            meta.insert(
                it.id.as_str(),
                ItemMeta {
                    lesson: l.id.as_str(),
                    fact: it.tests,
                },
            );
        }
    }

    let mut st = RunnerState::default();
    let record_outcome = |item: &str, correct: bool, gap: bool, st: &mut RunnerState| {
        st.quiz_count += 1;
        let Some(m) = meta.get(item) else { return };
        if let Some(len) = rung_len.get(m.lesson) {
            let p = st.rung_progress.entry(m.lesson.to_string()).or_default();
            *p = advance_rung(*p, correct, *len);
        }
        let Some(fact) = m.fact else { return };
        let fk = fact_key(m.lesson, fact);
        st.fact_last_item.insert(fk.clone(), item.to_string());
        if gap {
            return;
        }
        if correct {
            st.fact_attempts.insert(fk, 0);
        } else {
            *st.fact_attempts.entry(fk).or_default() += 1;
        }
    };

    for ev in events {
        match ev {
            CourseEvent::Predicted { lesson, .. } => {
                st.predicted.insert(lesson.clone());
            }
            CourseEvent::NoteWritten { lesson, .. } => {
                st.notes_written.insert(lesson.clone());
            }
            CourseEvent::FeynmanGraded { lesson, .. } => {
                st.feynman_done.insert(lesson.clone());
            }
            CourseEvent::BreakOffered { lesson, .. } => {
                st.break_offered.insert(lesson.clone());
            }
            CourseEvent::Shown { lesson, .. } => {
                let rung = st.rung_progress.get(lesson).copied().unwrap_or(0);
                st.shown_at_rung.insert(lesson.clone(), rung);
            }
            CourseEvent::Answered {
                item, correct, gap, ..
            } => {
                record_outcome(item, *correct, *gap, &mut st);
            }
            CourseEvent::RecallGraded { item, correct, .. } => {
                record_outcome(item, *correct, false, &mut st);
            }
            CourseEvent::ItemParked { item, .. } => {
                st.parked.insert(item.clone());
                if let Some(m) = meta.get(item.as_str()) {
                    if let Some(fact) = m.fact {
                        st.fact_attempts.insert(fact_key(m.lesson, fact), 0);
                    }
                }
            }
            _ => {}
        }
    }
    st
}

fn mastery_bar_gate(
    course: &Course,
    lesson_id: &str,
    mastery: &MasteryState,
) -> Result<(), GateError> {
    let Some(lesson) = course.lessons.iter().find(|l| l.id == lesson_id) else {
        return Ok(());
    };
    let open: Vec<String> = lesson
        .key_facts
        .iter()
        .enumerate()
        .filter(|(i, _)| {
            !matches!(
                mastery.status(lesson_id, *i),
                FactStatus::McPassed | FactStatus::Confirmed | FactStatus::Parked
            )
        })
        .map(|(_, f)| f.clone())
        .collect();
    if open.is_empty() {
        Ok(())
    } else {
        Err(GateError::MasteryBarOpen {
            lesson: lesson_id.to_string(),
            open_facts: open,
        })
    }
}

/// Lessons no other lesson depends on: terminal, "goal" lessons (F18).
fn goal_lessons(lessons: &[Lesson]) -> HashSet<String> {
    let dependents_of: HashSet<&str> = lessons
        .iter()
        .flat_map(|l| l.depends_on.iter().map(String::as_str))
        .collect();
    lessons
        .iter()
        .filter(|l| !dependents_of.contains(l.id.as_str()))
        .map(|l| l.id.clone())
        .collect()
}

/// An item from an earlier (already topo-ordered) lesson, for F17's mixed pass: the first
/// eligible item of the first earlier lesson that still has one.
fn earlier_unresolved_item(
    course: &Course,
    order: &[String],
    before: &str,
    runner: &RunnerState,
    flagged: &HashSet<&str>,
    contradicted: &HashSet<&str>,
) -> Option<(String, String)> {
    let pos = order.iter().position(|l| l == before)?;
    for lid in &order[..pos] {
        if contradicted.contains(lid.as_str()) {
            continue;
        }
        let lesson = course.lessons.iter().find(|l| &l.id == lid)?;
        if let Some(it) = lesson
            .items
            .iter()
            .find(|it| !flagged.contains(it.id.as_str()) && !runner.parked.contains(&it.id))
        {
            return Some((lid.clone(), it.id.clone()));
        }
    }
    None
}

/// What comes next for a method_version >= 1 course. Caller contract: append a `Shown` event
/// whenever an explanation (or one rung of a procedure) is shown; when an `Answered` or
/// `RecallGraded` miss brings a fact's consecutive-miss count to `settings.attempts_before_parking`,
/// append an `ItemParked` event for that item in the same batch; pass `start = true` only at the
/// start of a session, so due reviews (F05) are served first. `due` holds item ids already
/// computed from FSRS. A `method_version == 0` course (pre-dating this plan) always returns
/// `Close`: it runs on the older `course::next_step` engine instead, with none of these gates.
pub fn next_step(
    course: &Course,
    events: &[CourseEvent],
    mastery: &MasteryState,
    settings: &Settings,
    due: &[String],
    start: bool,
) -> Step {
    if course.method_version == 0 {
        return Step::Close;
    }
    if course.plan_confirmed_at.is_none() {
        return Step::ConfirmPlan;
    }

    let learner = crate::course::fold_learner(course, events);
    let flagged: HashSet<&str> = learner.flagged.iter().map(String::as_str).collect();
    let contradicted_lessons: HashSet<&str> = course
        .lessons
        .iter()
        .filter(|l| {
            l.claims
                .iter()
                .any(|c| c.verdict == ClaimVerdict::Contradicted)
        })
        .map(|l| l.id.as_str())
        .collect();

    if start {
        for id in due {
            if flagged.contains(id.as_str()) {
                continue;
            }
            let Some((lesson, _)) = crate::course::find_item(course, id) else {
                continue;
            };
            if contradicted_lessons.contains(lesson.id.as_str()) {
                continue;
            }
            return Step::Ask {
                item: id.clone(),
                lesson: lesson.id.clone(),
                kind: AskKind::Review,
                angle: None,
            };
        }
    }

    let order = dag::topo_order(&course.lessons)
        .unwrap_or_else(|_| course.lessons.iter().map(|l| l.id.clone()).collect());
    let runner = fold_runner(course, events);
    let load_bearing = dag::load_bearing(&course.lessons);
    let goals = goal_lessons(&course.lessons);

    for lid in &order {
        if contradicted_lessons.contains(lid.as_str()) {
            continue;
        }
        let Some(lesson) = course.lessons.iter().find(|l| &l.id == lid) else {
            continue;
        };

        if mastery_bar_gate(course, lid, mastery).is_ok() {
            let needs_feynman = (load_bearing.contains(lid) || goals.contains(lid))
                && !runner.feynman_done.contains(lid);
            if needs_feynman {
                return Step::Feynman {
                    lesson: lid.clone(),
                };
            }
            continue;
        }

        if lesson.kind != LessonKind::Definitional
            && lesson.predict.is_some()
            && !runner.predicted.contains(lid)
        {
            return Step::Predict {
                lesson: lid.clone(),
            };
        }

        let rung = if lesson.kind == LessonKind::Procedural && !lesson.rungs.is_empty() {
            Some(runner.rung_progress.get(lid).copied().unwrap_or(0))
        } else {
            None
        };
        if runner.shown_at_rung.get(lid).copied() != Some(rung.unwrap_or(0)) {
            return Step::Explain {
                lesson: lid.clone(),
                rung,
            };
        }

        if !runner.notes_written.contains(lid) {
            return Step::Notes {
                lesson: lid.clone(),
            };
        }

        // Quiz phase: the first non-passed, non-parked fact drives item selection.
        let Some(fact_idx) = (0..lesson.key_facts.len()).find(|&i| {
            !matches!(
                mastery.status(lid, i),
                FactStatus::McPassed | FactStatus::Confirmed | FactStatus::Parked
            )
        }) else {
            // The mastery bar disagreed (shouldn't happen since it was just checked above);
            // move on rather than getting stuck.
            continue;
        };
        let fk = fact_key(lid, fact_idx);
        let attempts = runner.fact_attempts.get(&fk).copied().unwrap_or(0);

        if attempts == 2 && !runner.break_offered.contains(lid) {
            return Step::Break {
                lesson: lid.clone(),
            };
        }

        let candidates: Vec<&crate::course::Item> = lesson
            .items
            .iter()
            .filter(|it| it.tests == Some(fact_idx))
            .filter(|it| !flagged.contains(it.id.as_str()) && !runner.parked.contains(&it.id))
            .collect();
        let Some(&item) = (match runner.fact_last_item.get(&fk) {
            Some(last) => candidates
                .iter()
                .find(|it| &it.id != last)
                .or_else(|| candidates.first()),
            None => candidates.first(),
        }) else {
            // Nothing left to ask on this fact (all parked); move on rather than getting stuck.
            continue;
        };

        if attempts == 0
            && settings.mixed_pass_every > 0
            && (runner.quiz_count + 1).is_multiple_of(settings.mixed_pass_every)
        {
            if let Some((el, ei)) = earlier_unresolved_item(
                course,
                &order,
                lid,
                &runner,
                &flagged,
                &contradicted_lessons,
            ) {
                return Step::Ask {
                    item: ei,
                    lesson: el,
                    kind: AskKind::Mixed,
                    angle: None,
                };
            }
        }

        let angle = (attempts > 0 && !lesson.angles.is_empty())
            .then(|| ((attempts - 1) as usize).min(lesson.angles.len() - 1));
        let kind = if attempts > 0 {
            AskKind::Retry
        } else {
            AskKind::New
        };
        return Step::Ask {
            item: item.id.clone(),
            lesson: lid.clone(),
            kind,
            angle,
        };
    }
    Step::Close
}

fn action_matches(action: &Action, step: &Step) -> bool {
    match (action, step) {
        (Action::Predict { lesson }, Step::Predict { lesson: l2 }) => lesson == l2,
        (Action::Explain { lesson }, Step::Explain { lesson: l2, .. }) => lesson == l2,
        (Action::WriteNote { lesson }, Step::Notes { lesson: l2 }) => lesson == l2,
        (Action::Answer { item, .. }, Step::Ask { item: i2, .. }) => item == i2,
        (Action::Feynman { lesson }, Step::Feynman { lesson: l2 }) => lesson == l2,
        (Action::TakeBreak { lesson }, Step::Break { lesson: l2 }) => lesson == l2,
        (Action::Close, Step::Close) => true,
        _ => false,
    }
}

/// The single gate every learner action passes through before it is appended as an event.
/// `method_version == 0` skips every gate here, so an old course keeps behaving exactly as it
/// did before this plan (it uses `course::check_action`... there is none; it simply has no
/// gates, and runs on `course::next_step` instead).
pub fn check_action(
    course: &Course,
    events: &[CourseEvent],
    mastery: &MasteryState,
    settings: &Settings,
    due: &[String],
    start: bool,
    action: &Action,
) -> Result<(), GateError> {
    if course.method_version == 0 {
        return Ok(());
    }
    match action {
        Action::AdvanceLesson { lesson } => mastery_bar_gate(course, lesson, mastery),
        Action::ConfirmPlan => {
            if dag::has_cycle(&course.lessons) {
                Err(GateError::PlanHasCycle)
            } else {
                Ok(())
            }
        }
        Action::InsertPrereq {
            lesson,
            before,
            visited,
        } => dag::insert_prereq(&course.lessons, (**lesson).clone(), before, visited)
            .map(|_| ())
            .map_err(|e| match e {
                dag::PrereqError::Cycle => GateError::PrereqCycle,
                dag::PrereqError::ReordersVisited => GateError::PrereqReordersVisited,
            }),
        _ => {
            let expected = next_step(course, events, mastery, settings, due, start);
            if action_matches(action, &expected) {
                Ok(())
            } else {
                Err(GateError::OutOfOrder {
                    expected: format!("{expected:?}"),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::{Claim, Course, Item, ItemKind, Lesson, Prediction, Rung, RungKind};
    use proptest::prelude::*;

    fn course(method_version: u32) -> Course {
        Course {
            id: "c".into(),
            method_version,
            lessons: vec![Lesson {
                id: "l00".into(),
                key_facts: vec!["f0".into(), "f1".into()],
                ..Lesson::default()
            }],
            ..Course::default()
        }
    }

    fn mcq_item(id: &str, tests: usize, misconceptions: [&str; 4]) -> Item {
        Item {
            id: id.into(),
            kind: ItemKind::Mcq,
            q: format!("{id}?"),
            options: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            answer: "a".into(),
            tests: Some(tests),
            misconceptions: misconceptions.iter().map(|s| s.to_string()).collect(),
            ..Item::default()
        }
    }

    fn answer(item: &str, correct: bool) -> CourseEvent {
        CourseEvent::Answered {
            ts: 0,
            item: item.into(),
            correct,
            chosen: if correct { "a".into() } else { "b".into() },
            confidence: None,
            ms: None,
            gap: false,
            reasoning: String::new(),
        }
    }

    fn shown(lesson: &str) -> CourseEvent {
        CourseEvent::Shown {
            ts: 0,
            lesson: lesson.into(),
        }
    }

    // ---------------------------------------------------------------- F14 (M1, kept)

    #[test]
    fn settings_defaults_match_the_plan() {
        let s = Settings::default();
        assert_eq!(s.bout_limit_minutes, 90);
        assert_eq!(s.gap_between_bouts_minutes, 120);
        assert_eq!(s.daily_limit_minutes, 270);
        assert_eq!(s.attempts_before_parking, 3);
        assert_eq!(s.mixed_pass_every, 3);
    }

    #[test]
    fn version_zero_skips_the_mastery_bar_gate() {
        let c = course(0);
        let mastery = MasteryState::default();
        let action = Action::AdvanceLesson {
            lesson: "l00".into(),
        };
        assert!(check_action(&c, &[], &mastery, &Settings::default(), &[], false, &action).is_ok());
    }

    #[test]
    fn open_facts_block_advancing_the_lesson() {
        let c = course(1);
        let mastery = MasteryState::default();
        let action = Action::AdvanceLesson {
            lesson: "l00".into(),
        };
        let err =
            check_action(&c, &[], &mastery, &Settings::default(), &[], false, &action).unwrap_err();
        assert_eq!(
            err,
            GateError::MasteryBarOpen {
                lesson: "l00".into(),
                open_facts: vec!["f0".into(), "f1".into()],
            }
        );
    }

    #[test]
    fn mc_passed_or_parked_facts_pass_the_gate() {
        let c = course(1);
        let mut mastery = MasteryState::default();
        mastery
            .facts
            .insert(fact_key("l00", 0), FactStatus::McPassed);
        mastery.facts.insert(fact_key("l00", 1), FactStatus::Parked);
        let action = Action::AdvanceLesson {
            lesson: "l00".into(),
        };
        assert!(check_action(&c, &[], &mastery, &Settings::default(), &[], false, &action).is_ok());
    }

    // ---------------------------------------------------------------- one table test per gate

    /// F05: a due review is served before any new teaching, at the start of a session.
    #[test]
    fn gate_f05_review_first() {
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        c.lessons[0].items = vec![mcq_item("l00-i0", 0, ["", "", "", ""])];
        let due = vec!["l00-i0".to_string()];
        let mastery = MasteryState::default();
        let step = next_step(&c, &[], &mastery, &Settings::default(), &due, true);
        assert_eq!(
            step,
            Step::Ask {
                item: "l00-i0".into(),
                lesson: "l00".into(),
                kind: AskKind::Review,
                angle: None,
            }
        );
    }

    /// F07: no teaching before the plan is confirmed, and a cyclic plan can never be confirmed.
    #[test]
    fn gate_f07_plan_must_be_confirmed_before_teaching() {
        let c = course(1); // plan_confirmed_at is None
        let mastery = MasteryState::default();
        assert_eq!(
            next_step(&c, &[], &mastery, &Settings::default(), &[], false),
            Step::ConfirmPlan
        );
        let action = Action::Explain {
            lesson: "l00".into(),
        };
        assert_eq!(
            check_action(&c, &[], &mastery, &Settings::default(), &[], false, &action),
            Err(GateError::OutOfOrder {
                expected: format!("{:?}", Step::ConfirmPlan)
            })
        );

        let mut cyclic = course(1);
        cyclic.lessons[0].depends_on = vec!["l00".into()];
        assert_eq!(
            check_action(
                &cyclic,
                &[],
                &mastery,
                &Settings::default(),
                &[],
                false,
                &Action::ConfirmPlan
            ),
            Err(GateError::PlanHasCycle)
        );
    }

    /// F09: a conceptual lesson's prediction must come before its explanation.
    #[test]
    fn gate_f09_predict_before_explain() {
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        c.lessons[0].predict = Some(Prediction {
            q: "guess?".into(),
            options: vec!["a".into(), "b".into()],
        });
        let mastery = MasteryState::default();
        assert_eq!(
            next_step(&c, &[], &mastery, &Settings::default(), &[], false),
            Step::Predict {
                lesson: "l00".into()
            }
        );
        let predicted = [CourseEvent::Predicted {
            ts: 0,
            lesson: "l00".into(),
            guess: "a".into(),
        }];
        assert_eq!(
            next_step(&c, &predicted, &mastery, &Settings::default(), &[], false),
            Step::Explain {
                lesson: "l00".into(),
                rung: None
            }
        );
    }

    /// F10: each lesson is one explanation step; a definitional lesson skips F09's predict.
    #[test]
    fn gate_f10_definitional_lessons_skip_prediction() {
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        c.lessons[0].kind = LessonKind::Definitional;
        c.lessons[0].predict = Some(Prediction::default());
        let mastery = MasteryState::default();
        assert_eq!(
            next_step(&c, &[], &mastery, &Settings::default(), &[], false),
            Step::Explain {
                lesson: "l00".into(),
                rung: None
            }
        );
    }

    /// F11: the notes gate sits between explanation and the first quiz item.
    #[test]
    fn gate_f11_notes_before_quiz() {
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        c.lessons[0].items = vec![mcq_item("l00-i0", 0, ["", "", "", ""])];
        let mastery = MasteryState::default();
        let events = [shown("l00")];
        assert_eq!(
            next_step(&c, &events, &mastery, &Settings::default(), &[], false),
            Step::Notes {
                lesson: "l00".into()
            }
        );
        let action = Action::Answer {
            item: "l00-i0".into(),
            lesson: "l00".into(),
        };
        assert_eq!(
            check_action(
                &c,
                &events,
                &mastery,
                &Settings::default(),
                &[],
                false,
                &action
            ),
            Err(GateError::OutOfOrder {
                expected: format!(
                    "{:?}",
                    Step::Notes {
                        lesson: "l00".into()
                    }
                )
            })
        );
    }

    /// F12: the honest quiz presents an unanswered item once teaching is complete.
    #[test]
    fn gate_f12_quiz_after_explain_and_notes() {
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        c.lessons[0].items = vec![mcq_item("l00-i0", 0, ["", "", "", ""])];
        let mastery = MasteryState::default();
        let events = [
            shown("l00"),
            CourseEvent::NoteWritten {
                ts: 0,
                lesson: "l00".into(),
                text: String::new(),
            },
        ];
        assert_eq!(
            next_step(&c, &events, &mastery, &Settings::default(), &[], false),
            Step::Ask {
                item: "l00-i0".into(),
                lesson: "l00".into(),
                kind: AskKind::New,
                angle: None,
            }
        );
    }

    /// F13: a miss shows a stored alternate explanation (angle) before the next attempt.
    #[test]
    fn gate_f13_no_reveal_reteach_shows_an_angle_after_a_miss() {
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        c.lessons[0].items = vec![
            mcq_item("l00-i0", 0, ["", "", "", ""]),
            mcq_item("l00-i1", 0, ["", "", "", ""]),
        ];
        c.lessons[0].angles = vec!["angle one".into(), "angle two".into()];
        let mastery = MasteryState::default();
        let events = [
            shown("l00"),
            CourseEvent::NoteWritten {
                ts: 0,
                lesson: "l00".into(),
                text: String::new(),
            },
            answer("l00-i0", false),
        ];
        let step = next_step(&c, &events, &mastery, &Settings::default(), &[], false);
        match step {
            Step::Ask {
                item, kind, angle, ..
            } => {
                assert_eq!(item, "l00-i1"); // a different item on the same fact (F27)
                assert_eq!(kind, AskKind::Retry);
                assert_eq!(angle, Some(0));
            }
            other => panic!("expected a retry Ask with an angle, got {other:?}"),
        }
    }

    /// F16: a procedural lesson's rungs show in order, full before blanked before solo.
    #[test]
    fn gate_f16_faded_examples_show_in_order() {
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        c.lessons[0].kind = LessonKind::Procedural;
        c.lessons[0].rungs = vec![
            Rung {
                kind: RungKind::Full,
                text: "full".into(),
            },
            Rung {
                kind: RungKind::Blanked,
                text: "blanked".into(),
            },
            Rung {
                kind: RungKind::Solo,
                text: "solo".into(),
            },
        ];
        c.lessons[0].items = vec![mcq_item("l00-i0", 0, ["", "", "", ""])];
        let mastery = MasteryState::default();
        assert_eq!(
            next_step(&c, &[], &mastery, &Settings::default(), &[], false),
            Step::Explain {
                lesson: "l00".into(),
                rung: Some(0)
            }
        );
        // A right answer at the floor rung advances to the next rung and asks for it to be
        // explained again before testing it.
        let events = [
            shown("l00"),
            CourseEvent::NoteWritten {
                ts: 0,
                lesson: "l00".into(),
                text: String::new(),
            },
            answer("l00-i0", true),
        ];
        assert_eq!(
            next_step(&c, &events, &mastery, &Settings::default(), &[], false),
            Step::Explain {
                lesson: "l00".into(),
                rung: Some(1)
            }
        );
        // A miss after that drops the rung back down, again asking for a re-explain.
        let mut events = events.to_vec();
        events.push(shown("l00"));
        events.push(answer("l00-i0", false));
        assert_eq!(
            next_step(&c, &events, &mastery, &Settings::default(), &[], false),
            Step::Explain {
                lesson: "l00".into(),
                rung: Some(0)
            }
        );
    }

    /// F17: every third quiz draws from an earlier, already-taught lesson, unlabelled from the
    /// learner's point of view but tagged `Mixed` so it still counts for mastery.
    #[test]
    fn gate_f17_every_third_quiz_is_mixed() {
        let mut c = Course {
            id: "c".into(),
            method_version: 1,
            plan_confirmed_at: Some(1),
            lessons: vec![
                Lesson {
                    id: "l00".into(),
                    key_facts: vec!["f0".into()],
                    items: vec![mcq_item("l00-i0", 0, ["", "", "", ""])],
                    ..Lesson::default()
                },
                Lesson {
                    id: "l01".into(),
                    key_facts: vec!["f0".into()],
                    depends_on: vec!["l00".into()],
                    items: vec![mcq_item("l01-i0", 0, ["", "", "", ""])],
                    ..Lesson::default()
                },
            ],
            ..Course::default()
        };
        c.lessons[1].angles = vec![];
        let mastery_l00 = MasteryState {
            facts: [(fact_key("l00", 0), FactStatus::McPassed)].into(),
        };
        let events = [
            shown("l00"),
            CourseEvent::NoteWritten {
                ts: 0,
                lesson: "l00".into(),
                text: String::new(),
            },
            answer("l00-i0", true),
            shown("l01"),
            CourseEvent::NoteWritten {
                ts: 0,
                lesson: "l01".into(),
                text: String::new(),
            },
        ];
        // quiz_count is already 1 (the l00 answer); the next one is the 2nd, not mixed yet.
        let settings = Settings {
            mixed_pass_every: 2,
            ..Settings::default()
        };
        let step = next_step(&c, &events, &mastery_l00, &settings, &[], false);
        assert_eq!(
            step,
            Step::Ask {
                item: "l00-i0".into(),
                lesson: "l00".into(),
                kind: AskKind::Mixed,
                angle: None,
            }
        );
    }

    /// F18: a Feynman pass is required on load-bearing and goal lessons once they are mastered.
    #[test]
    fn gate_f18_feynman_required_on_goal_lessons() {
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        let mastery = MasteryState {
            facts: [
                (fact_key("l00", 0), FactStatus::McPassed),
                (fact_key("l00", 1), FactStatus::McPassed),
            ]
            .into(),
        };
        assert_eq!(
            next_step(&c, &[], &mastery, &Settings::default(), &[], false),
            Step::Feynman {
                lesson: "l00".into()
            }
        );
        let events = [CourseEvent::FeynmanGraded {
            ts: 0,
            lesson: "l00".into(),
            correct: true,
            explanation: "it just does".into(),
        }];
        assert_eq!(
            next_step(&c, &events, &mastery, &Settings::default(), &[], false),
            Step::Close
        );
    }

    /// F20: a prerequisite insert is rejected if it would cycle or reorder a visited lesson,
    /// accepted (and keeps visited order) otherwise.
    #[test]
    fn gate_f20_prereq_insert_rejects_cycles_and_reorders() {
        let c = Course {
            id: "c".into(),
            method_version: 1,
            plan_confirmed_at: Some(1),
            lessons: vec![
                Lesson {
                    id: "a".into(),
                    ..Lesson::default()
                },
                Lesson {
                    id: "b".into(),
                    depends_on: vec!["a".into()],
                    ..Lesson::default()
                },
            ],
            ..Course::default()
        };
        let mastery = MasteryState::default();
        let settings = Settings::default();

        let ok = Action::InsertPrereq {
            lesson: Box::new(Lesson {
                id: "p".into(),
                ..Lesson::default()
            }),
            before: "b".into(),
            visited: vec!["a".into()],
        };
        assert!(check_action(&c, &[], &mastery, &settings, &[], false, &ok).is_ok());

        let cyclic = Action::InsertPrereq {
            lesson: Box::new(Lesson {
                id: "p".into(),
                depends_on: vec!["b".into()],
                ..Lesson::default()
            }),
            before: "a".into(),
            visited: vec![],
        };
        assert_eq!(
            check_action(&c, &[], &mastery, &settings, &[], false, &cyclic),
            Err(GateError::PrereqCycle)
        );
    }

    /// F21: two straight misses on one fact offer a break, once, instead of a third angle.
    #[test]
    fn gate_f21_break_offered_after_two_straight_misses() {
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        c.lessons[0].items = vec![
            mcq_item("l00-i0", 0, ["", "", "", ""]),
            mcq_item("l00-i1", 0, ["", "", "", ""]),
        ];
        let mastery = MasteryState::default();
        let events = [
            shown("l00"),
            CourseEvent::NoteWritten {
                ts: 0,
                lesson: "l00".into(),
                text: String::new(),
            },
            answer("l00-i0", false),
            answer("l00-i1", false),
        ];
        assert_eq!(
            next_step(&c, &events, &mastery, &Settings::default(), &[], false),
            Step::Break {
                lesson: "l00".into()
            }
        );
        let action = Action::Answer {
            item: "l00-i0".into(),
            lesson: "l00".into(),
        };
        assert!(check_action(
            &c,
            &events,
            &mastery,
            &Settings::default(),
            &[],
            false,
            &action
        )
        .is_err());

        // Once offered, it is not offered again; the retry resumes.
        let mut after = events.to_vec();
        after.push(CourseEvent::BreakOffered {
            ts: 0,
            lesson: "l00".into(),
        });
        let step = next_step(&c, &after, &mastery, &Settings::default(), &[], false);
        assert!(matches!(
            step,
            Step::Ask {
                kind: AskKind::Retry,
                ..
            }
        ));
    }

    /// F27: right after a miss, the very next step tests the same fact (with a different item
    /// when one exists), not something unrelated.
    #[test]
    fn gate_f27_retry_hits_the_same_fact() {
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        c.lessons[0].items = vec![
            mcq_item("l00-i0", 0, ["", "", "", ""]),
            mcq_item("l00-i1", 1, ["", "", "", ""]),
        ];
        let mastery = MasteryState::default();
        let events = [
            shown("l00"),
            CourseEvent::NoteWritten {
                ts: 0,
                lesson: "l00".into(),
                text: String::new(),
            },
            answer("l00-i0", false),
        ];
        let step = next_step(&c, &events, &mastery, &Settings::default(), &[], false);
        assert_eq!(
            step,
            Step::Ask {
                item: "l00-i0".into(), // only item testing fact 0; reused since no sibling exists
                lesson: "l00".into(),
                kind: AskKind::Retry,
                angle: None,
            }
        );
    }

    /// Flagged items and items in a lesson with a contradicted claim never appear.
    #[test]
    fn flagged_and_contradicted_items_never_appear() {
        let mut c = course(1);
        c.plan_confirmed_at = Some(1);
        c.lessons[0].items = vec![mcq_item("l00-i0", 0, ["", "", "", ""])];
        c.lessons[0].claims = vec![Claim {
            text: "a claim".into(),
            verdict: ClaimVerdict::Contradicted,
            source: None,
        }];
        let mastery = MasteryState::default();
        assert_eq!(
            next_step(&c, &[], &mastery, &Settings::default(), &[], false),
            Step::Close
        );
    }

    // A learner who always answers correctly always reaches Close, regardless of plan shape.
    proptest! {
        #[test]
        fn a_learner_can_always_reach_close(
            n_lessons in 1usize..5,
            n_facts in 1usize..3,
        ) {
            let mut lessons = Vec::new();
            for li in 0..n_lessons {
                let id = format!("l{li}");
                let deps = if li == 0 { vec![] } else { vec![format!("l{}", li - 1)] };
                let items: Vec<Item> = (0..n_facts)
                    .map(|fi| mcq_item(&format!("l{li}-i{fi}"), fi, ["", "", "", ""]))
                    .collect();
                lessons.push(Lesson {
                    id,
                    key_facts: (0..n_facts).map(|fi| format!("f{fi}")).collect(),
                    depends_on: deps,
                    items,
                    ..Lesson::default()
                });
            }
            let mut c = Course {
                id: "c".into(),
                method_version: 1,
                plan_confirmed_at: Some(1),
                lessons,
                ..Course::default()
            };
            c.plan_confirmed_at = Some(1);
            let settings = Settings::default();
            let mut events: Vec<CourseEvent> = Vec::new();
            let mut reached_close = false;
            for _ in 0..(n_lessons * n_facts * 6 + 20) {
                let mastery = mastery::fold_mastery(&c, &events);
                let step = next_step(&c, &events, &mastery, &settings, &[], false);
                match step {
                    Step::Close => { reached_close = true; break; }
                    Step::ConfirmPlan => { c.plan_confirmed_at = Some(1); }
                    Step::Predict { lesson } => events.push(CourseEvent::Predicted { ts: 0, lesson, guess: "x".into() }),
                    Step::Explain { lesson, .. } => events.push(shown(&lesson)),
                    Step::Notes { lesson } => events.push(CourseEvent::NoteWritten { ts: 0, lesson, text: String::new() }),
                    Step::Ask { item, .. } => events.push(answer(&item, true)),
                    Step::Break { lesson } => events.push(CourseEvent::BreakOffered { ts: 0, lesson }),
                    Step::Feynman { lesson } => events.push(CourseEvent::FeynmanGraded { ts: 0, lesson, correct: true, explanation: String::new() }),
                }
            }
            prop_assert!(reached_close);
        }
    }
}
