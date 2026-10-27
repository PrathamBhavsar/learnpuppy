//! Storage and domain port traits for hexagonal architecture (T10).
//! Plain sync traits and serde records; implementations live in `lp-store`.
use crate::fsrs::{Card, Rating, TurnUsage};
use crate::graph::{CoverageEntry, GraphChain, GraphNode};
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum StoreError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("storage failure: {0}")]
    Backend(String),
}

pub type StoreResult<T> = Result<T, StoreError>;

// ---------------------------------------------------------------- chats

/// `chats/<chat_id>.json`. `turns`/`extractions` are append-only usage logs, separate from `body`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ChatRecord {
    pub id: SmolStr,
    pub title: String,
    pub created_at: i64,
    pub updated_at: i64,
    /// Messages and plan tree, opaque to the port.
    pub body: serde_json::Value,
    pub turns: Vec<TurnUsage>,
    pub extractions: Vec<TurnUsage>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChatSummary {
    pub id: SmolStr,
    pub title: String,
    pub updated_at: i64,
}

pub trait ChatStore: Send + Sync {
    fn load_chat(&self, id: &str) -> StoreResult<ChatRecord>;
    /// Atomic whole-record write.
    fn save_chat(&self, chat: &ChatRecord) -> StoreResult<()>;
    /// Atomically append to `turns`.
    fn append_turn_usage(&self, id: &str, usage: TurnUsage) -> StoreResult<()>;
    /// Atomically append to `extractions`.
    fn append_extraction_usage(&self, id: &str, usage: TurnUsage) -> StoreResult<()>;
    fn list_chats(&self) -> StoreResult<Vec<ChatSummary>>;
}

// ---------------------------------------------------------------- graph

/// `sessions.jsonl` line: one-shot file import checkpoint (`chat_id = "src_<hash>"`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionCheckpoint {
    pub chat_id: SmolStr,
    pub folded_at: i64,
}

pub trait GraphStore: Send + Sync {
    fn append_node(&self, node: &GraphNode) -> StoreResult<()>;
    fn nodes(&self) -> StoreResult<Vec<GraphNode>>;
    fn append_chain(&self, chain: &GraphChain) -> StoreResult<()>;
    fn chains(&self) -> StoreResult<Vec<GraphChain>>;
    /// Atomically replace `chains.jsonl` (used after chain simplification).
    fn rewrite_chains(&self, chains: &[GraphChain]) -> StoreResult<()>;
    fn append_coverage(&self, entry: &CoverageEntry) -> StoreResult<()>;
    fn coverage(&self) -> StoreResult<Vec<CoverageEntry>>;
    fn append_checkpoint(&self, checkpoint: &SessionCheckpoint) -> StoreResult<()>;
    fn checkpoints(&self) -> StoreResult<Vec<SessionCheckpoint>>;
    /// FTS5 BM25 + prefix search over propositions.
    fn search_propositions(&self, query: &str, limit: usize) -> StoreResult<Vec<GraphNode>>;
}

