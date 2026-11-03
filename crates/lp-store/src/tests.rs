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

/// `updated_at` is an offset from a realistic epoch-ms base (small values would read as seconds).
fn chat(id: &str, updated_at: i64) -> ChatRecord {
    ChatRecord {
        id: id.into(),
        title: id.into(),
        created_at: 1_700_000_000_000,
        updated_at: 1_700_000_000_000 + updated_at,
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
    let cov = CoverageEntry {
        chat_id: "c".into(),
        fp: "f".into(),
        tokens: 3,
        folded_at: DateTime::UNIX_EPOCH,
    };
    s.append_coverage(&cov).unwrap();
    s.append_checkpoint(&SessionCheckpoint {
        chat_id: "src_1".into(),
        folded_at: 9,
    })
    .unwrap();
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
    let mut f = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap();
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
    assert_eq!(s.load_chat("c1").unwrap().updated_at, 1_700_000_000_002);
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
            std::thread::spawn(move || {
                s.append_turn_usage(
                    "c1",
                    TurnUsage {
                        input_tokens: 1,
                        ..Default::default()
                    },
                )
                .unwrap()
            })
        })
        .collect();
    threads.into_iter().for_each(|t| t.join().unwrap());
    s.append_extraction_usage("c1", TurnUsage::default())
        .unwrap();
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
    assert!(matches!(
        s.update_job(&job("zz", JobStatus::Done)),
        Err(StoreError::NotFound(_))
    ));
    let claims: Vec<_> = (0..4)
        .map(|_| {
            let s = s.clone();
            std::thread::spawn(move || s.claim_steers("c1").unwrap().len())
        })
        .collect();
    assert_eq!(
        claims.into_iter().map(|t| t.join().unwrap()).sum::<usize>(),
        1
    );
    assert_eq!(s.interrupted_jobs().unwrap().len(), 1);
}

#[test]
fn fts_ranks_prefix_and_survives_reindex() {
    let dir = tempfile::tempdir().unwrap();
    let s = FsGraphStore::open(dir.path()).unwrap();
    s.append_node(&node(
        "n1",
        "Gradient",
        "gradient descent follows the gradient of the loss",
    ))
    .unwrap();
    s.append_node(&node(
        "n2",
        "Loss",
        "a loss measures error; gradient is mentioned once",
    ))
    .unwrap();
    s.append_node(&node("n3", "Cats", "felines purr")).unwrap();
    let ids = |q: &str| {
        s.search_propositions(q, 10)
            .unwrap()
            .into_iter()
            .map(|n| n.id.to_string())
            .collect::<Vec<_>>()
    };
    assert_eq!(ids("gradient"), ["n1", "n2"]);
    assert_eq!(ids("grad"), ["n1", "n2"]);
    assert_eq!(ids("fel\""), ["n3"]);
    assert!(ids("   ").is_empty());
    // link-append of an existing id replaces the index row instead of duplicating it
    s.append_node(&node("n3", "Cats", "felines purr and nap"))
        .unwrap();
    assert_eq!(ids("felines").len(), 1);
    // reopening rebuilds the index from the ledger, including after a lost index
    drop(s);
    std::fs::remove_file(dir.path().join("graph.db")).unwrap();
    let s = FsGraphStore::open(dir.path()).unwrap();
    assert_eq!(s.search_propositions("nap", 10).unwrap().len(), 1);
    assert_eq!(s.nodes().unwrap().len(), 4);
}

#[test]
fn delete_nodes_rewrites_ledger_and_search_index() {
    let dir = tempfile::tempdir().unwrap();
    let s = FsGraphStore::open(dir.path()).unwrap();
    s.append_node(&node("n_a", "Nap", "naps help")).unwrap();
    s.append_node(&node("n_b", "Sleep", "sleep helps")).unwrap();
    s.delete_nodes(&["n_a".into()]).unwrap();
    let ids: Vec<_> = s.nodes().unwrap().into_iter().map(|n| n.id).collect();
    assert_eq!(ids, ["n_b"]);
    assert!(s.search_propositions("nap", 10).unwrap().is_empty());
    assert_eq!(s.search_propositions("sleep", 10).unwrap().len(), 1);
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
    s.upsert_session(&session(
        "s1",
        "refactor the parser. the parser handles tokens; parser parser",
    ))
    .unwrap();
    s.upsert_session(&session("s2", "one mention of parser in a long unrelated discussion about databases and indexes and many other things")).unwrap();
    s.upsert_session(&session("s3", "nothing relevant"))
        .unwrap();
    assert_eq!(s.search_transcripts("pars", 5).unwrap(), ["s1", "s2"]);
    s.upsert_session(&session("s1", "rewritten")).unwrap();
    assert_eq!(s.search_transcripts("parser", 5).unwrap(), ["s2"]);
}

