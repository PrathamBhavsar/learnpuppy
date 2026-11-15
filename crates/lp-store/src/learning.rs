//! `FsLearningStore`: workspace-wide `<root>/learning/{bouts,checkins,register,profile}.jsonl`
//! and `schedule.json`. Bout limits span courses, so none of this lives under `courses/`.
use crate::jsonl::{append, create_dir, read_all, read_json, write_json};
use lp_core::method::register::RegisterRow;
use lp_core::ports::{StoreError, StoreResult};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum BoutEvent {
    BoutStarted {
        ts: i64,
        bout: String,
        #[serde(default)]
        course: String,
    },
    BoutEnded {
        ts: i64,
        bout: String,
    },
    FocusLost {
        ts: i64,
        bout: String,
    },
    Heartbeat {
        ts: i64,
        bout: String,
    },
    /// F23's one logged override a day.
    Override {
        ts: i64,
        #[serde(default)]
        reason: String,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CheckIn {
    pub ts: i64,
    /// Hours slept, as the learner reports it.
    pub sleep_hours: f32,
    #[serde(default)]
    pub note: String,
}

/// Dated profile entry; the latest entry for a key wins.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileEntry {
    pub ts: i64,
    pub key: String,
    pub value: String,
}

/// A register row as written at grading time, tagged with its course and time.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RegisterRecord {
    pub ts: i64,
    pub course: String,
    #[serde(flatten)]
    pub row: RegisterRow,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Schedule {
    /// Wake time as "HH:MM", empty when unset.
    #[serde(default)]
    pub wake_time: String,
    /// Planned study slots as "HH:MM" start times.
    #[serde(default)]
    pub slots: Vec<String>,
    /// Setting overrides by name (tunables from `method::Settings`).
    #[serde(default)]
    pub settings: std::collections::BTreeMap<String, u32>,
}

pub struct FsLearningStore {
    dir: PathBuf,
}

impl FsLearningStore {
    /// `root` is the workspace root; nothing is created until the first write.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            dir: root.into().join("learning"),
        }
    }

    fn push(&self, file: &str, v: &impl Serialize) -> StoreResult<()> {
        create_dir(&self.dir)?;
        append(&self.dir.join(file), v)
    }

    pub fn append_bout(&self, e: &BoutEvent) -> StoreResult<()> {
        self.push("bouts.jsonl", e)
    }
    pub fn bouts(&self) -> StoreResult<Vec<BoutEvent>> {
        read_all(&self.dir.join("bouts.jsonl"))
    }

    pub fn append_checkin(&self, c: &CheckIn) -> StoreResult<()> {
        self.push("checkins.jsonl", c)
    }
    pub fn checkins(&self) -> StoreResult<Vec<CheckIn>> {
        read_all(&self.dir.join("checkins.jsonl"))
    }

    pub fn append_register(&self, r: &RegisterRecord) -> StoreResult<()> {
        self.push("register.jsonl", r)
    }
    pub fn register(&self) -> StoreResult<Vec<RegisterRecord>> {
        read_all(&self.dir.join("register.jsonl"))
    }

    pub fn append_profile(&self, p: &ProfileEntry) -> StoreResult<()> {
        self.push("profile.jsonl", p)
    }
    /// Latest entry per key, in key order.
    pub fn profile(&self) -> StoreResult<Vec<ProfileEntry>> {
        let mut by_key: std::collections::BTreeMap<String, ProfileEntry> = Default::default();
        for e in read_all::<ProfileEntry>(&self.dir.join("profile.jsonl"))? {
            if by_key.get(&e.key).is_none_or(|old| old.ts <= e.ts) {
                by_key.insert(e.key.clone(), e);
            }
        }
        Ok(by_key.into_values().collect())
    }

    /// Default schedule when none was saved.
    pub fn load_schedule(&self) -> StoreResult<Schedule> {
        match read_json(&self.dir.join("schedule.json"), "schedule") {
            Err(StoreError::NotFound(_)) => Ok(Schedule::default()),
            r => r,
        }
    }
    pub fn save_schedule(&self, s: &Schedule) -> StoreResult<()> {
        create_dir(&self.dir)?;
        write_json(&self.dir.join("schedule.json"), s)
    }
}