// ---------------------------------------------------------------- jobs

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobKind {
    Research,
    Pdf,
    Visual,
    Demo,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum JobStatus {
    Running,
    Done,
    Failed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeliveryStatus {
    Pending,
    Claimed,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JobRecord {
    pub id: SmolStr,
    pub chat_id: SmolStr,
    pub kind: JobKind,
    pub status: JobStatus,
    pub delivery: DeliveryStatus,
    pub created_at: i64,
    /// Kind-specific output, set on completion.
    pub result: Option<serde_json::Value>,
    pub usage: Vec<TurnUsage>,
}

pub trait JobStore: Send + Sync {
    fn create_job(&self, job: &JobRecord) -> StoreResult<()>;
    fn update_job(&self, job: &JobRecord) -> StoreResult<()>;
    /// Atomically claim pending result deliveries for a chat; each is returned exactly once.
    fn claim_steers(&self, chat_id: &str) -> StoreResult<Vec<JobRecord>>;
    /// Crash recovery: jobs still `Running` after a restart.
    fn interrupted_jobs(&self) -> StoreResult<Vec<JobRecord>>;
}

// ---------------------------------------------------------------- tracker

/// `ingest_cursors` row: fast-skip key for incremental transcript ingestion.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct IngestCursor {
    pub source_key: String,
    pub mtime_ms: i64,
    pub size_bytes: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodingSession {
    pub id: SmolStr,
    /// `claude`, `codex` or `agy`.
    pub source: SmolStr,
    pub cwd: String,
    pub git_branch: Option<String>,
    pub started_at: i64,
    pub transcript: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum DecisionBasis {
    Stated,
    Inferred,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Alternative {
    pub option: String,
    pub rejected_because: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DecisionRecord {
    pub id: SmolStr,
    pub session_id: SmolStr,
    pub title: String,
    pub what: String,
    pub why: String,
    pub basis: DecisionBasis,
    pub alternatives: Vec<Alternative>,
    pub risk: String,
    pub files: Vec<String>,
    pub commit_sha: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookChunk {
    pub id: SmolStr,
    pub source: String,
    pub text: String,
}

/// Four-option multiple-choice card plus its FSRS scheduling state.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FlashCard {
    pub id: SmolStr,
    pub front: String,
    pub options: [String; 4],
    pub correct_index: u8,
    pub rationale: String,
    pub card: Card,
}

/// Append-only `fsrs_reviews` row, also replicated via `sync-<device>.jsonl`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct FsrsReviewLog {
    pub card_id: SmolStr,
    pub ts: i64,
    pub rating: Rating,
    pub device_id: SmolStr,
}

pub trait TrackerStore: Send + Sync {
    fn cursor(&self, source_key: &str) -> StoreResult<Option<IngestCursor>>;
    fn set_cursor(&self, cursor: &IngestCursor) -> StoreResult<()>;
    fn upsert_session(&self, session: &CodingSession) -> StoreResult<()>;
    fn upsert_decision(&self, decision: &DecisionRecord) -> StoreResult<()>;
    fn decisions(&self) -> StoreResult<Vec<DecisionRecord>>;
    fn upsert_book_chunk(&self, chunk: &BookChunk) -> StoreResult<()>;
    fn upsert_card(&self, card: &FlashCard) -> StoreResult<()>;
    fn cards(&self) -> StoreResult<Vec<FlashCard>>;
    fn append_review(&self, review: &FsrsReviewLog) -> StoreResult<()>;
    fn reviews(&self) -> StoreResult<Vec<FsrsReviewLog>>;
    /// Append a review/card replication line to `sync-<device_id>.jsonl`.
    fn append_sync(&self, device_id: &str, line: &serde_json::Value) -> StoreResult<()>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    #[derive(Default)]
    struct Mem {
        chats: Mutex<Vec<ChatRecord>>,
        chains: Mutex<Vec<GraphChain>>,
        jobs: Mutex<Vec<JobRecord>>,
        cursors: Mutex<Vec<IngestCursor>>,
    }

    impl ChatStore for Mem {
        fn load_chat(&self, id: &str) -> StoreResult<ChatRecord> {
            let chats = self.chats.lock().unwrap();
            chats
                .iter()
                .find(|c| c.id == id)
                .cloned()
                .ok_or_else(|| StoreError::NotFound(id.into()))
        }
        fn save_chat(&self, chat: &ChatRecord) -> StoreResult<()> {
            let mut chats = self.chats.lock().unwrap();
            chats.retain(|c| c.id != chat.id);
            chats.push(chat.clone());
            Ok(())
        }
        fn append_turn_usage(&self, id: &str, usage: TurnUsage) -> StoreResult<()> {
            let mut chats = self.chats.lock().unwrap();
            let c = chats
                .iter_mut()
                .find(|c| c.id == id)
                .ok_or_else(|| StoreError::NotFound(id.into()))?;
            c.turns.push(usage);
            Ok(())
        }
        fn append_extraction_usage(&self, id: &str, usage: TurnUsage) -> StoreResult<()> {
            let mut chats = self.chats.lock().unwrap();
            let c = chats
                .iter_mut()
                .find(|c| c.id == id)
                .ok_or_else(|| StoreError::NotFound(id.into()))?;
            c.extractions.push(usage);
            Ok(())
        }
        fn list_chats(&self) -> StoreResult<Vec<ChatSummary>> {
            let chats = self.chats.lock().unwrap();
            Ok(chats
                .iter()
                .map(|c| ChatSummary {
                    id: c.id.clone(),
                    title: c.title.clone(),
                    updated_at: c.updated_at,
                })
                .collect())
        }
    }

    impl GraphStore for Mem {
        fn append_node(&self, _: &GraphNode) -> StoreResult<()> {
            Ok(())
        }
        fn nodes(&self) -> StoreResult<Vec<GraphNode>> {
            Ok(vec![])
        }
        fn append_chain(&self, chain: &GraphChain) -> StoreResult<()> {
            self.chains.lock().unwrap().push(chain.clone());
            Ok(())
        }
        fn chains(&self) -> StoreResult<Vec<GraphChain>> {
            Ok(self.chains.lock().unwrap().clone())
        }
        fn rewrite_chains(&self, chains: &[GraphChain]) -> StoreResult<()> {
            *self.chains.lock().unwrap() = chains.to_vec();
            Ok(())
        }
        fn append_coverage(&self, _: &CoverageEntry) -> StoreResult<()> {
            Ok(())
        }
        fn coverage(&self) -> StoreResult<Vec<CoverageEntry>> {
            Ok(vec![])
        }
        fn append_checkpoint(&self, _: &SessionCheckpoint) -> StoreResult<()> {
            Ok(())
        }
        fn checkpoints(&self) -> StoreResult<Vec<SessionCheckpoint>> {
            Ok(vec![])
        }
        fn search_propositions(&self, _: &str, _: usize) -> StoreResult<Vec<GraphNode>> {
            Ok(vec![])
        }
    }

    impl JobStore for Mem {
        fn create_job(&self, job: &JobRecord) -> StoreResult<()> {
            self.jobs.lock().unwrap().push(job.clone());
            Ok(())
        }
        fn update_job(&self, job: &JobRecord) -> StoreResult<()> {
            let mut jobs = self.jobs.lock().unwrap();
            let j = jobs
                .iter_mut()
                .find(|j| j.id == job.id)
                .ok_or_else(|| StoreError::NotFound(job.id.to_string()))?;
            *j = job.clone();
            Ok(())
        }
        fn claim_steers(&self, chat_id: &str) -> StoreResult<Vec<JobRecord>> {
            let mut jobs = self.jobs.lock().unwrap();
            let mut out = vec![];
            for j in jobs.iter_mut().filter(|j| {
                j.chat_id == chat_id
                    && j.status == JobStatus::Done
                    && j.delivery == DeliveryStatus::Pending
            }) {
                j.delivery = DeliveryStatus::Claimed;
                out.push(j.clone());
            }
            Ok(out)
        }
        fn interrupted_jobs(&self) -> StoreResult<Vec<JobRecord>> {
            let jobs = self.jobs.lock().unwrap();
            Ok(jobs
                .iter()
                .filter(|j| j.status == JobStatus::Running)
                .cloned()
                .collect())
        }
    }

    impl TrackerStore for Mem {
        fn cursor(&self, key: &str) -> StoreResult<Option<IngestCursor>> {
            Ok(self
                .cursors
                .lock()
                .unwrap()
                .iter()
                .find(|c| c.source_key == key)
                .cloned())
        }
        fn set_cursor(&self, cursor: &IngestCursor) -> StoreResult<()> {
            let mut cs = self.cursors.lock().unwrap();
            cs.retain(|c| c.source_key != cursor.source_key);
            cs.push(cursor.clone());
            Ok(())
        }
        fn upsert_session(&self, _: &CodingSession) -> StoreResult<()> {
            Ok(())
        }
        fn upsert_decision(&self, _: &DecisionRecord) -> StoreResult<()> {
            Ok(())
        }
        fn decisions(&self) -> StoreResult<Vec<DecisionRecord>> {
            Ok(vec![])
        }
        fn upsert_book_chunk(&self, _: &BookChunk) -> StoreResult<()> {
            Ok(())
        }
        fn upsert_card(&self, _: &FlashCard) -> StoreResult<()> {
            Ok(())
        }
        fn cards(&self) -> StoreResult<Vec<FlashCard>> {
            Ok(vec![])
        }
        fn append_review(&self, _: &FsrsReviewLog) -> StoreResult<()> {
            Ok(())
        }
        fn reviews(&self) -> StoreResult<Vec<FsrsReviewLog>> {
            Ok(vec![])
        }
        fn append_sync(&self, _: &str, _: &serde_json::Value) -> StoreResult<()> {
            Ok(())
        }
    }

    fn chat(id: &str) -> ChatRecord {
        ChatRecord {
            id: id.into(),
            title: "t".into(),
            created_at: 1,
            updated_at: 2,
            body: serde_json::Value::Null,
            turns: vec![],
            extractions: vec![],
        }
    }

    #[test]
    fn chat_store_appends_usage() {
        let s = Mem::default();
        s.save_chat(&chat("c1")).unwrap();
        s.append_turn_usage(
            "c1",
            TurnUsage {
                input_tokens: 5,
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(s.load_chat("c1").unwrap().turns[0].input_tokens, 5);
        assert_eq!(s.list_chats().unwrap().len(), 1);
        assert!(matches!(s.load_chat("x"), Err(StoreError::NotFound(_))));
    }

    #[test]
    fn graph_store_rewrite_replaces_chains() {
        let s = Mem::default();
        let c = GraphChain {
            id: "arg_1".into(),
            premises: vec!["n_a".into()],
            conclusion: "n_b".into(),
            provenance: vec![],
            created_at: chrono::DateTime::UNIX_EPOCH,
        };
        s.append_chain(&c).unwrap();
        s.append_chain(&c).unwrap();
        s.rewrite_chains(&[c]).unwrap();
        assert_eq!(s.chains().unwrap().len(), 1);
    }

    #[test]
    fn job_store_claims_each_steer_once() {
        let s = Mem::default();
        let job = JobRecord {
            id: "j1".into(),
            chat_id: "c1".into(),
            kind: JobKind::Research,
            status: JobStatus::Running,
            delivery: DeliveryStatus::Pending,
            created_at: 0,
            result: None,
            usage: vec![],
        };
        s.create_job(&job).unwrap();
        assert_eq!(s.interrupted_jobs().unwrap().len(), 1);
        s.update_job(&JobRecord {
            status: JobStatus::Done,
            ..job
        })
        .unwrap();
        assert_eq!(s.claim_steers("c1").unwrap().len(), 1);
        assert!(s.claim_steers("c1").unwrap().is_empty());
    }

    #[test]
    fn tracker_store_cursor_roundtrip() {
        let s = Mem::default();
        assert_eq!(s.cursor("claude:/a").unwrap(), None);
        let cur = IngestCursor {
            source_key: "claude:/a".into(),
            mtime_ms: 1,
            size_bytes: 2,
        };
        s.set_cursor(&cur).unwrap();
        assert_eq!(s.cursor("claude:/a").unwrap(), Some(cur));
    }
}
