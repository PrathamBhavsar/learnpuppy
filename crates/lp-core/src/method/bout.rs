//! F23: the bout clock — 90 active minutes, a 2-hour gap before the next bout, 270 minutes a
//! day, and one logged override. The limits are workspace-wide, not per course: whoever calls
//! this passes the flat heartbeat stream and the minutes already spent today across every
//! course, so the clock cannot be reset by switching courses.

use super::Settings;

const MS_PER_MINUTE: i64 = 60_000;

/// Minutes of real engagement folded from a heartbeat stream (millisecond timestamps, in
/// order). A gap of `idle_rewind_minutes` or more between two heartbeats is idle: instead of
/// just not counting it, it moves the clock back by `idle_rewind_minutes`, floored at 0 (F23's
/// "idle over 10 minutes moves the clock back 10 minutes"). `idle_rewind_minutes == 0` disables
/// idle detection: every gap counts as active time.
pub fn active_minutes(heartbeats_ms: &[i64], idle_rewind_minutes: u32) -> u32 {
    let mut active: i64 = 0;
    for pair in heartbeats_ms.windows(2) {
        let delta_minutes = (pair[1] - pair[0]).max(0) / MS_PER_MINUTE;
        if idle_rewind_minutes > 0 && delta_minutes >= idle_rewind_minutes as i64 {
            active = (active - idle_rewind_minutes as i64).max(0);
        } else {
            active += delta_minutes;
        }
    }
    active.max(0) as u32
}

/// F23: what the bout clock says right now.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BoutStatus {
    /// Still under every limit; keep teaching.
    Active { active_minutes: u32 },
    /// This bout hit the 90-minute limit: Close, then wait out the gap.
    BoutLimitReached,
    /// Within `gap_between_bouts_minutes` of the last bout ending.
    Gap { remaining_minutes: u32 },
    /// The day's 270 minutes are spent; locked until the next day, override aside.
    DailyLimitReached,
}

/// F23: the bout clock, across every course in the workspace.
///
/// - `heartbeats_ms`: the current (or most recently open) bout's heartbeat stream.
/// - `minutes_today_before_this_bout`: active minutes already spent today in earlier bouts,
///   across every course — this is how the daily limit spans courses.
/// - `bout_ended_at_ms`: when the last bout ended, if teaching is currently between bouts.
/// - `override_used_today`: F23's one logged override a day lifts the gap and daily limit.
pub fn bout_status(
    now_ms: i64,
    heartbeats_ms: &[i64],
    minutes_today_before_this_bout: u32,
    bout_ended_at_ms: Option<i64>,
    override_used_today: bool,
    settings: &Settings,
) -> BoutStatus {
    if !override_used_today {
        if let Some(ended_at) = bout_ended_at_ms {
            let since_minutes = ((now_ms - ended_at).max(0) / MS_PER_MINUTE) as u32;
            if since_minutes < settings.gap_between_bouts_minutes {
                return BoutStatus::Gap {
                    remaining_minutes: settings.gap_between_bouts_minutes - since_minutes,
                };
            }
        }
    }

    let this_bout = active_minutes(heartbeats_ms, settings.idle_rewind_minutes);
    let today_total = minutes_today_before_this_bout + this_bout;
    if !override_used_today && today_total >= settings.daily_limit_minutes {
        return BoutStatus::DailyLimitReached;
    }
    if this_bout >= settings.bout_limit_minutes {
        return BoutStatus::BoutLimitReached;
    }
    BoutStatus::Active {
        active_minutes: this_bout,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn idle_gap_rewinds_rather_than_just_skips() {
        // Two minutes active, then a 10-minute idle gap: rewinds the 2 active minutes to 0,
        // not down to -8.
        let hb = [0, 2 * MS_PER_MINUTE, 12 * MS_PER_MINUTE];
        assert_eq!(active_minutes(&hb, 10), 0);
    }

    #[test]
    fn idle_rewind_floors_at_zero_even_with_repeated_idle_gaps() {
        let hb = [
            0,
            20 * MS_PER_MINUTE,
            40 * MS_PER_MINUTE,
            60 * MS_PER_MINUTE,
        ];
        assert_eq!(active_minutes(&hb, 10), 0);
    }

    #[test]
    fn daily_limit_spans_courses() {
        let settings = Settings::default();
        // One heartbeat a minute for 30 minutes: continuous engagement, never idle.
        let hb: Vec<i64> = (0..=30).map(|m| m * MS_PER_MINUTE).collect();
        // 200 minutes already spent today on another course; this course's bout alone (30
        // minutes) stays under the 90-minute bout limit, but the combined total of 230 is
        // still short of the 270-minute daily limit.
        let status = bout_status(30 * MS_PER_MINUTE, &hb, 200, None, false, &settings);
        assert_eq!(status, BoutStatus::Active { active_minutes: 30 });

        // Pushing the prior-courses total to 240 means this same 30-minute bout now crosses
        // the 270-minute daily total, even though this bout alone never approached either
        // per-bout limit.
        let status = bout_status(30 * MS_PER_MINUTE, &hb, 240, None, false, &settings);
        assert_eq!(status, BoutStatus::DailyLimitReached);
    }

    #[test]
    fn gap_between_bouts_is_enforced() {
        let settings = Settings::default();
        let status = bout_status(0, &[], 0, Some(-60 * MS_PER_MINUTE), false, &settings);
        assert_eq!(
            status,
            BoutStatus::Gap {
                remaining_minutes: 60
            }
        );
    }

    #[test]
    fn override_lifts_the_gap_and_daily_limit() {
        let settings = Settings::default();
        let status = bout_status(0, &[], 300, Some(-60 * MS_PER_MINUTE), true, &settings);
        assert_eq!(status, BoutStatus::Active { active_minutes: 0 });
    }

    proptest! {
        #[test]
        fn active_minutes_never_negative(
            deltas in prop::collection::vec(0i64..200, 0..50),
            idle_rewind in 0u32..30,
        ) {
            let mut ts = vec![0i64];
            for d in &deltas {
                ts.push(ts.last().copied().unwrap() + d * MS_PER_MINUTE);
            }
            // active_minutes is a u32, so "never negative" holds by construction; the
            // property that matters is that it never silently wraps past 0.
            let before = active_minutes(&ts, idle_rewind);
            prop_assert!(before < u32::MAX / 2);
        }

        #[test]
        fn fold_is_deterministic(
            deltas in prop::collection::vec(0i64..200, 0..50),
            idle_rewind in 0u32..30,
        ) {
            let mut ts = vec![0i64];
            for d in &deltas {
                ts.push(ts.last().copied().unwrap() + d * MS_PER_MINUTE);
            }
            prop_assert_eq!(active_minutes(&ts, idle_rewind), active_minutes(&ts, idle_rewind));
        }
    }
}
