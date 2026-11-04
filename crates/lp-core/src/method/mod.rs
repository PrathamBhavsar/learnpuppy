//! The learning method: pure, synchronous rules on top of the course domain in `course.rs`.
//! No IO, no async, no clock — every function here takes `now` and any randomness as plain
//! arguments from its caller.

pub mod dag;
pub mod mastery;
pub mod probe;
pub mod recall;
pub mod register;

use crate::course::Course;
use mastery::{FactStatus, MasteryState};

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
}

/// F14's mastery-bar block: a lesson cannot be left behind while any of its non-parked facts
/// are below `mc-passed`. `method_version` 0 skips every new gate, including this one, so an
/// old course keeps behaving exactly as it did before this plan.
pub fn check_action(
    course: &Course,
    lesson_id: &str,
    mastery: &MasteryState,
) -> Result<(), GateError> {
    if course.method_version == 0 {
        return Ok(());
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::{Course, Lesson};
    use mastery::fact_key;

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
        assert!(check_action(&c, "l00", &mastery).is_ok());
    }

    #[test]
    fn open_facts_block_advancing_the_lesson() {
        let c = course(1);
        let mastery = MasteryState::default();
        let err = check_action(&c, "l00", &mastery).unwrap_err();
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
        assert!(check_action(&c, "l00", &mastery).is_ok());
    }
}
