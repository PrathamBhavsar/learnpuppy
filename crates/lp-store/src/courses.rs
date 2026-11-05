//! `FsCourseStore`: `<root>/courses/<id>/{course.json,events.jsonl}` plus `<root>/learner.json`.
use crate::jsonl::{append, be, create_dir, read_all, read_json, write_json};
use lp_core::course::{Course, CourseEvent};
use lp_core::ports::{StoreError, StoreResult};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CourseSummary {
    pub id: String,
    pub title: String,
    pub version: u32,
    /// `course.json` mtime, ms since the epoch.
    pub updated_at: i64,
}

pub struct FsCourseStore {
    root: PathBuf,
}

/// Only `[a-z0-9_-]`, so an id can never name anything outside `courses/`.
fn check_course_id(id: &str) -> StoreResult<()> {
    let ok = !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(StoreError::Backend(format!("invalid course id: {id:?}")))
    }
}

fn title_of(learner: &str, id: &str) -> String {
    learner
        .lines()
        .map(str::trim)
        .find(|l| !l.is_empty())
        .unwrap_or(id)
        .chars()
        .take(120)
        .collect()
}

fn mtime_ms(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map_or(0, |d| d.as_millis() as i64)
}

impl FsCourseStore {
    /// `root` is the workspace root; nothing is created until the first write.
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn dir(&self, id: &str) -> StoreResult<PathBuf> {
        check_course_id(id)?;
        Ok(self.root.join("courses").join(id))
    }

    pub fn save_course(&self, course: &Course) -> StoreResult<()> {
        let dir = self.dir(&course.id)?;
        create_dir(&dir)?;
        write_json(&dir.join("course.json"), course)
    }

    pub fn load_course(&self, id: &str) -> StoreResult<Course> {
        read_json(&self.dir(id)?.join("course.json"), id)
    }

    /// Readable courses, newest first. Unreadable course dirs are skipped.
    pub fn list(&self) -> StoreResult<Vec<CourseSummary>> {
        let dir = self.root.join("courses");
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(vec![]),
            Err(e) => return Err(be(e)),
        };
        let mut out = vec![];
        for entry in entries.flatten() {
            let Some(id) = entry.file_name().to_str().map(String::from) else {
                continue;
            };
            let Ok(course) = self.load_course(&id) else {
                continue;
            };
            out.push(CourseSummary {
                title: title_of(&course.learner, &id),
                version: course.version,
                updated_at: mtime_ms(&entry.path().join("course.json")),
                id,
            });
        }
        out.sort_by(|a, b| {
            b.updated_at
                .cmp(&a.updated_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(out)
    }

    pub fn append_event(&self, id: &str, event: &CourseEvent) -> StoreResult<()> {
        let dir = self.dir(id)?;
        create_dir(&dir)?;
        append(&dir.join("events.jsonl"), event)
    }

    pub fn events(&self, id: &str) -> StoreResult<Vec<CourseEvent>> {
        read_all(&self.dir(id)?.join("events.jsonl"))
    }

    /// Append markdown to `sessions/<date>-<n>.md`; `date` and `n` are validated so the path
    /// stays inside the course dir.
    pub fn append_session_log(&self, id: &str, date: &str, n: u32, md: &str) -> StoreResult<()> {
        let ok = !date.is_empty() && date.bytes().all(|b| b.is_ascii_digit() || b == b'-');
        if !ok {
            return Err(StoreError::Backend(format!("invalid date: {date:?}")));
        }
        let dir = self.dir(id)?.join("sessions");
        create_dir(&dir)?;
        let mut f = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(dir.join(format!("{date}-{n}.md")))
            .map_err(be)?;
        std::io::Write::write_all(&mut f, md.as_bytes()).map_err(be)
    }

    pub fn read_session_log(&self, id: &str, date: &str, n: u32) -> StoreResult<String> {
        let ok = !date.is_empty() && date.bytes().all(|b| b.is_ascii_digit() || b == b'-');
        if !ok {
            return Err(StoreError::Backend(format!("invalid date: {date:?}")));
        }
        match std::fs::read_to_string(
            self.dir(id)?
                .join("sessions")
                .join(format!("{date}-{n}.md")),
        ) {
            Ok(s) => Ok(s),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(String::new()),
            Err(e) => Err(be(e)),
        }
    }

    pub fn delete_course(&self, id: &str) -> StoreResult<()> {
        match std::fs::remove_dir_all(self.dir(id)?) {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(be(e)),
        }
    }

    /// The workspace-level learner profile; empty when none was saved.
    pub fn load_learner(&self) -> StoreResult<String> {
        match read_json(&self.root.join("learner.json"), "learner") {
            Err(StoreError::NotFound(_)) => Ok(String::new()),
            r => r,
        }
    }

    pub fn save_learner(&self, learner: &str) -> StoreResult<()> {
        create_dir(&self.root)?;
        write_json(&self.root.join("learner.json"), &learner)
    }
}
