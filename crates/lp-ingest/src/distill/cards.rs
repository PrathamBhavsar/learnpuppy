//! 4-option MC cards from decisions and book chunks, behind the `validate_card_shape` gate.
use super::{short_hash, DistillError};
use lp_cli_driver::engine::{LlmEngine, StructuredRequest};
use lp_core::fsrs::{validate_card_shape, Card, CardDraft, CardShapeError};
use lp_core::ports::{BookChunk, DecisionRecord, FlashCard, TrackerStore};
use schemars::JsonSchema;
use serde::Deserialize;
use std::collections::HashSet;
use tokio_util::sync::CancellationToken;

const CHUNK_CHARS: usize = 2000;

const RULES: &str = "Write self-contained 4-option multiple-choice questions: state any setup \
    inline (never say 'in this project' or refer to a commit/session), keep the correct option \
    no longer than the distractors, make distractors plausible and distinct.";

#[derive(Deserialize, JsonSchema)]
struct CardOut {
    front: String,
    right: String,
    wrong: [String; 3],
    rationale: String,
}

#[derive(Deserialize, JsonSchema)]
struct CardBatch {
    cards: Vec<CardOut>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct CardReport {
    pub saved: usize,
    /// Fronts of drafts that failed the shape gate.
    pub rejected: Vec<(String, CardShapeError)>,
}

/// One engine call per `prompt`; every valid, not-yet-stored draft becomes a card.
/// Existing cards are never rewritten, so their FSRS state survives re-generation.
async fn make_cards<E: LlmEngine>(
    engine: &E,
    store: &dyn TrackerStore,
    prompt: String,
    known: &mut HashSet<String>,
    now_ms: i64,
    cancel: CancellationToken,
    report: &mut CardReport,
) -> Result<(), DistillError> {
    let batch = engine
        .complete_structured::<CardBatch>(
            StructuredRequest {
                prompt,
                model: None,
            },
            cancel,
        )
        .await?
        .value;
    for c in batch.cards {
        let draft = CardDraft {
            front: c.front,
            right: c.right,
            wrong: c.wrong,
        };
        if let Err(e) = validate_card_shape(&draft) {
            report.rejected.push((draft.front, e));
            continue;
        }
        let id = format!("c_{}", short_hash(&[&draft.front]));
        if !known.insert(id.clone()) {
            continue;
        }
        // Spread the answer over A-D deterministically instead of always putting it first.
        let correct = usize::from(id.as_bytes()[id.len() - 1]) % 4;
        let mut wrong = draft.wrong.into_iter();
        let options = std::array::from_fn(|i| {
            if i == correct {
                draft.right.clone()
            } else {
                wrong.next().unwrap_or_default()
            }
        });
        store.upsert_card(&FlashCard {
            id: id.into(),
            front: draft.front,
            options,
            correct_index: correct as u8,
            rationale: c.rationale,
            card: Card::new(now_ms),
        })?;
        report.saved += 1;
    }
    Ok(())
}

async fn run<E: LlmEngine>(
    engine: &E,
    store: &dyn TrackerStore,
    prompts: Vec<String>,
    now_ms: i64,
    cancel: CancellationToken,
) -> Result<CardReport, DistillError> {
    let mut known: HashSet<String> = store
        .cards()?
        .into_iter()
        .map(|c| c.id.to_string())
        .collect();
    let mut report = CardReport::default();
    for p in prompts {
        make_cards(
            engine,
            store,
            p,
            &mut known,
            now_ms,
            cancel.clone(),
            &mut report,
        )
        .await?;
    }
    Ok(report)
}

/// Decision track: `why` / `predict` / `bug` questions.
pub async fn generate_decision_cards<E: LlmEngine>(
    engine: &E,
    store: &dyn TrackerStore,
    decisions: &[DecisionRecord],
    now_ms: i64,
    cancel: CancellationToken,
) -> Result<CardReport, DistillError> {
    let prompts = decisions
        .iter()
        .map(|d| {
            let rejected: String = d
                .alternatives
                .iter()
                .map(|a| format!("\n- rejected {}: {}", a.option, a.rejected_because))
                .collect();
            format!(
                "{RULES}\nKinds: why, predict, bug.\n\nDECISION {}\nwhat: {}\nwhy: {}\nrisk: {}{rejected}",
                d.title, d.what, d.why, d.risk
            )
        })
        .collect();
    run(engine, store, prompts, now_ms, cancel).await
}

/// Book / code track: `why` / `trace` / `ownership` / `bug` / `cloze` questions.
pub async fn generate_book_cards<E: LlmEngine>(
    engine: &E,
    store: &dyn TrackerStore,
    chunks: &[BookChunk],
    now_ms: i64,
    cancel: CancellationToken,
) -> Result<CardReport, DistillError> {
    let prompts = chunks
        .iter()
        .map(|c| {
            format!(
                "{RULES}\nKinds: why, trace, ownership, bug, cloze.\n\nSOURCE {}\n{}",
                c.source, c.text
            )
        })
        .collect();
    run(engine, store, prompts, now_ms, cancel).await
}

/// Splits Markdown (or already-extracted PDF text) into paragraph-aligned chunks and stores them.
pub fn ingest_book(
    store: &dyn TrackerStore,
    source: &str,
    text: &str,
) -> Result<Vec<BookChunk>, DistillError> {
    let mut texts: Vec<String> = vec![];
    for para in text.split("\n\n").map(str::trim).filter(|p| !p.is_empty()) {
        match texts.last_mut() {
            Some(t) if t.chars().count() + para.chars().count() < CHUNK_CHARS => {
                t.push_str("\n\n");
                t.push_str(para);
            }
            _ => texts.push(para.to_string()),
        }
    }
    let mut chunks = vec![];
    for t in texts {
        let chunk = BookChunk {
            id: format!("b_{}", short_hash(&[source, &t])).into(),
            source: source.into(),
            text: t,
        };
        store.upsert_book_chunk(&chunk)?;
        chunks.push(chunk);
    }
    Ok(chunks)
}
