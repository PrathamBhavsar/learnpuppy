//! `FsGraphStore`: JSONL ledgers are the source of truth; `graph.db` holds a rebuildable FTS5 index.
use crate::db::{self, fts_query};
use crate::jsonl::{append, be, read_all, rewrite};
use lp_core::graph::{CoverageEntry, GraphChain, GraphNode};
use lp_core::ports::{GraphStore, SessionCheckpoint, StoreResult};
use rusqlite::{params, Connection};
use smol_str::SmolStr;
use std::path::PathBuf;
use std::sync::Mutex;

const MIGRATIONS: &[&str] = &["CREATE VIRTUAL TABLE propositions_fts USING fts5(id UNINDEXED, name, statement, aliases, json UNINDEXED)"];

pub struct FsGraphStore {
    dir: PathBuf,
    db: Mutex<Connection>,
}

impl FsGraphStore {
    /// Opens `dir` (created if missing) and rebuilds the index from `nodes.jsonl`, so a crash between
    /// the ledger append and the index write heals on restart.
    pub fn open(dir: impl Into<PathBuf>) -> StoreResult<Self> {
        let dir = dir.into();
        std::fs::create_dir_all(&dir).map_err(be)?;
        let db = db::open(&dir.join("graph.db"), MIGRATIONS)?;
        db.execute("DELETE FROM propositions_fts", []).map_err(be)?;
        let store = Self {
            dir,
            db: Mutex::new(db),
        };
        for node in store.nodes()? {
            store.index(&node)?;
        }
        Ok(store)
    }

    fn index(&self, node: &GraphNode) -> StoreResult<()> {
        let db = self.db.lock().map_err(be)?;
        db.execute(
            "DELETE FROM propositions_fts WHERE id = ?1",
            [node.id.as_str()],
        )
        .map_err(be)?;
        db.execute(
            "INSERT INTO propositions_fts(id, name, statement, aliases, json) VALUES (?1, ?2, ?3, ?4, ?5)",
            params![node.id.as_str(), node.name, node.statement, node.aliases.join(" "), serde_json::to_string(node).map_err(be)?],
        )
        .map_err(be)?;
        Ok(())
    }

    fn path(&self, file: &str) -> PathBuf {
        self.dir.join(file)
    }
}

impl GraphStore for FsGraphStore {
    fn append_node(&self, node: &GraphNode) -> StoreResult<()> {
        append(&self.path("nodes.jsonl"), node)?;
        self.index(node)
    }
    fn nodes(&self) -> StoreResult<Vec<GraphNode>> {
        read_all(&self.path("nodes.jsonl"))
    }
    fn delete_nodes(&self, ids: &[SmolStr]) -> StoreResult<()> {
        let keep: Vec<_> = self
            .nodes()?
            .into_iter()
            .filter(|n| !ids.contains(&n.id))
            .collect();
        rewrite(&self.path("nodes.jsonl"), &keep)?;
        let db = self.db.lock().map_err(be)?;
        for id in ids {
            db.execute("DELETE FROM propositions_fts WHERE id = ?1", [id.as_str()])
                .map_err(be)?;
        }
        Ok(())
    }
    fn append_chain(&self, chain: &GraphChain) -> StoreResult<()> {
        append(&self.path("chains.jsonl"), chain)
    }
    fn chains(&self) -> StoreResult<Vec<GraphChain>> {
        read_all(&self.path("chains.jsonl"))
    }
    fn rewrite_chains(&self, chains: &[GraphChain]) -> StoreResult<()> {
        rewrite(&self.path("chains.jsonl"), chains)
    }
    fn append_coverage(&self, entry: &CoverageEntry) -> StoreResult<()> {
        append(&self.path("coverage.jsonl"), entry)
    }
    fn coverage(&self) -> StoreResult<Vec<CoverageEntry>> {
        read_all(&self.path("coverage.jsonl"))
    }
    fn append_checkpoint(&self, checkpoint: &SessionCheckpoint) -> StoreResult<()> {
        append(&self.path("sessions.jsonl"), checkpoint)
    }
    fn checkpoints(&self) -> StoreResult<Vec<SessionCheckpoint>> {
        read_all(&self.path("sessions.jsonl"))
    }
    fn search_propositions(&self, query: &str, limit: usize) -> StoreResult<Vec<GraphNode>> {
        let Some(q) = fts_query(query) else {
            return Ok(vec![]);
        };
        let db = self.db.lock().map_err(be)?;
        let mut stmt = db
            .prepare("SELECT json FROM propositions_fts WHERE propositions_fts MATCH ?1 ORDER BY bm25(propositions_fts) LIMIT ?2")
            .map_err(be)?;
        let rows = stmt
            .query_map(params![q, limit as i64], |r| r.get::<_, String>(0))
            .map_err(be)?;
        rows.map(|j| serde_json::from_str(&j.map_err(be)?).map_err(be))
            .collect()
    }
}
