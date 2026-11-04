//! F06: strand probe. One model call writes 3 strands at 3 levels each; code binary-searches
//! each strand, at most 2 questions, to fix a position among the 3 levels. No teaching and only
//! a one-line verdict.

use serde::{Deserialize, Serialize};

/// A strand's label after probing stops.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StrandLabel {
    Known,
    /// Right while guessing counts as `Edge`, never `Known`.
    Edge,
    Unknown,
    /// The learner flagged a missing prerequisite; code never infers this from right/wrong alone.
    Blocked,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Strand {
    pub name: String,
    pub label: StrandLabel,
}

/// One probe answer: which level was asked (1 to 3), whether it was right, whether the learner
/// flagged it as a guess, and whether they flagged a missing prerequisite.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProbeAnswer {
    pub level: u8,
    pub correct: bool,
    pub guessed: bool,
    pub blocked: bool,
}

/// The next level to ask, or `None` once the strand is settled (at most 2 questions). Starts at
/// level 2, then 3 if right, or 1 if wrong.
pub fn next_probe_level(answers: &[ProbeAnswer]) -> Option<u8> {
    match answers {
        [] => Some(2),
        [a] if a.blocked || (a.correct && a.guessed) => None,
        [a] if a.correct => Some(3),
        [a] if !a.correct => Some(1),
        _ => None,
    }
}

/// Labels a strand from the answers collected so far (at most 2).
pub fn label_strand(answers: &[ProbeAnswer]) -> StrandLabel {
    if answers.iter().any(|a| a.blocked) {
        return StrandLabel::Blocked;
    }
    match answers {
        [] => StrandLabel::Unknown,
        [a] if a.correct && a.guessed => StrandLabel::Edge,
        [l2, l3] if l2.level == 2 && l3.level == 3 => {
            if l3.correct && !l3.guessed {
                StrandLabel::Known
            } else {
                StrandLabel::Edge
            }
        }
        [l2, l1] if l2.level == 2 && l1.level == 1 => {
            if l1.correct {
                StrandLabel::Edge
            } else {
                StrandLabel::Unknown
            }
        }
        _ => StrandLabel::Unknown,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn a(level: u8, correct: bool, guessed: bool) -> ProbeAnswer {
        ProbeAnswer {
            level,
            correct,
            guessed,
            blocked: false,
        }
    }

    #[test]
    fn starts_in_the_middle() {
        assert_eq!(next_probe_level(&[]), Some(2));
        assert_eq!(label_strand(&[]), StrandLabel::Unknown);
    }

    #[test]
    fn right_at_top_is_known() {
        let path = [a(2, true, false), a(3, true, false)];
        assert_eq!(next_probe_level(&path[..1]), Some(3));
        assert_eq!(label_strand(&path), StrandLabel::Known);
    }

    #[test]
    fn wrong_at_top_is_edge() {
        let path = [a(2, true, false), a(3, false, false)];
        assert_eq!(label_strand(&path), StrandLabel::Edge);
    }

    #[test]
    fn right_while_guessing_is_edge_not_known() {
        let path = [a(2, true, true)];
        assert_eq!(next_probe_level(&path), None);
        assert_eq!(label_strand(&path), StrandLabel::Edge);
    }

    #[test]
    fn right_at_floor_after_middle_miss_is_edge() {
        let path = [a(2, false, false), a(1, true, false)];
        assert_eq!(next_probe_level(&path[..1]), Some(1));
        assert_eq!(label_strand(&path), StrandLabel::Edge);
    }

    #[test]
    fn wrong_at_floor_is_unknown() {
        let path = [a(2, false, false), a(1, false, false)];
        assert_eq!(next_probe_level(&path), None);
        assert_eq!(label_strand(&path), StrandLabel::Unknown);
    }

    #[test]
    fn blocked_flag_wins_and_stops_probing() {
        let path = [ProbeAnswer {
            level: 2,
            correct: false,
            guessed: false,
            blocked: true,
        }];
        assert_eq!(next_probe_level(&path), None);
        assert_eq!(label_strand(&path), StrandLabel::Blocked);
    }

    #[test]
    fn at_most_two_questions() {
        for first in [a(2, true, false), a(2, false, false)] {
            let second_level = next_probe_level(&[first]).unwrap();
            let second = a(second_level, true, false);
            assert_eq!(next_probe_level(&[first, second]), None);
        }
    }
}
