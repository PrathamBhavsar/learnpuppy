//! F23 + F03: start, heartbeat, end and override. All state is folded from `bouts.jsonl`.

use lp_core::method::bout::{active_minutes, bout_status, BoutStatus};
use lp_store::BoutEvent;

use super::{Clock, LearningError, LearningService, Refusal, Result};

/// F03 ready gate answers; the focus timer must run at least [`MIN_FOCUS_SECONDS`].
#[derive(Clone, Copy, Debug)]
pub struct Ready {
    pub phone_away: bool,
    pub tabs_closed: bool,
    pub focus_seconds: u32,
}

pub const MIN_FOCUS_SECONDS: u32 = 30;

#[derive(Default)]
pub(crate) struct Folded {
    /// Open bout id and its heartbeats (start included).
    pub open: Option<(String, Vec<i64>)>,
    pub last_end: Option<i64>,
    pub minutes_today: u32,
    pub override_today: bool,
}

impl LearningService {
    pub(crate) fn fold(&self, now: i64, idle_rewind: u32) -> Result<Folded> {
        let today = self.day_of(now);
        let mut f = Folded::default();
        // bout id -> heartbeats, in start order.
        let mut bouts: Vec<(String, Vec<i64>, bool)> = Vec::new();
        for e in self.store.bouts()? {
            match e {
                BoutEvent::BoutStarted { ts, bout, .. } => bouts.push((bout, vec![ts], false)),
                BoutEvent::Heartbeat { ts, bout } => {
                    if let Some(b) = bouts.iter_mut().rev().find(|b| b.0 == bout && !b.2) {
                        b.1.push(ts);
                    }
                }
                BoutEvent::BoutEnded { ts, bout } => {
                    if let Some(b) = bouts.iter_mut().rev().find(|b| b.0 == bout && !b.2) {
                        b.2 = true;
                        b.1.push(ts);
                        f.last_end = Some(f.last_end.map_or(ts, |p| p.max(ts)));
                    }
                }
                BoutEvent::FocusLost { .. } => {}
                BoutEvent::Override { ts, .. } => {
                    if self.day_of(ts) == today {
                        f.override_today = true;
                    }
                }
            }
        }
        for (id, hbs, ended) in bouts {
            if ended {
                let start_day = self.day_of(hbs[0]);
                if start_day == today {
                    f.minutes_today += active_minutes(&hbs, idle_rewind);
                }
            } else {
                f.open = Some((id, hbs));
            }
        }
        Ok(f)
    }

    /// The clock as of `now`; restored purely from the log, so it survives a restart.
    pub fn clock(&self, now: i64) -> Result<Clock> {
        let s = self.settings()?;
        let f = self.fold(now, s.idle_rewind_minutes)?;
        Ok(self.clock_from(now, &s, &f))
    }

    fn clock_from(&self, now: i64, s: &lp_core::method::Settings, f: &Folded) -> Clock {
        let hbs: &[i64] = f.open.as_ref().map_or(&[], |o| &o.1);
        let ended = if f.open.is_some() { None } else { f.last_end };
        let status = bout_status(now, hbs, f.minutes_today, ended, f.override_today, s);
        Clock {
            bout: f.open.as_ref().map(|o| o.0.clone()),
            status,
            minutes_today: f.minutes_today,
            override_used_today: f.override_today,
        }
    }

    /// Starts a bout, or returns the open one (restoring it after a restart). Refused inside the
    /// gap or when the day is spent, unless today's override was used.
    pub fn start_bout(&self, now: i64, course: &str, ready: Ready) -> Result<String> {
        let _g = self.lock.lock();
        let s = self.settings()?;
        let f = self.fold(now, s.idle_rewind_minutes)?;
        if let Some((id, _)) = &f.open {
            return Ok(id.clone());
        }
        if !ready.phone_away || !ready.tabs_closed || ready.focus_seconds < MIN_FOCUS_SECONDS {
            return Err(LearningError::Refused(Refusal::NotReady));
        }
        match self.clock_from(now, &s, &f).status {
            BoutStatus::Gap { remaining_minutes } => {
                return Err(LearningError::Refused(Refusal::Gap { remaining_minutes }))
            }
            BoutStatus::DailyLimitReached => {
                return Err(LearningError::Refused(Refusal::DailyLimit))
            }
            _ => {}
        }
        let id = format!("b_{now}");
        self.store.append_bout(&BoutEvent::BoutStarted {
            ts: now,
            bout: id.clone(),
            course: course.into(),
        })?;
        Ok(id)
    }

    /// Records a heartbeat for the open bout and returns the clock.
    pub fn heartbeat(&self, now: i64, bout: &str) -> Result<Clock> {
        let _g = self.lock.lock();
        let s = self.settings()?;
        let f = self.fold(now, s.idle_rewind_minutes)?;
        match &f.open {
            Some((id, _)) if id == bout => {}
            _ => return Err(LearningError::NotFound(format!("open bout {bout}"))),
        }
        self.store.append_bout(&BoutEvent::Heartbeat {
            ts: now,
            bout: bout.into(),
        })?;
        let f = self.fold(now, s.idle_rewind_minutes)?;
        Ok(self.clock_from(now, &s, &f))
    }

    /// The tab lost focus; logged, no effect on the clock.
    pub fn focus_lost(&self, now: i64, bout: &str) -> Result<()> {
        let _g = self.lock.lock();
        self.store.append_bout(&BoutEvent::FocusLost {
            ts: now,
            bout: bout.into(),
        })?;
        Ok(())
    }

    /// Ends the open bout (idempotent for an already-ended one is a NotFound).
    pub fn end_bout(&self, now: i64, bout: &str) -> Result<Clock> {
        let _g = self.lock.lock();
        let s = self.settings()?;
        let f = self.fold(now, s.idle_rewind_minutes)?;
        match &f.open {
            Some((id, _)) if id == bout => {}
            _ => return Err(LearningError::NotFound(format!("open bout {bout}"))),
        }
        self.store.append_bout(&BoutEvent::BoutEnded {
            ts: now,
            bout: bout.into(),
        })?;
        let f = self.fold(now, s.idle_rewind_minutes)?;
        Ok(self.clock_from(now, &s, &f))
    }

    /// F23's one logged override a day; lifts the gap and the daily limit until tomorrow.
    pub fn override_limits(&self, now: i64, reason: &str) -> Result<()> {
        let _g = self.lock.lock();
        let s = self.settings()?;
        if self.fold(now, s.idle_rewind_minutes)?.override_today {
            return Err(LearningError::OverrideUsed);
        }
        self.store.append_bout(&BoutEvent::Override {
            ts: now,
            reason: reason.into(),
        })?;
        Ok(())
    }
}