#[test]
fn session_lookup_returns_the_stored_session() {
    let dir = tempfile::tempdir().unwrap();
    let s = SqliteTrackerStore::open(dir.path()).unwrap();
    s.upsert_session(&session("s1", "t")).unwrap();
    assert_eq!(s.session("s1").unwrap(), Some(session("s1", "t")));
    assert_eq!(s.session("nope").unwrap(), None);
}

#[test]
fn tracker_tables_round_trip() {
    let dir = tempfile::tempdir().unwrap();
    let s = SqliteTrackerStore::open(dir.path()).unwrap();
    assert_eq!(s.cursor("k").unwrap(), None);
    let mut cur = IngestCursor {
        source_key: "k".into(),
        mtime_ms: 1,
        size_bytes: 2,
    };
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
        alternatives: vec![Alternative {
            option: "o".into(),
            rejected_because: "r".into(),
        }],
        risk: "low".into(),
        files: vec!["a.rs".into()],
        commit_sha: None,
    };
    s.upsert_decision(&d).unwrap();
    d.risk = "high".into();
    s.upsert_decision(&d).unwrap();
    assert_eq!(s.decisions().unwrap(), vec![d]);

    s.upsert_book_chunk(&BookChunk {
        id: "b".into(),
        source: "s".into(),
        text: "t".into(),
    })
    .unwrap();
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
    let r = FsrsReviewLog {
        card_id: "f1".into(),
        ts: 5,
        rating: Rating::Good,
        device_id: "d".into(),
    };
    s.append_review(&r).unwrap();
    s.append_review(&r).unwrap();
    assert_eq!(s.reviews().unwrap(), vec![r]);
}

#[test]
fn sync_files_export_and_import_idempotently() {
    let dir = tempfile::tempdir().unwrap();
    let a = SqliteTrackerStore::open(dir.path().join("a")).unwrap();
    let r = FsrsReviewLog {
        card_id: "f1".into(),
        ts: 5,
        rating: Rating::Easy,
        device_id: "phone".into(),
    };
    let mut line = serde_json::to_value(&r).unwrap();
    line["kind"] = "review".into();
    a.append_sync("phone", &line).unwrap();

    // another device's folder syncs the file in
    let b = SqliteTrackerStore::open(dir.path().join("b")).unwrap();
    std::fs::copy(
        dir.path().join("a/sync-phone.jsonl"),
        dir.path().join("b/sync-phone.jsonl"),
    )
    .unwrap();
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
            let r = FsrsReviewLog {
                card_id: "c".into(),
                ts,
                rating: Rating::Good,
                device_id: "d".into(),
            };
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

fn fcard(id: &str) -> FlashCard {
    FlashCard {
        id: id.into(),
        front: "q".into(),
        options: ["a".into(), "b".into(), "c".into(), "d".into()],
        correct_index: 0,
        rationale: String::new(),
        card: Card::new(0),
    }
}

#[test]
fn card_by_id_does_not_parse_other_cards() {
    let dir = tempfile::tempdir().unwrap();
    let s = SqliteTrackerStore::open(dir.path()).unwrap();
    s.upsert_card(&fcard("c1")).unwrap();
    s.upsert_card(&fcard("c2")).unwrap();
    rusqlite::Connection::open(dir.path().join("tracker.db"))
        .unwrap()
        .execute("UPDATE cards SET json = 'garbage' WHERE id = 'c2'", [])
        .unwrap();
    assert!(s.cards().is_err());
    assert_eq!(s.card("c1").unwrap().unwrap(), fcard("c1"));
    assert!(s.card("nope").unwrap().is_none());
}

#[test]
fn corrupt_interior_line_neither_hides_nor_deletes_later_records() {
    let dir = tempfile::tempdir().unwrap();
    let s = FsGraphStore::open(dir.path()).unwrap();
    for id in ["a1", "a2", "a3"] {
        s.append_chain(&chain(id)).unwrap();
    }
    let path = dir.path().join("chains.jsonl");
    let text = std::fs::read_to_string(&path).unwrap();
    let mut lines: Vec<_> = text.lines().collect();
    lines[1] = "deadbeef\t{\"bit\":\"flip\"}";
    std::fs::write(&path, lines.join("\n") + "\n").unwrap();
    s.append_chain(&chain("a4")).unwrap();
    assert_eq!(
        s.chains().unwrap(),
        vec![chain("a1"), chain("a3"), chain("a4")]
    );
}

#[test]
fn concurrent_appends_from_independent_handles_lose_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("l.jsonl");
    let threads: Vec<_> = (0..8)
        .map(|t| {
            let path = path.clone();
            std::thread::spawn(move || {
                for i in 0..20 {
                    crate::jsonl::append(&path, &format!("{t}-{i}")).unwrap();
                }
            })
        })
        .collect();
    threads.into_iter().for_each(|t| t.join().unwrap());
    assert_eq!(crate::jsonl::read_all::<String>(&path).unwrap().len(), 160);
}

