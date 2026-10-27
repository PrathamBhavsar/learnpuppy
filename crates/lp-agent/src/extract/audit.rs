//! Verdicts-only auditor: read-only graph tools plus the prompt built from their results.
//! The auditor never sees the transcript and never mints chains.

use std::collections::BTreeSet;

use lp_core::graph::{GraphNode, SessionFindings};
use lp_core::ports::{GraphStore, StoreResult};

const FIND_LIMIT: usize = 5;
const LIST_LIMIT: usize = 50;

/// `find`: BM25 candidates for a proposition name.
pub fn find(store: &dyn GraphStore, query: &str) -> StoreResult<Vec<GraphNode>> {
    store.search_propositions(query, FIND_LIMIT)
}

/// `list_sessions`: source ids that already contributed propositions.
pub fn list_sessions(store: &dyn GraphStore) -> StoreResult<BTreeSet<String>> {
    Ok(store
        .nodes()?
        .iter()
        .flat_map(|n| n.provenance.iter().map(|p| p.chat_id.to_string()))
        .collect())
}

/// `list_propositions`: the first propositions of one source.
pub fn list_propositions(store: &dyn GraphStore, session: &str) -> StoreResult<Vec<GraphNode>> {
    Ok(store
        .nodes()?
        .into_iter()
        .filter(|n| n.provenance.iter().any(|p| p.chat_id == session))
        .take(LIST_LIMIT)
        .collect())
}

pub fn prompt(store: &dyn GraphStore, findings: &SessionFindings) -> StoreResult<String> {
    let mut p = format!(
        "Emit identity verdicts only as {{\"links\":[{{\"id\":\"p1\",\"link_to\":\"n_...\"}}]}}. \
         Link a fresh proposition to an existing node when both state the same world-facing truth \
         (prefer merging paraphrases). Sessions in the graph: {:?}.\n",
        list_sessions(store)?
    );
    for prop in &findings.propositions {
        p.push_str(&format!(
            "\nFRESH {}: {} - {}\n",
            prop.id, prop.name, prop.statement
        ));
        for c in find(store, &prop.name)? {
            p.push_str(&format!(
                "  CANDIDATE {}: {} - {}\n",
                c.id, c.name, c.statement
            ));
        }
    }
    Ok(p)
}
