//! Workspace-wide learning-session wire types (docs/web/03 §Learning): the bout clock (F23),
//! gap pauses (F24), the sleep check-in (F04), the weekly schedule (F01) and the learner
//! profile (F34). Bouts span every course in the workspace, so every route here takes only
//! `{ws}`, never a course id.

use lp_core::method::bout::BoutStatus;
use lp_core::method::Settings as MethodSettings;
use lp_service::learning::bout::Ready;
use lp_service::learning::schedule::{SlotRecord, SlotState};
use lp_service::learning::Clock as SvcClock;
use lp_store::{ProfileEntry as StoreProfileEntry, Schedule as StoreSchedule};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum BoutStatusDto {
    /// Still under every limit; keep teaching.
    Active {
        #[serde(rename = "activeMinutes")]
        active_minutes: u32,
    },
    /// This bout hit the 90-minute limit: Close, then wait out the gap.
    BoutLimitReached,
    /// Within the gap after the last bout ended.
    Gap {
        #[serde(rename = "remainingMinutes")]
        remaining_minutes: u32,
    },
    /// The day's minutes are spent; locked until the next day, override aside.
    DailyLimitReached,
}

impl From<BoutStatus> for BoutStatusDto {
    fn from(s: BoutStatus) -> Self {
        match s {
            BoutStatus::Active { active_minutes } => Self::Active { active_minutes },
            BoutStatus::BoutLimitReached => Self::BoutLimitReached,
            BoutStatus::Gap { remaining_minutes } => Self::Gap { remaining_minutes },
            BoutStatus::DailyLimitReached => Self::DailyLimitReached,
        }
    }
}

/// F23's clock, plus F24's gap-pause flag and the limits the UI renders against.
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClockDto {
    pub bout: Option<String>,
    pub status: BoutStatusDto,
    pub minutes_today: u32,
    pub override_used_today: bool,
    /// F24: recomputed fresh on every call from the bout's own active-minute count, so it
    /// clears on its own once a heartbeat moves the count past the minute it was due on.
    pub gap_pause_due: bool,
    pub gap_pause_seconds: u32,
    pub bout_limit_minutes: u32,
    pub daily_limit_minutes: u32,
}

/// Builds the wire clock; `gap_pause_due` is computed by the route (it needs the bout id as a
/// seed, which the service's own [`SvcClock`] does not carry).
pub fn clock_dto(clock: &SvcClock, settings: &MethodSettings, gap_pause_due: bool) -> ClockDto {
    ClockDto {
        bout: clock.bout.clone(),
        status: clock.status.into(),
        minutes_today: clock.minutes_today,
        override_used_today: clock.override_used_today,
        gap_pause_due,
        gap_pause_seconds: settings.gap_pause_seconds,
        bout_limit_minutes: settings.bout_limit_minutes,
        daily_limit_minutes: settings.daily_limit_minutes,
    }
}

/// F03's ready-gate checklist; `focus_seconds` is the time the focus timer actually ran.
#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReadyAnswersDto {
    pub phone_away: bool,
    pub tabs_closed: bool,
    pub focus_seconds: u32,
}

impl From<ReadyAnswersDto> for Ready {
    fn from(r: ReadyAnswersDto) -> Self {
        Ready {
            phone_away: r.phone_away,
            tabs_closed: r.tabs_closed,
            focus_seconds: r.focus_seconds,
        }
    }
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct StartBoutRequest {
    pub course: String,
    pub ready: ReadyAnswersDto,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct HeartbeatRequest {
    pub bout: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct OverrideRequest {
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct CheckinNeededDto {
    pub needed: bool,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CheckinRequest {
    pub sleep_hours: f32,
    #[serde(default)]
    pub note: String,
}

/// `review_only_suggested`: self-reported sleep came in under the poor-sleep threshold (F04).
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CheckinResultDto {
    pub review_only_suggested: bool,
}

/// F01: weekly slots and wake time. `.ics` export is deferred past G4.
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScheduleDto {
    pub wake_time: String,
    pub slots: Vec<String>,
}

impl From<StoreSchedule> for ScheduleDto {
    fn from(s: StoreSchedule) -> Self {
        Self {
            wake_time: s.wake_time,
            slots: s.slots,
        }
    }
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SetScheduleRequest {
    pub wake_time: String,
    pub slots: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SuggestSlotsRequest {
    pub wake_time: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SuggestSlotsResultDto {
    pub slots: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SlotStateDto {
    Kept,
    Missed,
    Upcoming,
}

impl From<SlotState> for SlotStateDto {
    fn from(s: SlotState) -> Self {
        match s {
            SlotState::Kept => Self::Kept,
            SlotState::Missed => Self::Missed,
            SlotState::Upcoming => Self::Upcoming,
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct SlotRecordDto {
    pub day: i64,
    pub slot: String,
    pub state: SlotStateDto,
}

impl From<SlotRecord> for SlotRecordDto {
    fn from(r: SlotRecord) -> Self {
        Self {
            day: r.day,
            slot: r.slot,
            state: r.state.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct TrackSlotsResultDto {
    pub slots: Vec<SlotRecordDto>,
}

/// F34: one dated profile entry; the latest per key wins.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ProfileEntryDto {
    pub ts: i64,
    pub key: String,
    pub value: String,
}

impl From<StoreProfileEntry> for ProfileEntryDto {
    fn from(e: StoreProfileEntry) -> Self {
        Self {
            ts: e.ts,
            key: e.key,
            value: e.value,
        }
    }
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct SetProfileRequest {
    pub key: String,
    pub value: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ProfileResultDto {
    pub entries: Vec<ProfileEntryDto>,
}

/// One tunable method setting with its default and allowed range.
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MethodSettingDto {
    pub name: String,
    pub value: u32,
    pub default: u32,
    pub min: u32,
    pub max: u32,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MethodSettingsDto {
    pub settings: Vec<MethodSettingDto>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct SetMethodSettingRequest {
    pub name: String,
    pub value: u32,
}

pub fn method_settings_dto(current: &MethodSettings) -> MethodSettingsDto {
    let defaults = lp_service::learning::setting_values(&MethodSettings::default());
    let values = lp_service::learning::setting_values(current);
    let settings = lp_service::learning::SETTING_BOUNDS
        .iter()
        .zip(values.iter().zip(defaults.iter()))
        .map(
            |(&(name, min, max), (&(_, value), &(_, default)))| MethodSettingDto {
                name: name.to_owned(),
                value,
                default,
                min,
                max,
            },
        )
        .collect();
    MethodSettingsDto { settings }
}