#[test]
fn index_holds_the_merged_node_not_the_last_link_line() {
    let dir = tempfile::tempdir().unwrap();
    let s = FsGraphStore::open(dir.path()).unwrap();
    s.append_node(&node("n1", "Original", "original wording"))
        .unwrap();
    s.append_node(&node("n1", "Paraphrase", "reworded claim"))
        .unwrap();
    let check = |s: &FsGraphStore| {
        let hit = s.search_propositions("original", 10).unwrap();
        assert_eq!(hit.len(), 1);
        assert_eq!(hit[0].statement, "original wording");
        assert_eq!(hit[0].aliases, ["reworded claim"]);
        assert_eq!(s.search_propositions("reworded", 10).unwrap().len(), 1);
    };
    check(&s);
    drop(s);
    check(&FsGraphStore::open(dir.path()).unwrap());
}

#[test]
fn store_files_are_owner_only() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("lp");
    let g = FsGraphStore::open(root.join("graph")).unwrap();
    g.append_chain(&chain("a1")).unwrap();
    FsChatStore::open(root.join("chats"))
        .unwrap()
        .save_chat(&chat("c1", 1))
        .unwrap();
    SqliteTrackerStore::open(root.join("tracker")).unwrap();
    let mode = |p: std::path::PathBuf| std::fs::metadata(p).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode(root.clone()), 0o700);
    assert_eq!(mode(root.join("graph/chains.jsonl")), 0o600);
    assert_eq!(mode(root.join("chats/c1.json")), 0o600);
    assert_eq!(mode(root.join("tracker/tracker.db")), 0o600);
}

#[test]
fn one_corrupt_chat_or_job_file_is_skipped() {
    let dir = tempfile::tempdir().unwrap();
    let chats = FsChatStore::open(dir.path().join("chats")).unwrap();
    chats.save_chat(&chat("c1", 1)).unwrap();
    std::fs::write(dir.path().join("chats/bad.json"), b"{nope").unwrap();
    assert_eq!(chats.list_chats().unwrap().len(), 1);
    let jobs = FsJobStore::open(dir.path().join("jobs")).unwrap();
    jobs.create_job(&job("j1", JobStatus::Running)).unwrap();
    std::fs::write(dir.path().join("jobs/bad.json"), b"{nope").unwrap();
    assert_eq!(jobs.interrupted_jobs().unwrap().len(), 1);
}

#[test]
fn new_ledger_handles_wait_on_a_locked_database_instead_of_failing() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("x.db");
    let holder = rusqlite::Connection::open(&path).unwrap();
    holder.execute_batch("CREATE TABLE t(a)").unwrap();
    holder.execute_batch("BEGIN EXCLUSIVE").unwrap();
    let t = std::thread::spawn(move || {
        std::thread::sleep(std::time::Duration::from_millis(300));
        holder.execute_batch("COMMIT").unwrap();
    });
    crate::db::open(&path, &[]).unwrap();
    t.join().unwrap();
}

#[test]
fn commit_review_is_all_or_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let s = SqliteTrackerStore::open(dir.path()).unwrap();
    let mut c = fcard("f1");
    s.upsert_card(&c).unwrap();
    rusqlite::Connection::open(dir.path().join("tracker.db"))
        .unwrap()
        .execute_batch("DROP TABLE fsrs_reviews")
        .unwrap();
    c.rationale = "changed".into();
    let log = FsrsReviewLog {
        card_id: "f1".into(),
        ts: 1,
        rating: Rating::Good,
        device_id: "d".into(),
    };
    assert!(s.commit_review(&c, &log).is_err());
    assert_eq!(s.cards().unwrap()[0].rationale, "");
}

