use super::*;
use chrono::DateTime;
use lp_core::fsrs::{Card, Rating, TurnUsage};
use lp_core::graph::{CoverageEntry, GraphChain, GraphNode};
use lp_core::ports::*;
use std::io::Write;
use std::sync::Arc;

fn node(id: &str, name: &str, statement: &str) -> GraphNode {
    GraphNode {
        id: id.into(),
        name: name.into(),
        statement: statement.into(),
        aliases: vec![],
        created_at: DateTime::UNIX_EPOCH,
        provenance: vec![],
    }
}

fn chain(id: &str) -> GraphChain {
    GraphChain {
        id: id.into(),
        premises: vec!["n_a".into()],
        conclusion: "n_b".into(),
        provenance: vec![],
        created_at: DateTime::UNIX_EPOCH,
    }
}

fn chat(id: &str, updated_at: i64) -> ChatRecord {
    ChatRecord {
        id: id.into(),
        title: id.into(),
        created_at: 1,
        updated_at,
        body: serde_json::json!({"messages": []}),
        turns: vec![],
        extractions: vec![],
    }
}

fn job(id: &str, status: JobStatus) -> JobRecord {
    JobRecord {
        id: id.into(),
        chat_id: "c1".into(),
        kind: JobKind::Research,
        status,
        delivery: DeliveryStatus::Pending,
        created_at: 0,
        result: None,
        usage: vec![],
    }
}

#[test]
fn ledger_round_trips_and_rewrites_chains() {
    let dir = tempfile::tempdir().unwrap();
    let s = FsGraphStore::open(dir.path()).unwrap();
    s.append_chain(&chain("a1")).unwrap();
    s.append_chain(&chain("a2")).unwrap();
    assert_eq!(s.chains().unwrap().len(), 2);
    s.rewrite_chains(&[chain("a2")]).unwrap();
    assert_eq!(s.chains().unwrap(), vec![chain("a2")]);
    let cov = CoverageEntry { chat_id: "c".into(), fp: "f".into(), tokens: 3, folded_at: DateTime::UNIX_EPOCH };
    s.append_coverage(&cov).unwrap();
    s.append_checkpoint(&SessionCheckpoint { chat_id: "src_1".into(), folded_at: 9 }).unwrap();
    assert_eq!(s.coverage().unwrap(), vec![cov]);
    assert_eq!(s.checkpoints().unwrap().len(), 1);
}

#[test]
fn torn_and_corrupt_tail_is_dropped_then_overwritten() {
    let dir = tempfile::tempdir().unwrap();
    let s = FsGraphStore::open(dir.path()).unwrap();
    s.append_chain(&chain("a1")).unwrap();
    let path = dir.path().join("chains.jsonl");
    // crash mid-write: half a line, then a bad-CRC full line
    let mut f = std::fs::OpenOptions::new().append(true).open(&path).unwrap();
    f.write_all(b"deadbeef\t{\"id\":\"zz\"}\n0123").unwrap();
    assert_eq!(s.chains().unwrap(), vec![chain("a1")]);
    s.append_chain(&chain("a2")).unwrap();
    assert_eq!(s.chains().unwrap(), vec![chain("a1"), chain("a2")]);
}

#[test]
fn atomic_write_survives_crash_before_rename() {
    let dir = tempfile::tempdir().unwrap();
    let s = FsChatStore::open(dir.path()).unwrap();
    s.save_chat(&chat("c1", 1)).unwrap();
    // crash left a partial temp file; the committed chat is untouched and not listed twice
    std::fs::write(dir.path().join(".c1.json.tmp"), b"{\"id\":").unwrap();
    assert_eq!(s.load_chat("c1").unwrap(), chat("c1", 1));
    assert_eq!(s.list_chats().unwrap().len(), 1);
    s.save_chat(&chat("c1", 2)).unwrap();
    assert_eq!(s.load_chat("c1").unwrap().updated_at, 2);
}

