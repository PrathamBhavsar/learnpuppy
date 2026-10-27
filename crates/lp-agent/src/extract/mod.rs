//! 3-stage knowledge-graph extraction: Extractor -> Auditor -> Merger.
//!
//! Sources extract serially; the audit+merge section is a global FIFO mutex so every
//! auditor sees the graph as left by the previous merge.

pub mod audit;

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use chrono::Utc;
use lp_cli_driver::engine::{DriverError, LlmEngine, StructuredRequest};
use lp_core::graph::{
    merge_findings, AuditVerdicts, CoverageEntry, ExtractedChain, ExtractedProposition, GraphNode,
    GraphSnapshot, SessionFindings,
};
use lp_core::ports::{GraphStore, SessionCheckpoint, StoreError};
use schemars::JsonSchema;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use smol_str::SmolStr;
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

#[derive(Debug, thiserror::Error)]
pub enum ExtractError {
    #[error(transparent)]
    Driver(#[from] DriverError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

/// One teacher passage (or answered check pair) eligible for extraction.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceMessage {
    pub id: SmolStr,
    pub text: String,
}

impl SourceMessage {
    /// `sha256` of the message view lines, the `coverage.jsonl` key.
    fn fingerprint(&self) -> String {
        hex::encode(Sha256::digest(format!("{}\n{}", self.id, self.text)))
    }
}

/// Extractor `submit` payload.
#[derive(Debug, Deserialize, JsonSchema)]
struct Submission {
    propositions: Vec<ExtractedProposition>,
    chains: Vec<ExtractedChain>,
}

pub struct Pipeline<E, G> {
    engine: Arc<E>,
    store: Arc<G>,
    sources: parking_lot::Mutex<HashMap<SmolStr, Arc<Mutex<()>>>>,
    audit_merge: Mutex<()>,
}

/// ~20k tokens of source text per extractor call.
const BATCH_CHARS: usize = 80_000;

impl<E: LlmEngine, G: GraphStore> Pipeline<E, G> {
    pub fn new(engine: Arc<E>, store: Arc<G>) -> Self {
        Self {
            engine,
            store,
            sources: Default::default(),
            audit_merge: Mutex::new(()),
        }
    }

    /// Extract every not-yet-covered message of `source`; returns how many were folded.
    pub async fn extract(
        &self,
        source: &str,
        messages: &[SourceMessage],
        cancel: CancellationToken,
    ) -> Result<usize, ExtractError> {
        let lane = self
            .sources
            .lock()
            .entry(SmolStr::new(source))
            .or_default()
            .clone();
        let _serial = lane.lock().await;

        let covered: HashSet<String> = self
            .store
            .coverage()?
            .into_iter()
            .filter(|c| c.chat_id == source)
            .map(|c| c.fp)
            .collect();
        let fresh: Vec<&SourceMessage> = messages
            .iter()
            .filter(|m| !covered.contains(&m.fingerprint()))
            .collect();
        if fresh.is_empty() {
            return Ok(0);
        }

        for batch in batches(&fresh) {
            let digest = graph_so_far(&self.store.nodes()?, source);
            let sub = self
                .engine
                .complete_structured::<Submission>(
                    StructuredRequest {
                        prompt: extractor_prompt(&digest, batch),
                        model: None,
                    },
                    cancel.clone(),
                )
                .await?
                .value;
            let findings = scope_local_ids(
                SessionFindings {
                    chat_id: source.into(),
                    message_ids: batch.iter().map(|m| m.id.to_string()).collect(),
                    propositions: sub.propositions,
                    chains: sub.chains,
                },
                &batch[0].id,
            );

            let _turn = self.audit_merge.lock().await;
            let verdicts = self.audit(&findings, cancel.clone()).await?;
            let now = Utc::now();
            let snap = GraphSnapshot::from_ledgers(&self.store.nodes()?, &self.store.chains()?);
            let out = merge_findings(&snap, &findings, &verdicts, now);
            for n in &out.node_lines {
                self.store.append_node(n)?;
            }
            if out.rewrite {
                self.store.rewrite_chains(&out.chains)?;
            } else {
                for c in &out.chain_lines {
                    self.store.append_chain(c)?;
                }
            }
            // Coverage last: a failed write above leaves the messages eligible for retry.
            for m in batch {
                self.store.append_coverage(&CoverageEntry {
                    chat_id: source.into(),
                    fp: m.fingerprint(),
                    tokens: (m.text.len() / 4) as u32,
                    folded_at: now,
                })?;
            }
        }
        Ok(fresh.len())
    }

