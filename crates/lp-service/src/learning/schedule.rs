//! F01: weekly slots, kept and missed tracking. `.ics` export is deferred past G4.

use lp_store::{BoutEvent, Schedule};

use super::{LearningError, LearningService, Result, MS_PER_DAY, MS_PER_MINUTE};

/// A bout starting within this long after a slot's start keeps the slot.
pub const SLOT_WINDOW_MINUTES: i64 = 60;
/// First slot is about this long after waking.
pub const FIRST_SLOT_AFTER_WAKE_MINUTES: u32 = 180;
/// Bout (90) plus gap (120).
const SLOT_STEP_MINUTES: u32 = 210;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SlotState {
    Kept,
    Missed,
    Upcoming,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotRecord {
    /// Local day number (days since epoch).
    pub day: i64,
    pub slot: String,
    pub state: SlotState,
}

fn parse_hm(s: &str) -> Option<u32> {
    let (h, m) = s.split_once(':')?;
    let (h, m): (u32, u32) = (h.parse().ok()?, m.parse().ok()?);
    (h < 24 && m < 60 && s.len() == 5).then_some(h * 60 + m)
}

fn fmt_hm(min: u32) -> String {
    format!("{:02}:{:02}", min / 60 % 24, min % 60)
}

impl LearningService {
    pub fn schedule(&self) -> Result<Schedule> {
        Ok(self.store.load_schedule()?)
    }

    /// Saves wake time and slots ("HH:MM"), keeping saved settings.
    pub fn set_schedule(&self, wake_time: &str, slots: Vec<String>) -> Result<Schedule> {
        if !wake_time.is_empty() && parse_hm(wake_time).is_none() {
            return Err(LearningError::Invalid("wake time must be HH:MM".into()));
        }
        if slots.iter().any(|s| parse_hm(s).is_none()) {
            return Err(LearningError::Invalid("slots must be HH:MM".into()));
        }
        let mut slots = slots;
        slots.sort();
        slots.dedup();
        let mut sch = self.store.load_schedule()?;
        sch.wake_time = wake_time.into();
        sch.slots = slots;
        self.store.save_schedule(&sch)?;
        Ok(sch)
    }

    /// Three suggested slots: the first about 3 hours after waking, then one bout plus gap apart.
    pub fn suggest_slots(&self, wake_time: &str) -> Result<Vec<String>> {
        let wake = parse_hm(wake_time)
            .ok_or_else(|| LearningError::Invalid("wake time must be HH:MM".into()))?;
        let first = wake + FIRST_SLOT_AFTER_WAKE_MINUTES;
        Ok((0..3)
            .map(|i| fmt_hm(first + i * SLOT_STEP_MINUTES))
            .collect())
    }

    /// Kept, missed or upcoming for each slot over the last `days` local days up to `now`.
    pub fn track_slots(&self, now: i64, days: u32) -> Result<Vec<SlotRecord>> {
        let sch = self.store.load_schedule()?;
        let starts: Vec<i64> = self
            .store
            .bouts()?
            .iter()
            .filter_map(|e| match e {
                BoutEvent::BoutStarted { ts, .. } => Some(*ts),
                _ => None,
            })
            .collect();
        let off = self.tz_offset_minutes as i64 * MS_PER_MINUTE;
        let today = self.day_of(now);
        let mut out = Vec::new();
        for day in (today - days as i64 + 1)..=today {
            for slot in &sch.slots {
                let Some(min) = parse_hm(slot) else { continue };
                let begin = day * MS_PER_DAY + min as i64 * MS_PER_MINUTE - off;
                let end = begin + SLOT_WINDOW_MINUTES * MS_PER_MINUTE;
                let state = if starts.iter().any(|t| (begin..end).contains(t)) {
                    SlotState::Kept
                } else if now >= end {
                    SlotState::Missed
                } else {
                    SlotState::Upcoming
                };
                out.push(SlotRecord {
                    day,
                    slot: slot.clone(),
                    state,
                });
            }
        }
        Ok(out)
    }
}
