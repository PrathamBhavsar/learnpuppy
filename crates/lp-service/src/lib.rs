//! Application logic behind the LearnPuppy web server: settings, engine routing, the classroom
//! turn loop, FSRS review, digest, graph edits, accounts, ingest and
//! the multi-workspace registry. No HTTP and no UI types;
//! `lp-server` maps these plain structs to DTOs.

pub mod accounts;
pub mod chat;
pub mod digest;
pub mod engine;
pub mod events;
pub mod graph;
pub mod ingest;
pub mod paths;
pub mod review;
pub mod settings;
pub mod stores;
pub mod workspace;

/// Server-authoritative wall clock in epoch milliseconds.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