#[test]
fn chat_usage_appends_and_ids_are_confined() {
    let dir = tempfile::tempdir().unwrap();
    let s = Arc::new(FsChatStore::open(dir.path()).unwrap());
    s.save_chat(&chat("c1", 1)).unwrap();
    s.save_chat(&chat("c2", 5)).unwrap();
    let threads: Vec<_> = (0..8)
        .map(|_| {
            let s = s.clone();
            std::thread::spawn(move || s.append_turn_usage("c1", TurnUsage { input_tokens: 1, ..Default::default() }).unwrap())
        })
        .collect();
    threads.into_iter().for_each(|t| t.join().unwrap());
    s.append_extraction_usage("c1", TurnUsage::default()).unwrap();
    let c = s.load_chat("c1").unwrap();
    assert_eq!((c.turns.len(), c.extractions.len()), (8, 1));
    assert_eq!(s.list_chats().unwrap()[0].id, "c2");
    assert!(matches!(s.load_chat("nope"), Err(StoreError::NotFound(_))));
    assert!(matches!(s.load_chat("../x"), Err(StoreError::Backend(_))));
}

#[test]
fn claim_steers_is_exactly_once_under_contention() {
    let dir = tempfile::tempdir().unwrap();
    let s = Arc::new(FsJobStore::open(dir.path()).unwrap());
    s.create_job(&job("j1", JobStatus::Running)).unwrap();
    s.create_job(&job("j2", JobStatus::Running)).unwrap();
    assert_eq!(s.interrupted_jobs().unwrap().len(), 2);
    s.update_job(&job("j1", JobStatus::Done)).unwrap();
    assert!(matches!(s.update_job(&job("zz", JobStatus::Done)), Err(StoreError::NotFound(_))));
    let claims: Vec<_> = (0..4)
        .map(|_| {
            let s = s.clone();
            std::thread::spawn(move || s.claim_steers("c1").unwrap().len())
        })
        .collect();
    assert_eq!(claims.into_iter().map(|t| t.join().unwrap()).sum::<usize>(), 1);
    assert_eq!(s.interrupted_jobs().unwrap().len(), 1);
}

#[test]
fn fts_ranks_prefix_and_survives_reindex() {
    let dir = tempfile::tempdir().unwrap();
    let s = FsGraphStore::open(dir.path()).unwrap();
    s.append_node(&node("n1", "Gradient", "gradient descent follows the gradient of the loss")).unwrap();
    s.append_node(&node("n2", "Loss", "a loss measures error; gradient is mentioned once")).unwrap();
    s.append_node(&node("n3", "Cats", "felines purr")).unwrap();
    let ids = |q: &str| s.search_propositions(q, 10).unwrap().into_iter().map(|n| n.id.to_string()).collect::<Vec<_>>();
    assert_eq!(ids("gradient"), ["n1", "n2"]);
    assert_eq!(ids("grad"), ["n1", "n2"]);
    assert_eq!(ids("fel\""), ["n3"]);
    assert!(ids("   ").is_empty());
    // link-append of an existing id replaces the index row instead of duplicating it
    s.append_node(&node("n3", "Cats", "felines purr and nap")).unwrap();
    assert_eq!(ids("felines").len(), 1);
    // reopening rebuilds the index from the ledger, including after a lost index
    drop(s);
    std::fs::remove_file(dir.path().join("graph.db")).unwrap();
    let s = FsGraphStore::open(dir.path()).unwrap();
    assert_eq!(s.search_propositions("nap", 10).unwrap().len(), 1);
    assert_eq!(s.nodes().unwrap().len(), 4);
}

fn session(id: &str, transcript: &str) -> CodingSession {
    CodingSession {
        id: id.into(),
        source: "claude".into(),
        cwd: "/w".into(),
        git_branch: None,
        started_at: 1,
        transcript: transcript.into(),
    }
}

#[test]
fn transcript_fts_ranks_by_bm25() {
    let dir = tempfile::tempdir().unwrap();
    let s = SqliteTrackerStore::open(dir.path()).unwrap();
    s.upsert_session(&session("s1", "refactor the parser. the parser handles tokens; parser parser")).unwrap();
    s.upsert_session(&session("s2", "one mention of parser in a long unrelated discussion about databases and indexes and many other things")).unwrap();
    s.upsert_session(&session("s3", "nothing relevant")).unwrap();
    assert_eq!(s.search_transcripts("pars", 5).unwrap(), ["s1", "s2"]);
    s.upsert_session(&session("s1", "rewritten")).unwrap();
    assert_eq!(s.search_transcripts("parser", 5).unwrap(), ["s2"]);
}