    /// One-shot notes import (Markdown or already-extracted PDF text): paragraphs become
    /// messages of source `src_<hash>`; a checkpoint makes re-import a no-op.
    pub async fn import_notes(
        &self,
        text: &str,
        cancel: CancellationToken,
    ) -> Result<SmolStr, ExtractError> {
        let hash = hex::encode(Sha256::digest(text));
        let source = SmolStr::new(format!("src_{}", &hash[..12]));
        if self
            .store
            .checkpoints()?
            .iter()
            .any(|c| c.chat_id == source)
        {
            return Ok(source);
        }
        let messages: Vec<SourceMessage> = text
            .split("\n\n")
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .enumerate()
            .map(|(i, p)| SourceMessage {
                id: format!("p{i}").into(),
                text: p.into(),
            })
            .collect();
        self.extract(&source, &messages, cancel).await?;
        self.store.append_checkpoint(&SessionCheckpoint {
            chat_id: source.clone(),
            folded_at: Utc::now().timestamp(),
        })?;
        Ok(source)
    }

    async fn audit(
        &self,
        findings: &SessionFindings,
        cancel: CancellationToken,
    ) -> Result<AuditVerdicts, ExtractError> {
        if findings.propositions.is_empty() {
            return Ok(AuditVerdicts::default());
        }
        let prompt = audit::prompt(self.store.as_ref(), findings)?;
        let out = self
            .engine
            .complete_structured::<AuditVerdicts>(
                StructuredRequest {
                    prompt,
                    model: None,
                },
                cancel,
            )
            .await?;
        Ok(out.value)
    }
}

/// `merge_findings` mints node ids from `chat_id/local_id`, so local ids reused by a later
/// batch of the same source (`p1` again) would collide. Prefix them with the batch's first message.
fn scope_local_ids(mut f: SessionFindings, batch: &str) -> SessionFindings {
    // The proposition text is hashed in, so a retry that reuses `p1` for different text mints a new id.
    let minted: HashMap<String, String> = f
        .propositions
        .iter()
        .map(|p| {
            let h = hex::encode(Sha256::digest(p.statement.as_bytes()));
            (p.id.clone(), format!("{batch}:{}", &h[..8]))
        })
        .collect();
    let scope = |id: &mut String| {
        if let Some(new) = minted.get(id.as_str()) {
            *id = new.clone();
        }
    };
    f.propositions.iter_mut().for_each(|p| scope(&mut p.id));
    for c in &mut f.chains {
        c.premises.iter_mut().for_each(&scope);
        scope(&mut c.conclusion);
    }
    f
}

/// "THE GRAPH SO FAR": propositions already extracted from this same source.
fn graph_so_far(nodes: &[GraphNode], source: &str) -> String {
    nodes
        .iter()
        .filter(|n| n.provenance.iter().any(|p| p.chat_id == source))
        .map(|n| format!("{}: {} - {}", n.id, n.name, n.statement))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Greedy runs of messages whose text stays under `BATCH_CHARS` (a lone oversized message is its own batch).
fn batches<'a>(fresh: &'a [&'a SourceMessage]) -> Vec<&'a [&'a SourceMessage]> {
    let (mut out, mut start, mut size) = (Vec::new(), 0, 0);
    for (i, m) in fresh.iter().enumerate() {
        if i > start && size + m.text.len() > BATCH_CHARS {
            out.push(&fresh[start..i]);
            (start, size) = (i, 0);
        }
        size += m.text.len();
    }
    out.push(&fresh[start..]);
    out
}

fn extractor_prompt(digest: &str, fresh: &[&SourceMessage]) -> String {
    let mut p = String::from(
        "Extract atomic propositions and the chains (premises => conclusion) the passages argue. \
         Reference existing node ids from THE GRAPH SO FAR directly in chains.\n\nTHE GRAPH SO FAR\n",
    );
    p.push_str(digest);
    for m in fresh {
        p.push_str(&format!("\n\nTEACHER [{}]\n{}", m.id, m.text));
    }
    p
}

#[cfg(test)]
mod tests;
