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
        let Some(&(_, min, max)) = SETTING_BOUNDS.iter().find(|(n, _, _)| *n == name) else {
            return Err(LearningError::Invalid(format!("unknown setting {name}")));
        };
        if value < min || value > max {
            return Err(LearningError::Invalid(format!(
                "{name} must be {min} to {max}"
            )));
        }
        let mut next = self.settings()?;
        apply_setting(&mut next, name, value);
        if next.error_band_low_pct >= next.error_band_high_pct {
            return Err(LearningError::Invalid(
                "error_band_low_pct must stay below error_band_high_pct".into(),
            ));
        }
        let mut sch = self.store.load_schedule()?;
        sch.settings.insert(name.into(), value);
        self.store.save_schedule(&sch)?;
        Ok(())
    }
}

/// Every tunable method setting: name, minimum, maximum. Zero is allowed only where it means "off".
pub const SETTING_BOUNDS: &[(&str, u32, u32)] = &[
    ("bout_limit_minutes", 5, 180),
    ("gap_between_bouts_minutes", 0, 720),
    ("daily_limit_minutes", 15, 600),
    ("idle_rewind_minutes", 1, 60),
    ("gap_pause_seconds", 0, 60),
    ("gap_pause_every_minutes", 0, 120),
    ("mixed_pass_every", 0, 10),
    ("target_error_rate_pct", 1, 50),
    ("error_band_low_pct", 1, 50),
    ("error_band_high_pct", 2, 90),
    ("attempts_before_parking", 1, 10),
    ("probe_strands", 1, 5),
    ("probe_questions_per_strand", 1, 4),
    ("probe_questions_max", 1, 20),
    ("strong_calls_per_hour", 1, 10),
];

/// The current value of every tunable setting, in `SETTING_BOUNDS` order.
pub fn setting_values(s: &MethodSettings) -> Vec<(&'static str, u32)> {
    SETTING_BOUNDS
        .iter()
        .map(|&(name, _, _)| {
            let mut probe = *s;
            (name, *setting_field(&mut probe, name).expect("listed name"))
        })
        .collect()
}

fn setting_field<'a>(s: &'a mut MethodSettings, name: &str) -> Option<&'a mut u32> {
    Some(match name {
        "bout_limit_minutes" => &mut s.bout_limit_minutes,
        "gap_between_bouts_minutes" => &mut s.gap_between_bouts_minutes,
        "daily_limit_minutes" => &mut s.daily_limit_minutes,
        "idle_rewind_minutes" => &mut s.idle_rewind_minutes,
        "gap_pause_seconds" => &mut s.gap_pause_seconds,
        "gap_pause_every_minutes" => &mut s.gap_pause_every_minutes,
        "mixed_pass_every" => &mut s.mixed_pass_every,
        "target_error_rate_pct" => &mut s.target_error_rate_pct,
        "error_band_low_pct" => &mut s.error_band_low_pct,
        "error_band_high_pct" => &mut s.error_band_high_pct,
        "attempts_before_parking" => &mut s.attempts_before_parking,
        "probe_strands" => &mut s.probe_strands,
        "probe_questions_per_strand" => &mut s.probe_questions_per_strand,
        "probe_questions_max" => &mut s.probe_questions_max,
        "strong_calls_per_hour" => &mut s.strong_calls_per_hour,
        _ => return None,
    })
}

fn apply_setting(s: &mut MethodSettings, name: &str, v: u32) -> bool {
    match setting_field(s, name) {
        Some(field) => {
            *field = v;
            true
        }
        None => false,
    }
}
