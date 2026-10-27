//! Transcript and repository ingestion for LearnPuppy.
mod agy;
pub mod distill;
mod git;
mod redact;
mod scan;
mod transcript;

pub use git::{commits_for_session, GitCommit};
pub use scan::{ingest, IngestReport, Roots};

#[cfg(test)]
mod tests;
