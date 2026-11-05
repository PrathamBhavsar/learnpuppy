//! The learning service (M7): workspace-wide bout clock, schedule, sleep check-in and learner
//! profile. Wraps `FsLearningStore`; the rules live in `lp_core::method`. Every clock reads
//! server timestamps from the append-only bout log, so a refresh or restart cannot reset it.
//! Not covered: `.ics` export (F01) and the rest prompt (F33), both deferred past G4.

pub mod bout;
pub mod checkin;
pub mod profile;
pub mod schedule;
#[cfg(test)]
mod tests;

use std::path::Path;
use std::sync::Arc;

use lp_core::method::bout::BoutStatus;
use lp_core::method::Settings as MethodSettings;
use lp_core::ports::StoreError;
use lp_store::FsLearningStore;
use parking_lot::Mutex;

pub const MS_PER_MINUTE: i64 = 60_000;
pub const MS_PER_DAY: i64 = 86_400_000;

/// Why a bout cannot start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// Within the gap after the last bout; `remaining_minutes` left.
    Gap { remaining_minutes: u32 },
    /// The day's minutes are spent.
    DailyLimit,
    /// The ready gate (F03) was not completed.
    NotReady,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LearningError {
    #[error("bout refused: {0:?}")]
    Refused(Refusal),
    #[error("{0}")]
    Invalid(String),
    #[error("not found: {0}")]
    NotFound(String),
    #[error("override already used today")]
    OverrideUsed,
    #[error("storage failure: {0}")]
    Store(String),
}

impl From<StoreError> for LearningError {
    fn from(e: StoreError) -> Self {
        LearningError::Store(e.to_string())
    }
}

pub type Result<T> = std::result::Result<T, LearningError>;

/// What the clock says, for the UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Clock {
    pub bout: Option<String>,
    pub status: BoutStatus,
    pub minutes_today: u32,
    pub override_used_today: bool,
}

#[derive(Clone)]
pub struct LearningService {
    pub(crate) store: Arc<FsLearningStore>,
    lock: Arc<Mutex<()>>,
    /// Local offset from UTC in minutes, used only to decide where a "day" starts.
    tz_offset_minutes: i32,
}

impl LearningService {
    /// `ws_root` is the workspace root (`paths::workspace_dir`).
    pub fn open(ws_root: &Path) -> Self {
        Self {
            store: Arc::new(FsLearningStore::new(ws_root)),
            lock: Arc::new(Mutex::new(())),
            tz_offset_minutes: 0,
        }
    }

    pub fn with_tz_offset(mut self, minutes: i32) -> Self {
        self.tz_offset_minutes = minutes;
        self
    }

    pub(crate) fn day_of(&self, ts: i64) -> i64 {
        (ts + self.tz_offset_minutes as i64 * MS_PER_MINUTE).div_euclid(MS_PER_DAY)
    }

    /// Method settings with the workspace's saved overrides applied by name.
    pub fn settings(&self) -> Result<MethodSettings> {
        let mut s = MethodSettings::default();
        for (k, v) in self.store.load_schedule()?.settings {
            apply_setting(&mut s, &k, v);
        }
        Ok(s)
    }

    /// Saves one setting override; unknown names are rejected.
    pub fn set_setting(&self, name: &str, value: u32) -> Result<()> {
        let _g = self.lock.lock();
        if !apply_setting(&mut MethodSettings::default(), name, value) {
            return Err(LearningError::Invalid(format!("unknown setting {name}")));
        }
        let mut sch = self.store.load_schedule()?;
        sch.settings.insert(name.into(), value);
        self.store.save_schedule(&sch)?;
        Ok(())
    }
}

fn apply_setting(s: &mut MethodSettings, name: &str, v: u32) -> bool {
    let field = match name {
        "bout_limit_minutes" => &mut s.bout_limit_minutes,
        "gap_between_bouts_minutes" => &mut s.gap_between_bouts_minutes,
        "daily_limit_minutes" => &mut s.daily_limit_minutes,
        "idle_rewind_minutes" => &mut s.idle_rewind_minutes,
        "gap_pause_seconds" => &mut s.gap_pause_seconds,
        "gap_pause_every_minutes" => &mut s.gap_pause_every_minutes,
        _ => return false,
    };
    *field = v;
    true
}