#[test]
fn unreadable_chat_and_job_files_are_reported_not_silent() {
    let dir = tempfile::tempdir().unwrap();
    let chats = FsChatStore::open(dir.path().join("chats")).unwrap();
    chats.save_chat(&chat("c1", 1)).unwrap();
    std::fs::write(dir.path().join("chats/bad.json"), b"{").unwrap();
    let (list, skipped) = chats.list_chats_report().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(skipped.len(), 1);
    assert!(skipped[0].ends_with("bad.json"));

    let jobs = FsJobStore::open(dir.path().join("jobs")).unwrap();
    std::fs::write(dir.path().join("jobs/bad.json"), b"{").unwrap();
    let (list, skipped) = jobs.all_report().unwrap();
    assert!(list.is_empty());
    assert!(skipped[0].ends_with("bad.json"));
}

mod course_store {
    use super::*;
    use lp_core::course::{Course, CourseEvent};
    use std::os::unix::fs::PermissionsExt;

    fn course(id: &str, learner: &str) -> Course {
        Course {
            id: id.into(),
            learner: learner.into(),
            lessons: vec![],
            must_fix: vec![],
            version: 1,
        }
    }

    fn ev(n: u32) -> CourseEvent {
        CourseEvent::Shown {
            ts: i64::from(n),
            lesson: format!("l{n}"),
        }
    }

    #[test]
    fn round_trip_list_delete_learner() {
        let d = tempfile::tempdir().unwrap();
        let s = FsCourseStore::new(d.path());
        assert!(s.list().unwrap().is_empty());
        assert_eq!(s.load_learner().unwrap(), "");
        s.save_course(&course("c1", "\n  learn rust\nmore"))
            .unwrap();
        assert_eq!(s.load_course("c1").unwrap().learner, "\n  learn rust\nmore");
        s.append_event("c1", &ev(1)).unwrap();
        assert_eq!(s.events("c1").unwrap(), vec![ev(1)]);
        std::fs::create_dir_all(d.path().join("courses/broken")).unwrap();
        let l = s.list().unwrap();
        assert_eq!(
            (l.len(), l[0].title.as_str(), l[0].version),
            (1, "learn rust", 1)
        );
        assert!(l[0].updated_at > 0);
        s.save_learner("likes diagrams").unwrap();
        assert_eq!(s.load_learner().unwrap(), "likes diagrams");
        s.delete_course("c1").unwrap();
        assert!(matches!(s.load_course("c1"), Err(StoreError::NotFound(_))));
        s.delete_course("c1").unwrap();
    }

    #[test]
    fn torn_tail_then_append() {
        let d = tempfile::tempdir().unwrap();
        let s = FsCourseStore::new(d.path());
        s.append_event("c1", &ev(1)).unwrap();
        let p = d.path().join("courses/c1/events.jsonl");
        let mut f = std::fs::OpenOptions::new().append(true).open(&p).unwrap();
        f.write_all(b"deadbeef\t{\"kind\":\"sho").unwrap();
        assert_eq!(s.events("c1").unwrap(), vec![ev(1)]);
        s.append_event("c1", &ev(2)).unwrap();
        assert_eq!(s.events("c1").unwrap(), vec![ev(1), ev(2)]);
    }

    #[test]
    fn concurrent_appends_from_independent_handles() {
        let d = tempfile::tempdir().unwrap();
        let hs: Vec<_> = (0..8u32)
            .map(|t| {
                let root = d.path().to_path_buf();
                std::thread::spawn(move || {
                    let s = FsCourseStore::new(root);
                    for i in 0..25 {
                        s.append_event("c1", &ev(t * 100 + i)).unwrap();
                    }
                })
            })
            .collect();
        hs.into_iter().for_each(|h| h.join().unwrap());
        assert_eq!(
            FsCourseStore::new(d.path()).events("c1").unwrap().len(),
            200
        );
    }

