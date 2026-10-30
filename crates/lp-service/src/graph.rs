//! Knowledge graph reads and edits over a workspace's ledgers.

use lp_core::graph::{GraphSnapshot, NodeId};
use lp_core::ports::{GraphStore, StoreResult};
use smol_str::SmolStr;

/// The merged graph as of the ledgers on disk.
pub fn snapshot(g: &dyn GraphStore) -> StoreResult<GraphSnapshot> {
    Ok(GraphSnapshot::from_ledgers(&g.nodes()?, &g.chains()?))
}

/// The chains a deletion of `ids` would also remove.
pub fn delete_preview(g: &dyn GraphStore, ids: &[NodeId]) -> StoreResult<Vec<SmolStr>> {
    Ok(snapshot(g)?.deletion_blast_radius(ids))
}

/// Deletes `ids` and every chain that touches them; returns the removed chain ids.
/// Any read failure aborts before a write, so the chain ledger is never rewritten from a partial read.
pub fn delete_nodes(g: &dyn GraphStore, ids: &[NodeId]) -> StoreResult<Vec<SmolStr>> {
    let chains = g.chains()?;
    let doomed = GraphSnapshot::from_ledgers(&g.nodes()?, &chains).deletion_blast_radius(ids);
    let keep: Vec<_> = chains
        .into_iter()
        .filter(|c| !doomed.contains(&c.id))
        .collect();
    g.rewrite_chains(&keep)?;
    g.delete_nodes(ids)?;
    Ok(doomed)
}

/// The opening learner message of a practice session over the chains `chain_ids`.
pub fn practice_prompt(snap: &GraphSnapshot, chain_ids: &[SmolStr]) -> String {
    let name = |id: &str| snap.nodes.get(id).map_or(id.to_owned(), |n| n.name.clone());
    let lines: Vec<String> = snap
        .chains
        .iter()
        .filter(|c| chain_ids.contains(&c.id))
        .map(|c| {
            let premises: Vec<String> = c.premises.iter().map(|p| name(p)).collect();
            format!("- {} ⟹ {}", premises.join(" ∧ "), name(&c.conclusion))
        })
        .collect();
    format!("Practice these reasoning chains:\n{}", lines.join("\n"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_core::graph::{GraphChain, GraphNode};
    use lp_store::FsGraphStore;

    fn seeded() -> (tempfile::TempDir, FsGraphStore) {
        let d = tempfile::tempdir().unwrap();
        let g = FsGraphStore::open(d.path()).unwrap();
        for (id, name) in [("n_a", "Alpha"), ("n_b", "Beta"), ("n_c", "Gamma")] {
            g.append_node(&GraphNode {
                id: id.into(),
                name: name.into(),
                statement: id.into(),
                aliases: vec![],
                created_at: chrono::DateTime::UNIX_EPOCH,
                provenance: vec![],
            })
            .unwrap();
        }
        let chain = |id: &str, p: &str, c: &str| GraphChain {
            id: id.into(),
            premises: vec![p.into()],
            conclusion: c.into(),
            provenance: vec![],
            created_at: chrono::DateTime::UNIX_EPOCH,
        };
        g.append_chain(&chain("ch1", "n_a", "n_b")).unwrap();
        g.append_chain(&chain("ch2", "n_b", "n_c")).unwrap();
        (d, g)
    }

    #[test]
    fn delete_removes_nodes_and_touching_chains() {
        let (_d, g) = seeded();
        let ids = [SmolStr::from("n_a")];
        assert_eq!(delete_preview(&g, &ids).unwrap(), ["ch1"]);
        assert_eq!(delete_nodes(&g, &ids).unwrap(), ["ch1"]);
        let nodes: Vec<_> = g.nodes().unwrap().into_iter().map(|n| n.id).collect();
        assert_eq!(nodes, ["n_b", "n_c"]);
        let chains: Vec<_> = g.chains().unwrap().into_iter().map(|c| c.id).collect();
        assert_eq!(chains, ["ch2"]);
        assert_eq!(snapshot(&g).unwrap().nodes.len(), 2);
    }

    #[test]
    fn practice_prompt_names_the_selected_chains() {
        let (_d, g) = seeded();
        let p = practice_prompt(&snapshot(&g).unwrap(), &["ch2".into()]);
        assert_eq!(p, "Practice these reasoning chains:\n- Beta ⟹ Gamma");
    }
}
