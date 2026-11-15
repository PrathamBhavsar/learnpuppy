//! Lesson rounds, derived from the event log (so they survive a restart), per
//! `brain/cheap-tutor/model-routing.md` section 3:
//!
//! - a lesson is done when all its items have an outcome (`First`);
//! - under 50%: its explanation is shown again and the missed items are retried by the runner;
//! - when the missed items have been answered again (`Second`), still under 50% asks for one cheap
//!   re-teach call; after that the digest carries it to the next refill.

use std::collections::{HashMap, HashSet};

use lp_core::course::{rating_from_grade, Course, CourseEvent, Lesson, Score};
use lp_core::fsrs::Rating;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Round {
    First(Score),
    Second(Score),
}

fn under_half(s: Score) -> bool {
    s.correct * 2 < s.total
}

/// `(item, correct)` of an answer-like event.
fn outcome(ev: &CourseEvent) -> Option<(&str, bool)> {
    match ev {
        CourseEvent::Answered { item, correct, .. } => Some((item, *correct)),
        CourseEvent::SelfGraded { item, grade, .. } => {
            Some((item, rating_from_grade(grade)? != Rating::Again))
        }
        _ => None,
    }
}

/// Latest outcome per item among `events`.
fn latest<'a>(events: impl Iterator<Item = &'a CourseEvent>) -> HashMap<&'a str, bool> {
    events.filter_map(outcome).collect()
}

fn score(items: &[&str], last: &HashMap<&str, bool>) -> Score {
    Score {
        correct: items
            .iter()
            .filter(|i| last.get(**i) == Some(&true))
            .count() as u32,
        total: items.len() as u32,
    }
}

/// The round that this event log has just completed for `lesson`, if any. Flagged items do not count.
pub fn lesson_round(lesson: &Lesson, flagged: &[String], events: &[CourseEvent]) -> Option<Round> {
    let items: Vec<&str> = lesson
        .items
        .iter()
        .filter(|i| !flagged.contains(&i.id))
        .map(|i| i.id.as_str())
        .collect();
    if items.is_empty() {
        return None;
    }
    let done_at: Vec<usize> = events
        .iter()
        .enumerate()
        .filter(|(_, e)| matches!(e, CourseEvent::LessonDone { lesson: l, .. } if *l == lesson.id))
        .map(|(i, _)| i)
        .collect();
    let last = latest(events.iter());
    match done_at.as_slice() {
        [] => items
            .iter()
            .all(|i| last.contains_key(i))
            .then(|| Round::First(score(&items, &last))),
        [d1] => {
            let CourseEvent::LessonDone { correct, total, .. } = events[*d1] else {
                return None;
            };
            if !under_half(Score { correct, total }) {
                return None;
            }
            let shown = events
                .iter()
                .enumerate()
                .skip(*d1)
                .find(|(_, e)| matches!(e, CourseEvent::Shown { lesson: l, .. } if *l == lesson.id))
                .map(|(i, _)| i)?;
            let before = latest(events[..*d1].iter());
            let after: HashSet<&str> = events[shown..]
                .iter()
                .filter_map(outcome)
                .map(|(i, _)| i)
                .collect();
            let again = items
                .iter()
                .filter(|i| before.get(**i) != Some(&true))
                .all(|i| after.contains(i));
            again.then(|| Round::Second(score(&items, &last)))
        }
        _ => None,
    }
}

/// A lesson whose explanation should be shown again: it ended a round under 50% and has not been
/// shown since. A lesson being re-taught by the cheap model waits for the new text.
pub fn pending_reexplain(
    course: &Course,
    events: &[CourseEvent],
    reteaching: &HashSet<String>,
) -> Option<String> {
    course.lessons.iter().find_map(|l| {
        if reteaching.contains(&l.id) {
            return None;
        }
        let d = events.iter().rposition(
            |e| matches!(e, CourseEvent::LessonDone { lesson, .. } if *lesson == l.id),
        )?;
        let CourseEvent::LessonDone { correct, total, .. } = events[d] else {
            return None;
        };
        let shown_since = events[d..]
            .iter()
            .any(|e| matches!(e, CourseEvent::Shown { lesson, .. } if *lesson == l.id));
        (under_half(Score { correct, total }) && !shown_since).then(|| l.id.clone())
    })
}
