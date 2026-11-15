//! Persistence adapters for LearnPuppy: JSONL ledgers + atomic JSON files + SQLite WAL/FTS5.
mod chat;
mod courses;
mod db;
mod graph;
mod jobs;
mod jsonl;
mod learning;
mod textbooks;
mod tracker;

pub use chat::FsChatStore;
pub use courses::{CourseSummary, FsCourseStore};
pub use graph::FsGraphStore;
pub use jobs::FsJobStore;
pub use learning::{BoutEvent, CheckIn, FsLearningStore, ProfileEntry, RegisterRecord, Schedule};
pub use textbooks::FsTextbookStore;
pub use tracker::SqliteTrackerStore;

#[cfg(test)]
mod tests;
