//! The turn loop: which role serves the next turn, and running turns until the learner is up.

use std::path::Path;
use std::sync::Arc;

use lp_agent::loops::{run_turn, Kind, Session, TurnError};
use lp_cli_driver::engine::{LlmEngine, StreamDelta};
use lp_core::plan::Phase;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Model slot that serves the next turn of `s`.
pub fn role(s: &Session) -> &'static str {
    match (s.kind, s.phase()) {
        (Kind::Practice, _) => "practice",
        (Kind::Teach, Phase::Probe) => "prober",
        (Kind::Teach, _) => "teacher",
    }
}

/// Runs turns until the session waits for the learner, delivering queued steers in between.
pub async fn run_to_rest<E: LlmEngine>(
    engine_for: &dyn Fn(&str) -> Arc<E>,
    root: &Path,
    s: &mut Session,
    events: &mpsc::Sender<StreamDelta>,
    cancel: &CancellationToken,
) -> Result<(), TurnError> {
    loop {
        let engine = engine_for(role(s));
        let phase = s.phase();
        run_turn(engine.as_ref(), root, s, events, cancel).await?;
        if !s.wake() {
            return Ok(());
        }
        if s.phase() == phase {
            tracing::info!(
                ?phase,
                reason = "queued steer delivered",
                "learner message triggered another model turn in the same phase"
            );
        }
    }
}
