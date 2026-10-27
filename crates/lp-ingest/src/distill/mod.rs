//! Decision distillation, self-contained MC cards and the self-hosting seed.
mod cards;
mod seed;

pub use cards::{generate_book_cards, generate_decision_cards, ingest_book, CardReport};
pub use seed::seed_rust_sysdesign;

use crate::git::GitCommit;
use crate::transcript::clip;
use lp_agent::extract::{ExtractError, Pipeline, SourceMessage};
use lp_cli_driver::engine::{DriverError, LlmEngine, StructuredRequest};
use lp_core::ports::{
    Alternative, CodingSession, DecisionBasis, DecisionRecord, GraphStore, StoreError, TrackerStore,
};
use schemars::JsonSchema;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

#[derive(Debug, thiserror::Error)]
pub enum DistillError {
    #[error(transparent)]
    Driver(#[from] DriverError),
    #[error(transparent)]
    Store(#[from] StoreError),
    #[error(transparent)]
    Extract(#[from] ExtractError),
}

const TRANSCRIPT_CAP: usize = 12_000;

#[derive(Deserialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
enum Basis {
    Stated,
    Inferred,
}

#[derive(Deserialize, JsonSchema)]
struct Rejected {
    option: String,
    rejected_because: String,
}

#[derive(Deserialize, JsonSchema)]
struct DecisionDraft {
    title: String,
    what: String,
    why: String,
    basis: Basis,
    alternatives: Vec<Rejected>,
    risk: String,
    files: Vec<String>,
    commit_sha: Option<String>,
}

#[derive(Deserialize, JsonSchema)]
struct Distilled {
    decisions: Vec<DecisionDraft>,
}

/// Stable short hash used for decision and card ids, so re-runs upsert instead of duplicating.
pub(crate) fn short_hash(parts: &[&str]) -> String {
    let mut h = Sha256::new();
    for p in parts {
        h.update(p.as_bytes());
        h.update([0]);
    }
    hex::encode(&h.finalize()[..6])
}

fn prompt(session: &CodingSession, commits: &[GitCommit]) -> String {
    let mut p = String::from(
        "Distill the design decisions made in this coding session. Each needs a concrete title, \
         `what` (1-2 sentences naming real files/types/functions), `why` (the tradeoff), \
         basis `stated` if the transcript says so or `inferred` if only the diff shows it, \
         rejected alternatives, a risk, touched files, and the commit sha if one matches.\n\nSESSION\n",
    );
    p.push_str(&clip(&session.transcript, TRANSCRIPT_CAP));
    for c in commits {
        p.push_str(&format!(
            "\n\nCOMMIT {}\n{}\n{}\nfiles: {}\n{}",
            c.sha,
            c.subject,
            c.body,
            c.files.join(", "),
            c.diff
        ));
    }
    p
}

/// Distills one session (plus its matched commits) into stored `DecisionRecord`s.
/// A `commit_sha` the model invented, i.e. not among `commits`, is dropped.
pub async fn distill_session<E: LlmEngine>(
    engine: &E,
    store: &dyn TrackerStore,
    session: &CodingSession,
    commits: &[GitCommit],
    cancel: CancellationToken,
) -> Result<Vec<DecisionRecord>, DistillError> {
    let out = engine
        .complete_structured::<Distilled>(
            StructuredRequest {
                prompt: prompt(session, commits),
                model: None,
            },
            cancel,
        )
        .await?
        .value;
    let mut records = vec![];
    for d in out.decisions {
        let rec = DecisionRecord {
            id: format!("d_{}", short_hash(&[&session.id, &d.title])).into(),
            session_id: session.id.clone(),
            title: d.title,
            what: d.what,
            why: d.why,
            basis: match d.basis {
                Basis::Stated => DecisionBasis::Stated,
                Basis::Inferred => DecisionBasis::Inferred,
            },
            alternatives: d
                .alternatives
                .into_iter()
                .map(|a| Alternative {
                    option: a.option,
                    rejected_because: a.rejected_because,
                })
                .collect(),
            risk: d.risk,
            files: d.files,
            commit_sha: d.commit_sha.filter(|s| commits.iter().any(|c| &c.sha == s)),
        };
        store.upsert_decision(&rec)?;
        records.push(rec);
    }
    Ok(records)
}

/// Feeds each decision to the graph extractor as a passage, so coding work becomes
/// System Design / Rust propositions and trade-off chains. Returns passages folded.
pub async fn emit_to_graph<E: LlmEngine, G: GraphStore>(
    pipeline: &Pipeline<E, G>,
    source: &str,
    decisions: &[DecisionRecord],
    cancel: CancellationToken,
) -> Result<usize, DistillError> {
    let messages: Vec<SourceMessage> = decisions
        .iter()
        .map(|d| {
            let rejected: String = d
                .alternatives
                .iter()
                .map(|a| format!(" Rejected {}: {}.", a.option, a.rejected_because))
                .collect();
            SourceMessage {
                id: d.id.clone(),
                text: format!("{}. {} Why: {}{rejected}", d.title, d.what, d.why),
            }
        })
        .collect();
    Ok(pipeline.extract(source, &messages, cancel).await?)
}

#[cfg(test)]
mod tests;