#[test]
fn tracker_tables_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let s = SqliteTrackerStore::open(dir.path()).unwrap();
    assert_eq!(s.cursor("k").unwrap(), None);
    let mut cur = IngestCursor { source_key: "k".into(), mtime_ms: 1, size_bytes: 2 };
    s.set_cursor(&cur).unwrap();
    cur.mtime_ms = 7;
    s.set_cursor(&cur).unwrap();
    assert_eq!(s.cursor("k").unwrap(), Some(cur));

    let mut d = DecisionRecord {
        id: "d1".into(),
        session_id: "s1".into(),
        title: "t".into(),
        what: "w".into(),
        why: "y".into(),
        basis: DecisionBasis::Stated,
        alternatives: vec![Alternative { option: "o".into(), rejected_because: "r".into() }],
        risk: "low".into(),
        files: vec!["a.rs".into()],
        commit_sha: None,
    };
    s.upsert_decision(&d).unwrap();
    d.risk = "high".into();
    s.upsert_decision(&d).unwrap();
    assert_eq!(s.decisions().unwrap(), vec![d]);

    s.upsert_book_chunk(&BookChunk { id: "b".into(), source: "s".into(), text: "t".into() }).unwrap();
    let card = FlashCard {
        id: "f1".into(),
        front: "q".into(),
        options: ["a".into(), "b".into(), "c".into(), "d".into()],
        correct_index: 2,
        rationale: "r".into(),
        card: Card::new(10),
    };
    s.upsert_card(&card).unwrap();
    assert_eq!(s.cards().unwrap(), vec![card]);
    let r = FsrsReviewLog { card_id: "f1".into(), ts: 5, rating: Rating::Good, device_id: "d".into() };
    s.append_review(&r).unwrap();
    s.append_review(&r).unwrap();
    assert_eq!(s.reviews().unwrap(), vec![r]);
}

#[test]
fn sync_files_export_and_import_idempotently() {
    let dir = tempfile::tempdir().unwrap();
    let a = SqliteTrackerStore::open(dir.path().join("a")).unwrap();
    let r = FsrsReviewLog { card_id: "f1".into(), ts: 5, rating: Rating::Easy, device_id: "phone".into() };
    let mut line = serde_json::to_value(&r).unwrap();
    line["kind"] = "review".into();
    a.append_sync("phone", &line).unwrap();

    // another device's folder syncs the file in
    let b = SqliteTrackerStore::open(dir.path().join("b")).unwrap();
    std::fs::copy(dir.path().join("a/sync-phone.jsonl"), dir.path().join("b/sync-phone.jsonl")).unwrap();
    b.import_sync("laptop").unwrap();
    b.import_sync("laptop").unwrap();
    assert_eq!(b.reviews().unwrap(), vec![r.clone()]);
    // a device never re-imports its own file
    let c = SqliteTrackerStore::open(dir.path().join("b")).unwrap();
    c.import_sync("phone").unwrap();
    assert_eq!(c.reviews().unwrap().len(), 1);
}

#[test]
fn wal_readers_see_writes_from_another_connection() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().to_path_buf();
    let writer = SqliteTrackerStore::open(&path).unwrap();
    let reader = SqliteTrackerStore::open(&path).unwrap();
    let w = std::thread::spawn(move || {
        for ts in 0..50 {
            let r = FsrsReviewLog { card_id: "c".into(), ts, rating: Rating::Good, device_id: "d".into() };
            writer.append_review(&r).unwrap();
        }
    });
    let mut last = 0;
    while !w.is_finished() {
        let n = reader.reviews().unwrap().len();
        assert!(n >= last, "reads never go backwards");
        last = n;
    }
    w.join().unwrap();
    assert_eq!(reader.reviews().unwrap().len(), 50);
}
