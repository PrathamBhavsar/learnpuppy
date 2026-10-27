use std::sync::Arc;

use lp_cli_driver::engine::FauxEngine;
use lp_core::graph::{CoverageEntry, GraphChain, GraphNode, Provenance};
use lp_core::ports::{GraphStore, SessionCheckpoint, StoreResult};
use serde_json::{json, Value};

use super::*;

#[derive(Default)]
struct MemGraph {
    nodes: parking_lot::Mutex<Vec<GraphNode>>,
    chains: parking_lot::Mutex<Vec<GraphChain>>,
    coverage: parking_lot::Mutex<Vec<CoverageEntry>>,
    checkpoints: parking_lot::Mutex<Vec<SessionCheckpoint>>,
}

impl GraphStore for MemGraph {
    fn append_node(&self, n: &GraphNode) -> StoreResult<()> {
        self.nodes.lock().push(n.clone());
        Ok(())
    }
    fn nodes(&self) -> StoreResult<Vec<GraphNode>> {
        Ok(self.nodes.lock().clone())
    }
    fn append_chain(&self, c: &GraphChain) -> StoreResult<()> {
        self.chains.lock().push(c.clone());
        Ok(())
    }
    fn chains(&self) -> StoreResult<Vec<GraphChain>> {
        Ok(self.chains.lock().clone())
    }
    fn rewrite_chains(&self, c: &[GraphChain]) -> StoreResult<()> {
        *self.chains.lock() = c.to_vec();
        Ok(())
    }
    fn append_coverage(&self, e: &CoverageEntry) -> StoreResult<()> {
        self.coverage.lock().push(e.clone());
        Ok(())
    }
    fn coverage(&self) -> StoreResult<Vec<CoverageEntry>> {
        Ok(self.coverage.lock().clone())
    }
    fn append_checkpoint(&self, c: &SessionCheckpoint) -> StoreResult<()> {
        self.checkpoints.lock().push(c.clone());
        Ok(())
    }
    fn checkpoints(&self) -> StoreResult<Vec<SessionCheckpoint>> {
        Ok(self.checkpoints.lock().clone())
    }
    fn search_propositions(&self, q: &str, limit: usize) -> StoreResult<Vec<GraphNode>> {
        let q = q.to_lowercase();
        Ok(self
            .nodes
            .lock()
            .iter()
            .filter(|n| n.name.to_lowercase().contains(&q))
            .take(limit)
            .cloned()
            .collect())
    }
}

fn setup() -> (
    Arc<FauxEngine>,
    Arc<MemGraph>,
    Pipeline<FauxEngine, MemGraph>,
) {
    let engine = Arc::new(FauxEngine::new());
    let store = Arc::new(MemGraph::default());
    let pipe = Pipeline::new(engine.clone(), store.clone());
    (engine, store, pipe)
}

fn msg(id: &str, text: &str) -> SourceMessage {
    SourceMessage {
        id: id.into(),
        text: text.into(),
    }
}

fn prop(id: &str, name: &str) -> Value {
    json!({"id": id, "name": name, "statement": format!("{name} holds")})
}

fn id_of(store: &MemGraph, name: &str) -> String {
    store
        .nodes()
        .unwrap()
        .iter()
        .find(|n| n.name == name)
        .unwrap()
        .id
        .to_string()
}

async fn run(p: &Pipeline<FauxEngine, MemGraph>, src: &str, m: &[SourceMessage]) -> usize {
    p.extract(src, m, CancellationToken::new()).await.unwrap()
}

#[tokio::test]
async fn chain_assembles_across_batches() {
    let (engine, store, pipe) = setup();
    engine
        .faux_structured(json!({"propositions": [prop("p1", "A"), prop("p2", "B")], "chains": []}));
    engine.faux_structured(json!({"links": []}));
    assert_eq!(run(&pipe, "c1", &[msg("m1", "a and b")]).await, 1);

    let (a, b) = (id_of(&store, "A"), id_of(&store, "B"));
    engine.faux_structured(json!({
        "propositions": [prop("p1", "C")],
        "chains": [{"premises": [a, b], "conclusion": "p1"}],
    }));
    engine.faux_structured(json!({"links": []}));
    run(&pipe, "c1", &[msg("m2", "so c")]).await;

    let chains = store.chains().unwrap();
    assert_eq!(chains.len(), 1);
    assert_eq!(chains[0].premises.len(), 2);
    assert_eq!(chains[0].conclusion.as_str(), id_of(&store, "C"));
}

