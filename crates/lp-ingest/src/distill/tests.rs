use super::*;
use lp_agent::extract::Pipeline;
use lp_cli_driver::engine::FauxEngine;
use lp_core::fsrs::CardShapeError;
use lp_store::{FsGraphStore, SqliteTrackerStore};
use serde_json::json;
use std::sync::Arc;

fn session() -> CodingSession {
    CodingSession {
        id: "claude:s1".into(),
        source: "claude".into(),
        cwd: "/r".into(),
        git_branch: None,
        started_at: 0,
        transcript: "U: why a snapshot copy?\nA: WAL files are live.\n".into(),
    }
}

fn commit(sha: &str) -> GitCommit {
    GitCommit {
        sha: sha.into(),
        subject: "snapshot".into(),
        body: String::new(),
        files: vec!["src/agy.rs".into()],
        diff: "+copy".into(),
    }
}

fn decision(sha: &str) -> serde_json::Value {
    json!({
        "title": "Snapshot copy for WAL ingestion",
        "what": "agy.rs copies the db before reading.",
        "why": "The live WAL-mode db may be mid-write.",
        "basis": "stated",
        "alternatives": [{"option": "open in place", "rejected_because": "locks the writer"}],
        "risk": "copy can be torn",
        "files": ["src/agy.rs"],
        "commit_sha": sha,
    })
}

fn stores() -> (tempfile::TempDir, SqliteTrackerStore) {
    let dir = tempfile::tempdir().unwrap();
    let store = SqliteTrackerStore::open(dir.path()).unwrap();
    (dir, store)
}

fn card(front: &str, right: &str, wrong: [&str; 3]) -> serde_json::Value {
    json!({"front": front, "right": right, "wrong": wrong, "rationale": "because"})
}

#[tokio::test]
async fn distills_and_drops_invented_commit_shas() {
    let (_d, store) = stores();
    let engine = FauxEngine::new();
    let go = |sha: &'static str| async {
        engine.faux_structured(json!({"decisions": [decision(sha)]}));
        let (sess, commits) = (session(), [commit("abc123")]);
        distill_session(&engine, &store, &sess, &commits, CancellationToken::new()).await
    };
    let real = go("abc123").await.unwrap();
    assert_eq!(real[0].commit_sha.as_deref(), Some("abc123"));
    assert_eq!(real[0].basis, DecisionBasis::Stated);
    let invented = go("deadbeef").await.unwrap();
    assert_eq!(invented[0].commit_sha, None);
    // same session + title upserts one row
    assert_eq!(store.decisions().unwrap().len(), 1);
}

#[tokio::test]
async fn decisions_bridge_into_the_graph() {
    let (_d, store) = stores();
    let gdir = tempfile::tempdir().unwrap();
    let graph = Arc::new(FsGraphStore::open(gdir.path()).unwrap());
    let engine = Arc::new(FauxEngine::new());
    engine.faux_structured(json!({"decisions": [decision("abc123")]}));
    let recs = distill_session(
        engine.as_ref(),
        &store,
        &session(),
        &[commit("abc123")],
        CancellationToken::new(),
    )
    .await
    .unwrap();

    engine.faux_structured(json!({
        "propositions": [{"id": "p1", "name": "Snapshot before reading WAL", "statement": "Copy a live WAL db before reading it."}],
        "chains": [],
    }));
    engine.faux_structured(json!({"links": []}));
    let pipe = Pipeline::new(engine, graph.clone());
    let n = emit_to_graph(&pipe, "claude:s1", &recs, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(n, 1);
    let nodes = graph.nodes().unwrap();
    assert_eq!(nodes[0].name, "Snapshot before reading WAL");
    assert_eq!(nodes[0].provenance[0].message_id, recs[0].id);
}

#[tokio::test]
async fn card_shape_gate_rejects_bad_drafts_and_places_the_answer() {
    let (_d, store) = stores();
    let engine = FauxEngine::new();
    let d = DecisionRecord {
        id: "d1".into(),
        session_id: "claude:s1".into(),
        title: "t".into(),
        what: "w".into(),
        why: "y".into(),
        basis: DecisionBasis::Inferred,
        alternatives: vec![],
        risk: "r".into(),
        files: vec![],
        commit_sha: None,
    };
    engine.faux_structured(json!({"cards": [
        card("Why copy a WAL-mode SQLite db before reading it?", "live writes", ["disk space", "file mode", "page size"]),
        card("Why did we copy the db in this project?", "live writes", ["disk space", "file mode", "page size"]),
        card("Why copy a WAL db before reading it?", "because the live database file may be mid-write and torn", ["no", "x", "y"]),
    ]}));
    let rep = generate_decision_cards(
        &engine,
        &store,
        std::slice::from_ref(&d),
        1000,
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(rep.saved, 1);
    assert_eq!(rep.rejected[0].1, CardShapeError::ContextDependent);
    assert_eq!(rep.rejected[1].1, CardShapeError::LengthTell);
    let cards = store.cards().unwrap();
    assert_eq!(cards.len(), 1);
    assert_eq!(
        cards[0].options[cards[0].correct_index as usize],
        "live writes"
    );
    assert_eq!(cards[0].card.due, 1000);

    // regenerating the same front must not reset the stored card
    engine.faux_structured(json!({"cards": [
        card("Why copy a WAL-mode SQLite db before reading it?", "live writes", ["disk space", "file mode", "page size"]),
    ]}));
    let again = generate_decision_cards(
        &engine,
        &store,
        std::slice::from_ref(&d),
        9999,
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(again.saved, 0);
    assert_eq!(store.cards().unwrap()[0].card.due, 1000);
}

#[tokio::test]
async fn book_chunks_become_cards() {
    let (_d, store) = stores();
    let text = format!("# Ownership\n\n{}\n\nsecond paragraph", "a".repeat(1980));
    let chunks = ingest_book(&store, "rust-book.md", &text).unwrap();
    assert_eq!(chunks.len(), 2);
    let again = ingest_book(&store, "rust-book.md", &text).unwrap();
    assert_eq!(chunks, again);

    let engine = FauxEngine::new();
    engine.faux_structured(json!({"cards": [
        card("What does moving a `String` into a function do?", "transfers ownership", ["copies bytes", "borrows it", "pins it"]),
    ]}));
    engine.faux_structured(json!({"cards": []}));
    let rep = generate_book_cards(&engine, &store, &chunks, 5, CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(rep.saved, 1);
}

#[test]
fn seed_ingests_only_tracker_and_bunfiiliate() {
    let (_d, store) = stores();
    let home = tempfile::tempdir().unwrap();
    for repo in ["tracker", "bunfiiliate", "other"] {
        let cwd = home.path().join("code").join(repo);
        std::fs::create_dir_all(&cwd).unwrap();
        let dir = home.path().join(".claude/projects").join(repo);
        std::fs::create_dir_all(&dir).unwrap();
        let line = json!({
            "type": "user", "cwd": cwd, "timestamp": "2026-05-01T10:00:00Z",
            "message": {"content": format!("work in {repo}")},
        });
        std::fs::write(dir.join(format!("{repo}.jsonl")), format!("{line}\n")).unwrap();
    }
    let rep = seed_rust_sysdesign(&store, home.path());
    assert_eq!((rep.sessions, rep.errors.len()), (2, 0));
}
