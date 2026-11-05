//! F04: the first bout of the day asks how you slept. Self-reported only.

use lp_store::CheckIn;

use super::{LearningError, LearningService, Result, MS_PER_DAY};
use lp_store::BoutEvent;

/// Below this many hours the day is suggested as review-only.
pub const POOR_SLEEP_HOURS: f32 = 6.0;
/// A night counts toward the target at this many hours or more.
pub const TARGET_SLEEP_HOURS: f32 = 7.0;
/// The share of nights (percent) that should reach the target.
pub const TARGET_SHARE_PCT: u32 = 80;
const WINDOW_DAYS: i64 = 14;

#[derive(Clone, Debug, PartialEq)]
pub struct SleepSummary {
    pub checkins: usize,
    pub nights_on_target: usize,
    /// Percent of check-ins in the last 14 days at the target, or `None` with no data.
    pub on_target_pct: Option<u32>,
    pub meets_80_pct_target: bool,
}

impl LearningService {
    /// True when no check-in exists today and no bout has started today.
    pub fn needs_checkin(&self, now: i64) -> Result<bool> {
        let today = self.day_of(now);
        let done = self
            .store
            .checkins()?
            .iter()
            .any(|c| self.day_of(c.ts) == today);
        let started =
            self.store.bouts()?.iter().any(
                |e| matches!(e, BoutEvent::BoutStarted { ts, .. } if self.day_of(*ts) == today),
            );
        Ok(!done && !started)
    }

    /// Records a check-in and returns whether a review-only day is suggested.
    pub fn record_checkin(&self, now: i64, sleep_hours: f32, note: &str) -> Result<bool> {
        if !(0.0..=24.0).contains(&sleep_hours) || sleep_hours.is_nan() {
            return Err(LearningError::Invalid("sleep hours must be 0 to 24".into()));
        }
        self.store.append_checkin(&CheckIn {
            ts: now,
            sleep_hours,
            note: note.into(),
        })?;
        Ok(sleep_hours < POOR_SLEEP_HOURS)
    }

    pub fn sleep_summary(&self, now: i64) -> Result<SleepSummary> {
        let from = now - WINDOW_DAYS * MS_PER_DAY;
        let recent: Vec<CheckIn> = self
            .store
            .checkins()?
            .into_iter()
            .filter(|c| c.ts >= from)
            .collect();
        let on = recent
            .iter()
            .filter(|c| c.sleep_hours >= TARGET_SLEEP_HOURS)
            .count();
        let pct = (!recent.is_empty()).then(|| (on * 100 / recent.len()) as u32);
        Ok(SleepSummary {
            checkins: recent.len(),
            nights_on_target: on,
            on_target_pct: pct,
            meets_80_pct_target: pct.is_some_and(|p| p >= TARGET_SHARE_PCT),
        })
    }
}
