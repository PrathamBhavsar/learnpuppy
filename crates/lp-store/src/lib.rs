//! Persistence adapters for LearnPuppy: JSONL ledgers + atomic JSON files + SQLite WAL/FTS5.
mod chat;
mod db;
mod graph;
mod jobs;
mod jsonl;
mod tracker;

pub use chat::FsChatStore;
pub use graph::FsGraphStore;
pub use jobs::FsJobStore;
pub use tracker::SqliteTrackerStore;

#[cfg(test)]
mod tests;
