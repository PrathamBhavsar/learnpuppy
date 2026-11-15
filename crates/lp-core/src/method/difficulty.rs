//! F26: difficulty control. Items carry a level (`Item.level` in `course.rs`, 1 to 3); this
//! module only decides which way that level should move, from the error rate over the last 20
//! answers. The band is deliberately wide: a move only happens outside
//! `[error_band_low_pct, error_band_high_pct]`, aiming near `target_error_rate_pct`.

use super::Settings;

/// Which way the level should move, if at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DifficultyMove {
    /// Error rate is above the band: step down a level.
    Easier,
    /// Error rate is below the band and a harder level exists: step up.
    Harder,
    /// Error rate is below the band and there is no harder level: offer skip-ahead instead.
    SkipAhead,
    /// Inside the band: no change.
    Hold,
}

/// Error rate, in whole percent, over the last 20 answers (oldest beyond that dropped). An
/// empty window reads as 0%, not an error.
pub fn rolling_error_rate_pct(last_answers_correct: &[bool]) -> u32 {
    let start = last_answers_correct.len().saturating_sub(20);
    let window = &last_answers_correct[start..];
    if window.is_empty() {
        return 0;
    }
    let misses = window.iter().filter(|correct| !**correct).count();
    (misses * 100 / window.len()) as u32
}

/// F26: which way (if any) the level should move, from the rolling error rate.
/// `harder_level_exists` is false once the item is already at the top level, in which case a
/// rate below the band offers `SkipAhead` instead of a `Harder` level that does not exist.
pub fn next_move(
    last_answers_correct: &[bool],
    harder_level_exists: bool,
    settings: &Settings,
) -> DifficultyMove {
    if last_answers_correct.is_empty() {
        return DifficultyMove::Hold;
    }
    let rate = rolling_error_rate_pct(last_answers_correct);
    if rate > settings.error_band_high_pct {
        DifficultyMove::Easier
    } else if rate < settings.error_band_low_pct {
        if harder_level_exists {
            DifficultyMove::Harder
        } else {
            DifficultyMove::SkipAhead
        }
    } else {
        DifficultyMove::Hold
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn empty_window_holds() {
        let settings = Settings::default();
        assert_eq!(next_move(&[], true, &settings), DifficultyMove::Hold);
    }

    #[test]
    fn high_error_rate_moves_easier() {
        let settings = Settings::default();
        let answers = [false; 20];
        assert_eq!(next_move(&answers, true, &settings), DifficultyMove::Easier);
    }

    #[test]
    fn low_error_rate_moves_harder_or_skips_ahead_at_the_top() {
        let settings = Settings::default();
        let answers = [true; 20];
        assert_eq!(next_move(&answers, true, &settings), DifficultyMove::Harder);
        assert_eq!(
            next_move(&answers, false, &settings),
            DifficultyMove::SkipAhead
        );
    }

    #[test]
    fn only_the_last_20_answers_count() {
        // 20 misses in a row, after an arbitrarily long run of hits further back: the rate is
        // 100%, not diluted by history outside the window.
        let mut answers = vec![true; 500];
        answers.extend(std::iter::repeat_n(false, 20));
        assert_eq!(rolling_error_rate_pct(&answers), 100);
    }

    proptest! {
        #[test]
        fn difficulty_changes_only_outside_the_band(
            answers in prop::collection::vec(any::<bool>(), 0..40),
            harder_level_exists in any::<bool>(),
        ) {
            let settings = Settings::default();
            let rate = rolling_error_rate_pct(&answers);
            let mv = next_move(&answers, harder_level_exists, &settings);
            let in_band = answers.is_empty()
                || (rate >= settings.error_band_low_pct && rate <= settings.error_band_high_pct);
            prop_assert_eq!(mv == DifficultyMove::Hold, in_band);
        }
    }
}
