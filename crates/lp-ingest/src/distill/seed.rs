//! Self-hosting seed: ingest the user's own Rust / system-design repos so a curriculum exists on day one.
use crate::{ingest, IngestReport, Roots};
use lp_core::ports::TrackerStore;
use std::path::Path;

const SEED_REPOS: [&str; 2] = ["code/tracker", "code/bunfiiliate"];

/// `learnpuppy ingest --seed-rust-sysdesign`: ingests every session of `home`'s agents whose
/// cwd is inside the tracker or bunfiiliate repo.
pub fn seed_rust_sysdesign(store: &dyn TrackerStore, home: &Path) -> IngestReport {
    let allow = SEED_REPOS.map(|r| home.join(r));
    ingest(store, &Roots::from_home(home), &allow, 0)
}
