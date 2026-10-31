//! Graph mutation transactions and the practice-chat integration point.
use lp_core::graph::{simplify_chains, NodeId};
use lp_core::ports::GraphStore;
use parking_lot::Mutex;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::graph::{self, Loaded};
use crate::workspace::WorkspaceHandle;

#[derive(Debug, thiserror::Error)]
pub enum MutationError {
    #[error("select at least one existing node")]
    Selection,
    #[error("the graph changed; preview the deletion again")]
    Stale,
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Preview {
    pub deleted: usize,
    pub blast_radius: Vec<String>,
    pub token: String,
}

/// Serializes previews, deletes and layout writes, including the background read-side layout.
#[derive(Default)]
pub struct GraphMutations {
    locks: Mutex<HashMap<PathBuf, Arc<Mutex<()>>>>,
}

impl GraphMutations {
    pub fn workspace_lock(&self, root: &Path) -> Arc<Mutex<()>> {
        let mut locks = self.locks.lock();
        locks.retain(|path, lock| path == root || Arc::strong_count(lock) > 1);
        locks.entry(root.to_owned()).or_default().clone()
    }
    pub fn preview(&self, ws: &WorkspaceHandle, ids: &[NodeId]) -> Result<Preview, MutationError> {
        let lock = self.workspace_lock(ws.root());
        let _guard = lock.lock();
        preview(ws, ids)
    }

    pub fn delete(
        &self,
        ws: &WorkspaceHandle,
        ids: &[NodeId],
        token: &str,
    ) -> Result<Loaded, MutationError> {
        let lock = self.workspace_lock(ws.root());
        let _guard = lock.lock();
        ws.ensure_writable().map_err(anyhow::Error::from)?;
        let p = preview(ws, ids).map_err(|e| match e {
            MutationError::Selection => MutationError::Stale,
            other => other,
        })?;
        if p.token != token {
            return Err(MutationError::Stale);
        }
        let g = &*ws.stores().graph;
        let snap = graph::snapshot(g).map_err(anyhow::Error::from)?;
        let keep: Vec<_> = snap
            .chains
            .into_iter()
            .filter(|c| !p.blast_radius.contains(&c.id.to_string()))
            .filter(|c| {
                snap.nodes.contains_key(&c.conclusion)
                    && c.premises.iter().all(|p| snap.nodes.contains_key(p))
            })
            .collect();
        // Remove dangling chains first so a failed node write leaves no dangling references.
        g.rewrite_chains(&simplify_chains(&keep))
            .map_err(anyhow::Error::from)?;
        g.delete_nodes(ids).map_err(anyhow::Error::from)?;
        let loaded = Loaded::new(graph::snapshot(g).map_err(anyhow::Error::from)?);
        relayout(ws.root(), &loaded).map_err(MutationError::Other)?;
        ws.emit(
            "graph.changed",
            serde_json::json!({"version": loaded.version}),
        );
        Ok(loaded)
    }

    pub fn relayout(&self, ws: &WorkspaceHandle) -> anyhow::Result<Loaded> {
        let lock = self.workspace_lock(ws.root());
        let _guard = lock.lock();
        ws.ensure_writable()?;
        let loaded = Loaded::new(graph::snapshot(&*ws.stores().graph)?);
        relayout(ws.root(), &loaded)?;
        ws.emit(
            "graph.changed",
            serde_json::json!({"version": loaded.version}),
        );
        Ok(loaded)
    }
}

fn preview(ws: &WorkspaceHandle, ids: &[NodeId]) -> Result<Preview, MutationError> {
    let g = &*ws.stores().graph;
    let nodes = g.nodes().map_err(anyhow::Error::from)?;
    let chains = g.chains().map_err(anyhow::Error::from)?;
    let snap = lp_core::graph::GraphSnapshot::from_ledgers(&nodes, &chains);
    if ids.is_empty() || ids.iter().any(|id| !snap.nodes.contains_key(id)) {
        return Err(MutationError::Selection);
    }
    let mut selected = ids.to_vec();
    selected.sort();
    selected.dedup();
    let doomed = snap.deletion_blast_radius(&selected);
    let survivors: Vec<_> = snap
        .chains
        .iter()
        .filter(|c| !doomed.contains(&c.id))
        .filter(|c| {
            snap.nodes.contains_key(&c.conclusion)
                && c.premises.iter().all(|p| snap.nodes.contains_key(p))
        })
        .cloned()
        .collect();
    let survivors = simplify_chains(&survivors);
    let mut blast_radius: Vec<_> = snap
        .chains
        .iter()
        .filter(|c| !survivors.iter().any(|kept| kept.id == c.id))
        .map(|c| c.id.to_string())
        .collect();
    blast_radius.sort();
    // Bind to full ledger content, workspace and selection, including provenance and chain text.
    let bytes =
        serde_json::to_vec(&(ws.id(), &selected, nodes, chains)).map_err(anyhow::Error::from)?;
    Ok(Preview {
        deleted: selected.len(),
        blast_radius,
        token: hex::encode(Sha256::digest(bytes)),
    })
}

fn relayout(root: &Path, loaded: &Loaded) -> anyhow::Result<()> {
    let dir = root.join("graph");
    let previous = std::fs::read(dir.join(graph::POSITIONS_FILE))
        .ok()
        .and_then(|b| serde_json::from_slice::<graph::Positions>(&b).ok());
    let mut positions = graph::compute_positions(loaded);
    positions.layout_version = previous.map_or(loaded.version, |p| {
        p.layout_version.max(p.version).saturating_add(1)
    });
    graph::write_positions(&dir, &positions)
}

/// B21 supplies a chat creator implementing this interface when its chat routes merge.
#[async_trait::async_trait]
pub trait PracticeSeeder: Send + Sync {
    async fn seed(
        &self,
        workspace: Arc<WorkspaceHandle>,
        node_ids: Vec<String>,
    ) -> anyhow::Result<Option<String>>;
}

#[derive(Default)]
pub struct UnavailablePracticeSeeder;
#[async_trait::async_trait]
impl PracticeSeeder for UnavailablePracticeSeeder {
    async fn seed(
        &self,
        _: Arc<WorkspaceHandle>,
        _: Vec<String>,
    ) -> anyhow::Result<Option<String>> {
        Ok(None)
    }
}