#[tokio::test]
async fn auditor_links_equivalent_propositions_across_sessions() {
    let (engine, store, pipe) = setup();
    engine.faux_structured(json!({"propositions": [prop("p1", "Borrowing")], "chains": []}));
    engine.faux_structured(json!({"links": []}));
    run(&pipe, "c1", &[msg("m1", "x")]).await;
    let existing = id_of(&store, "Borrowing");

    engine.faux_structured(json!({"propositions": [prop("p1", "Borrow rule")], "chains": []}));
    engine.faux_structured(json!({"links": [{"id": "m1:p1", "link_to": existing}]}));
    run(&pipe, "c2", &[msg("m1", "y")]).await;

    let snap = GraphSnapshot::from_ledgers(&store.nodes().unwrap(), &[]);
    assert_eq!(snap.nodes.len(), 1);
    let node = &snap.nodes[existing.as_str()];
    assert!(node.aliases.contains(&"Borrow rule".to_string()));
    assert_eq!(node.provenance.len(), 2);
}

#[tokio::test]
async fn redundant_transitive_chain_is_dropped() {
    let (engine, store, pipe) = setup();
    engine.faux_structured(json!({
        "propositions": [prop("a", "A"), prop("b", "B"), prop("c", "C"), prop("x", "X"), prop("y", "Y")],
        "chains": [
            {"premises": ["a", "b"], "conclusion": "x"},
            {"premises": ["x", "c"], "conclusion": "y"},
            {"premises": ["a", "b", "c"], "conclusion": "y"},
        ],
    }));
    engine.faux_structured(json!({"links": []}));
    run(&pipe, "c1", &[msg("m1", "t")]).await;

    let chains = store.chains().unwrap();
    assert_eq!(chains.len(), 2);
    assert!(chains.iter().all(|c| c.premises.len() <= 2));
}

#[tokio::test]
async fn late_shortcut_rewrites_existing_ledger() {
    let (engine, store, pipe) = setup();
    engine.faux_structured(json!({
        "propositions": [prop("a", "A"), prop("b", "B"), prop("c", "C")],
        "chains": [{"premises": ["a", "b"], "conclusion": "c"}],
    }));
    engine.faux_structured(json!({"links": []}));
    run(&pipe, "c1", &[msg("m1", "t")]).await;

    let (a, c) = (id_of(&store, "A"), id_of(&store, "C"));
    engine.faux_structured(
        json!({"propositions": [], "chains": [{"premises": [a], "conclusion": c}]}),
    );
    run(&pipe, "c1", &[msg("m2", "u")]).await;

    let chains = store.chains().unwrap();
    assert_eq!(chains.len(), 1);
    assert_eq!(chains[0].premises.len(), 1);
}

#[tokio::test]
async fn covered_messages_are_skipped_without_llm_calls() {
    let (engine, store, pipe) = setup();
    engine.faux_structured(json!({"propositions": [prop("p1", "A")], "chains": []}));
    engine.faux_structured(json!({"links": []}));
    let m = [msg("m1", "t")];
    assert_eq!(run(&pipe, "c1", &m).await, 1);
    assert_eq!(store.coverage().unwrap().len(), 1);
    // Faux queue is empty: any LLM call would fail with FauxExhausted.
    assert_eq!(run(&pipe, "c1", &m).await, 0);
}

#[tokio::test]
async fn failed_extraction_leaves_messages_uncovered() {
    let (_engine, store, pipe) = setup();
    let r = pipe
        .extract("c1", &[msg("m1", "t")], CancellationToken::new())
        .await;
    assert!(matches!(
        r,
        Err(ExtractError::Driver(DriverError::FauxExhausted))
    ));
    assert!(store.coverage().unwrap().is_empty());
}

#[tokio::test]
async fn notes_import_is_one_shot() {
    let (engine, store, pipe) = setup();
    engine.faux_structured(json!({"propositions": [prop("p1", "A")], "chains": []}));
    engine.faux_structured(json!({"links": []}));
    let notes = "# T\n\nfirst para\n\nsecond para";
    let src = pipe
        .import_notes(notes, CancellationToken::new())
        .await
        .unwrap();
    assert!(src.starts_with("src_"));
    assert_eq!(store.coverage().unwrap().len(), 3);
    assert_eq!(store.checkpoints().unwrap().len(), 1);
    let again = pipe
        .import_notes(notes, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(again, src);
}

#[test]
fn prompts_carry_digest_and_auditor_tools_find_candidates() {
    let store = MemGraph::default();
    store
        .append_node(&GraphNode {
            id: "n_1".into(),
            name: "Ownership".into(),
            statement: "s".into(),
            aliases: vec![],
            created_at: Utc::now(),
            provenance: vec![Provenance {
                chat_id: "c1".into(),
                message_id: "m".into(),
            }],
        })
        .unwrap();
    let m = msg("m9", "text");
    let p = extractor_prompt(&graph_so_far(&store.nodes().unwrap(), "c1"), &[&m]);
    assert!(p.contains("THE GRAPH SO FAR\nn_1: Ownership"));
    assert!(p.contains("TEACHER [m9]"));
    assert!(graph_so_far(&store.nodes().unwrap(), "other").is_empty());

    assert_eq!(audit::list_sessions(&store).unwrap().len(), 1);
    assert_eq!(audit::list_propositions(&store, "c1").unwrap().len(), 1);
    assert_eq!(audit::find(&store, "owner").unwrap()[0].id, "n_1");
}
