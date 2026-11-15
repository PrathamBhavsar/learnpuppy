//! F02 (your why) and F34 (learner profile): dated entries that supersede rather than overwrite.

use lp_store::ProfileEntry;

use super::{LearningError, LearningService, Result};

const MAX_VALUE_CHARS: usize = 1000;
/// Cap on the packet handed to a tutor prompt.
pub const PACKET_CHARS: usize = 1500;

fn why_key(course: &str) -> String {
    format!("why:{course}")
}

impl LearningService {
    /// F02: required one-liner when creating a course.
    pub fn set_why(&self, now: i64, course: &str, why: &str) -> Result<()> {
        if why.trim().is_empty() {
            return Err(LearningError::Invalid("a course needs a why".into()));
        }
        self.set_profile(now, &why_key(course), why)
    }

    pub fn why(&self, course: &str) -> Result<Option<String>> {
        let k = why_key(course);
        Ok(self
            .store
            .profile()?
            .into_iter()
            .find(|e| e.key == k)
            .map(|e| e.value))
    }

    /// Adds a dated entry; the latest per key wins and older ones stay in the log.
    pub fn set_profile(&self, now: i64, key: &str, value: &str) -> Result<()> {
        if key.trim().is_empty() || value.trim().is_empty() {
            return Err(LearningError::Invalid(
                "profile key and value are required".into(),
            ));
        }
        if value.chars().count() > MAX_VALUE_CHARS {
            return Err(LearningError::Invalid("profile value too long".into()));
        }
        self.store.append_profile(&ProfileEntry {
            ts: now,
            key: key.trim().into(),
            value: value.trim().into(),
        })?;
        Ok(())
    }

    /// Current entries (latest per key), excluding per-course whys.
    pub fn profile(&self) -> Result<Vec<ProfileEntry>> {
        Ok(self
            .store
            .profile()?
            .into_iter()
            .filter(|e| !e.key.starts_with("why:"))
            .collect())
    }

    /// Capped text for a prompt: the current entries, newest first, cut at [`PACKET_CHARS`].
    pub fn profile_packet(&self) -> Result<String> {
        let mut es = self.profile()?;
        es.sort_by_key(|e| std::cmp::Reverse(e.ts));
        let mut out = String::new();
        for e in es {
            let line = format!("- {}: {}\n", e.key, e.value);
            if out.chars().count() + line.chars().count() > PACKET_CHARS {
                break;
            }
            out.push_str(&line);
        }
        Ok(out)
    }
}
