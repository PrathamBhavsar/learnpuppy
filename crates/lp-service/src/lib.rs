//! Application logic behind the LearnPuppy web server: settings, engine routing, the classroom
//! turn loop, FSRS review, digest, graph edits, accounts, ingest and
//! the multi-workspace registry. No HTTP and no UI types;
//! `lp-server` maps these plain structs to DTOs.

pub mod accounts;
pub mod chat;
pub mod course;
pub mod digest;
pub mod engine;
pub mod events;
pub mod export_jobs;
pub mod graph;
pub mod graph_mut;
pub mod ingest;
pub mod ingest_svc;
pub mod paths;
pub mod review;
pub mod settings;
pub mod stores;
pub mod workspace;

/// Server-authoritative wall clock in epoch milliseconds.
pub fn now_ms() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