    #[test]
    fn roots_isolated() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let (sa, sb) = (FsCourseStore::new(a.path()), FsCourseStore::new(b.path()));
        sa.save_course(&course("c1", "x")).unwrap();
        sa.save_learner("me").unwrap();
        sa.append_event("c1", &ev(1)).unwrap();
        assert!(sb.list().unwrap().is_empty());
        assert!(sb.events("c1").unwrap().is_empty());
        assert_eq!(sb.load_learner().unwrap(), "");
    }

    #[test]
    fn bad_ids_rejected() {
        let d = tempfile::tempdir().unwrap();
        let s = FsCourseStore::new(d.path());
        for id in ["", "..", ".", "../x", "a/b", "A", "a.b", "a b", "a\\b"] {
            assert!(s.save_course(&course(id, "x")).is_err(), "{id:?}");
            assert!(s.load_course(id).is_err());
            assert!(s.append_event(id, &ev(1)).is_err());
            assert!(s.events(id).is_err());
            assert!(s.delete_course(id).is_err());
        }
        assert!(s.save_course(&course("ok_1-x", "x")).is_ok());
    }

    #[test]
    fn owner_only_permissions() {
        let d = tempfile::tempdir().unwrap();
        let s = FsCourseStore::new(d.path());
        s.save_course(&course("c1", "x")).unwrap();
        s.append_event("c1", &ev(1)).unwrap();
        s.save_learner("me").unwrap();
        let mode = |p: &str| {
            std::fs::metadata(d.path().join(p))
                .unwrap()
                .permissions()
                .mode()
                & 0o777
        };
        assert_eq!(mode("courses/c1/course.json"), 0o600);
        assert_eq!(mode("courses/c1/events.jsonl"), 0o600);
        assert_eq!(mode("learner.json"), 0o600);
        assert_eq!(mode("courses/c1"), 0o700);
        assert_eq!(mode("courses"), 0o700);
    }

    #[test]
    fn large_event_count_read() {
        let d = tempfile::tempdir().unwrap();
        let s = FsCourseStore::new(d.path());
        for i in 0..3000 {
            s.append_event("c1", &ev(i)).unwrap();
        }
        assert_eq!(s.events("c1").unwrap().len(), 3000);
    }
#[test]
fn second_based_timestamps_read_back_as_milliseconds() {
    let dir = tempfile::tempdir().unwrap();
    let s = FsChatStore::open(dir.path()).unwrap();
    let mut old = chat("old", 0);
    old.created_at = 1_793_131_000;
    old.updated_at = 1_793_131_171;
    s.save_chat(&old).unwrap();
    let mut new = chat("new", 0);
    new.updated_at = 1_793_131_171_000;
    s.save_chat(&new).unwrap();
    let loaded = s.load_chat("old").unwrap();
    assert_eq!(
        (loaded.created_at, loaded.updated_at),
        (1_793_131_000_000, 1_793_131_171_000)
    );
    let listed = s.list_chats().unwrap();
    assert!(listed.iter().all(|c| c.updated_at == 1_793_131_171_000));
    assert_eq!(epoch_ms(0), 0);
    let ws = tempfile::tempdir().unwrap();
    let jobs = FsJobStore::open(ws.path().join("jobs")).unwrap();
    let mut j = job("j1", JobStatus::Done);
    j.created_at = 1_793_131_171;
    jobs.create_job(&j).unwrap();
    assert_eq!(
        jobs.all_report().unwrap().0[0].created_at,
        1_793_131_171_000
    );
}

#[test]
fn migrated_job_artifact_path_resolves_inside_the_workspace_only() {
    let ws = tempfile::tempdir().unwrap();
    let arts = ws.path().join("artifacts");
    std::fs::create_dir_all(&arts).unwrap();
    std::fs::write(arts.join("job_1.svg"), "<svg/>").unwrap();
    let jobs = FsJobStore::open(ws.path().join("jobs")).unwrap();
    let mut j = job("j1", JobStatus::Done);
    j.result = Some(serde_json::json!({"artifact": "/old/home/.learnpuppy/artifacts/job_1.svg"}));
    jobs.create_job(&j).unwrap();
    let mut other = job("j2", JobStatus::Done);
    other.result =
        Some(serde_json::json!({"artifact": "/old/home/.learnpuppy/artifacts/gone.svg"}));
    jobs.create_job(&other).unwrap();
    let mut esc = job("j3", JobStatus::Done);
    esc.result = Some(serde_json::json!({"artifact": "/old/../../etc/job_1.svg/.."}));
    jobs.create_job(&esc).unwrap();
    let got = jobs.all_report().unwrap().0;
    let art = |id: &str| {
        got.iter()
            .find(|j| j.id == id)
            .unwrap()
            .result
            .as_ref()
            .unwrap()["artifact"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    assert_eq!(art("j1"), arts.join("job_1.svg").display().to_string());
    assert_eq!(art("j2"), "/old/home/.learnpuppy/artifacts/gone.svg");
    assert!(!art("j3").starts_with(ws.path().to_str().unwrap()));
}
