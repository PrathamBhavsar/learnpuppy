//! End-to-end tests of every docs/web/02 operation against real stores in temp dirs.

use std::os::unix::fs::MetadataExt;
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use lp_core::fsrs::Rating;
use lp_core::graph::{GraphChain, GraphNode, Provenance};
use lp_core::ports::{
    ChatRecord, ChatStore, CodingSession, DeliveryStatus, GraphStore, IngestCursor, JobKind,
    JobRecord, JobStatus, JobStore, StoreError, TrackerStore,
};
use parking_lot::Mutex;

use super::export::store_counts;
use super::lock::ServerLock;
use super::meta::SCHEMA;
use super::migrate::{self, MarkerState, MigrationOutcome, MARKER};
use super::*;
use crate::paths;
use crate::review::tests::card;
use crate::settings::Settings;
use crate::stores::Stores;

// ---------- fixtures ----------

fn node(id: &str) -> GraphNode {
    GraphNode {
        id: id.into(),
        name: format!("{id} name"),
        statement: format!("{id} statement about ownership"),
        aliases: vec![],
        created_at: chrono::DateTime::from_timestamp_millis(1_761_700_000_000).unwrap(),
        provenance: vec![Provenance {
            chat_id: "c_1a17ef24da3".into(),
            message_id: "m1".into(),
        }],
    }
}

fn chain(id: &str, premises: &[&str], conclusion: &str) -> GraphChain {
    GraphChain {
        id: id.into(),
        premises: premises.iter().map(|p| (*p).into()).collect(),
        conclusion: conclusion.into(),
        provenance: vec![],
        created_at: chrono::DateTime::from_timestamp_millis(1_761_700_000_000).unwrap(),
    }
}

fn chat(id: &str, updated_at: i64) -> ChatRecord {
    ChatRecord {
        id: id.into(),
        title: format!("chat {id}"),
        created_at: updated_at,
        updated_at,
        body: serde_json::json!({ "messages": [{ "role": "user", "text": "hi" }] }),
        turns: vec![],
        extractions: vec![],
    }
}

/// Fill a store root with a bit of everything: nodes, chains, chats, a job, cards, a graded
/// review (so a sync log), an ingest session and cursor.
fn populate(root: &Path, device: &str) {
    let s = Stores::open(root).unwrap();
    for n in ["n_own", "n_borrow", "n_move"] {
        s.graph.append_node(&node(n)).unwrap();
    }
    s.graph
        .append_chain(&chain("ch_1", &["n_own", "n_borrow"], "n_move"))
        .unwrap();
    s.chats
        .save_chat(&chat("c_1a17ef24da3", 1_761_700_000_000))
        .unwrap();
    s.chats
        .save_chat(&chat("c_1a17f0a7459", 1_761_700_100_000))
        .unwrap();
    s.jobs
        .create_job(&JobRecord {
            id: "job_0efa1274f8b9".into(),
            chat_id: "c_1a17ef24da3".into(),
            kind: JobKind::Visual,
            status: JobStatus::Done,
            delivery: DeliveryStatus::Claimed,
            created_at: 1_761_700_000_000,
            result: None,
            usage: vec![],
        })
        .unwrap();
    s.tracker.upsert_card(&card("card_1", 0)).unwrap();
    s.tracker.upsert_card(&card("card_2", i64::MAX)).unwrap();
    crate::review::grade(
        &*s.tracker,
        device,
        "card_1",
        Rating::Good,
        1_761_700_000_000,
    )
    .unwrap();
    s.tracker
        .upsert_session(&CodingSession {
            id: "sess_1".into(),
            source: "claude".into(),
            cwd: "/Users/me/code/foo".into(),
            git_branch: Some("main".into()),
            started_at: 1_761_700_000_000,
            transcript: "we chose tokio for the borrow checker friendly runtime".into(),
        })
        .unwrap();
    s.tracker
        .set_cursor(&IngestCursor {
            source_key: "claude:/x.jsonl".into(),
            mtime_ms: 5,
            size_bytes: 10,
        })
        .unwrap();
}

/// The real pre-workspaces `~/.learnpuppy` shape: stores straight at the root, plus global files.
fn legacy_fixture(root: &Path) {
    Settings {
        device_id: "devfixture".into(),
        ..Settings::default()
    }
    .save(root)
    .unwrap();
    std::fs::create_dir_all(root.join("logs")).unwrap();
    std::fs::write(root.join("logs/learnpuppy.log.2026-10-27"), "log\n").unwrap();
    populate(root, "devfixture");
    std::fs::create_dir_all(root.join("artifacts")).unwrap();
    std::fs::write(root.join("artifacts/job_0efa1274f8b9.svg"), "<svg/>").unwrap();
}

/// Visible files under `dir`, relative and sorted (sidecar locks and SQLite WAL files skipped).
fn tree(dir: &Path) -> Vec<String> {
    fn walk(base: &Path, dir: &Path, out: &mut Vec<String>) {
        for e in std::fs::read_dir(dir).unwrap().flatten() {
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || name.ends_with("-wal") || name.ends_with("-shm") {
                continue;
            }
            if e.file_type().unwrap().is_dir() {
                walk(base, &e.path(), out);
            } else {
                out.push(e.path().strip_prefix(base).unwrap().display().to_string());
            }
        }
    }
    let mut out = vec![];
    walk(dir, dir, &mut out);
    out.sort();
    out
}

#[derive(Default)]
struct Recorder(Mutex<Vec<WorkspaceEvent>>);

impl EventSink for Recorder {
    fn emit(&self, e: WorkspaceEvent) {
        self.0.lock().push(e);
    }
}

impl Recorder {
    fn kinds(&self) -> Vec<String> {
        self.0.lock().iter().map(|e| e.kind.clone()).collect()
    }
}

fn registry(root: &Path) -> (WorkspaceRegistry, Arc<Recorder>) {
    let rec = Arc::new(Recorder::default());
    (WorkspaceRegistry::new(root, rec.clone()).unwrap(), rec)
}

fn named(name: &str) -> NewWorkspace {
    NewWorkspace {
        name: name.into(),
        ..NewWorkspace::default()
    }
}

// ---------- migration ----------

const GOLDEN_LEGACY: [&str; 11] = [
    "artifacts/job_0efa1274f8b9.svg",
    "chats/c_1a17ef24da3.json",
    "chats/c_1a17f0a7459.json",
    "graph/chains.jsonl",
    "graph/graph.db",
    "graph/nodes.jsonl",
    "jobs/job_0efa1274f8b9.json",
    "logs/learnpuppy.log.2026-10-27",
    "settings.json",
    "tracker/sync-devfixture.jsonl",
    "tracker/tracker.db",
];

#[test]
fn migration_moves_the_golden_legacy_layout_into_default() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    legacy_fixture(root);
    assert_eq!(tree(root), GOLDEN_LEGACY);
    let before = store_counts(root, &Stores::open(root).unwrap()).unwrap();
    let inode = std::fs::metadata(root.join("graph/nodes.jsonl"))
        .unwrap()
        .ino();
    assert!(migrate::needed(root).unwrap());

    let lock = ServerLock::acquire(root, None, None).unwrap();
    let MigrationOutcome::Migrated {
        workspace_id: id,
        moved,
        resumed,
    } = migrate::migrate(root, &lock).unwrap()
    else {
        panic!("expected a migration");
    };
    assert!(!resumed);
    assert_eq!(moved, ["graph", "chats", "jobs", "artifacts", "tracker"]);

    // Global files stay; store dirs moved by rename (same inode: no copy, no extra disk).
    let ws = paths::workspace_dir(root, &id);
    for gone in ["graph", "chats", "jobs", "artifacts", "tracker"] {
        assert!(!root.join(gone).exists(), "{gone}");
    }
    assert!(root.join("settings.json").is_file() && root.join("logs").is_dir());
    assert_eq!(
        std::fs::metadata(ws.join("graph/nodes.jsonl"))
            .unwrap()
            .ino(),
        inode
    );
    let mut expected: Vec<String> = GOLDEN_LEGACY
        .iter()
        .filter(|p| !p.starts_with("logs/") && **p != "settings.json")
        .map(|p| (*p).to_owned())
        .chain(["workspace.json".to_owned()])
        .collect();
    expected.sort();
    assert_eq!(tree(&ws), expected);

    let (reg, _) = registry(root);
    let m = reg.get(&id).unwrap();
    assert_eq!((m.name.as_str(), m.slug.as_str()), ("Default", "default"));
    let h = reg.open(&id).unwrap();
    assert_eq!(store_counts(&ws, h.stores()).unwrap(), before);
    assert_eq!(
        State::load_or_init(root).unwrap().last_workspace,
        Some(id.clone())
    );
    assert_eq!(State::load_or_init(root).unwrap().device_id, "devfixture");

    let marker = migrate::marker(root).unwrap().unwrap();
    assert_eq!(marker.state, MarkerState::Done);
    assert_eq!(marker.moves.len(), 5);
    assert_eq!(marker.moves[0].to, format!("workspaces/{id}/graph"));
    // Idempotent.
    assert!(!migrate::needed(root).unwrap());
    assert_eq!(
        migrate::migrate(root, &lock).unwrap(),
        MigrationOutcome::AlreadyMigrated { workspace_id: id }
    );
}

#[test]
fn fresh_installs_and_workspace_roots_need_no_migration() {
    let d = tempfile::tempdir().unwrap();
    let lock = ServerLock::acquire(d.path(), None, None).unwrap();
    assert_eq!(
        migrate::migrate(d.path(), &lock).unwrap(),
        MigrationOutcome::NotNeeded
    );
    let (reg, _) = registry(d.path());
    reg.create(named("A")).unwrap();
    // A stray legacy-looking dir next to real workspaces is not migrated.
    std::fs::create_dir(d.path().join("chats")).unwrap();
    assert!(!migrate::needed(d.path()).unwrap());
    assert!(!d.path().join(MARKER).exists());
}

#[test]
fn a_migration_killed_midway_resumes_without_loss() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    legacy_fixture(root);
    let before = store_counts(root, &Stores::open(root).unwrap()).unwrap();

    // Crash right before the third move (`jobs`).
    let mut n = 0;
    let err = migrate::migrate_with(root, &mut |_| {
        n += 1;
        if n == 3 {
            Err(WorkspaceError::Internal("killed".into()))
        } else {
            Ok(())
        }
    })
    .unwrap_err();
    assert_eq!(err.to_string(), "killed");
    let m = migrate::marker(root).unwrap().unwrap();
    assert_eq!(m.state, MarkerState::InProgress);
    assert_eq!(m.done, ["graph", "chats"]);
    assert!(root.join("jobs").exists() && !root.join("graph").exists());
    assert!(migrate::needed(root).unwrap());

    // Also simulate a crash between a rename and its marker update: `jobs` already moved.
    let id = m.workspace_id.clone();
    std::fs::rename(
        root.join("jobs"),
        paths::workspace_dir(root, &id).join("jobs"),
    )
    .unwrap();

    let lock = ServerLock::acquire(root, None, None).unwrap();
    match migrate::migrate(root, &lock).unwrap() {
        MigrationOutcome::Migrated {
            workspace_id,
            moved,
            resumed,
        } => {
            assert!(resumed);
            assert_eq!(workspace_id, id);
            assert_eq!(moved, ["graph", "chats", "jobs", "artifacts", "tracker"]);
        }
        other => panic!("{other:?}"),
    }
    let (reg, _) = registry(root);
    let list = reg.list(false).unwrap();
    assert_eq!(list.workspaces.len(), 1);
    assert!(list.damaged.is_empty());
    let h = reg.open(&id).unwrap();
    assert_eq!(store_counts(h.root(), h.stores()).unwrap(), before);
}

#[test]
fn revert_restores_the_legacy_layout_including_new_data() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    legacy_fixture(root);
    let lock = ServerLock::acquire(root, None, None).unwrap();
    let MigrationOutcome::Migrated { workspace_id, .. } = migrate::migrate(root, &lock).unwrap()
    else {
        panic!()
    };
    {
        // Use the migrated workspace: new data must survive the revert.
        let (reg, _) = registry(root);
        let h = reg.open(&workspace_id).unwrap();
        h.stores().graph.append_node(&node("n_new")).unwrap();
    }
    let report = migrate::revert(root, &lock).unwrap();
    assert_eq!(report.workspace_id, workspace_id);
    assert_eq!(
        report.restored,
        ["graph", "chats", "jobs", "artifacts", "tracker"]
    );
    assert_eq!(report.left_behind, None);
    let mut expected = [&GOLDEN_LEGACY[..], &["server.lock", "state.json"]].concat();
    expected.sort_unstable();
    assert_eq!(tree(root), expected);
    assert!(!root.join("workspaces").exists() && !root.join(MARKER).exists());
    assert_eq!(Stores::open(root).unwrap().graph.nodes().unwrap().len(), 4);
    assert_eq!(State::load_or_init(root).unwrap().last_workspace, None);
    // And it can migrate again.
    assert!(migrate::needed(root).unwrap());
    assert!(matches!(
        migrate::migrate(root, &lock).unwrap(),
        MigrationOutcome::Migrated { .. }
    ));
    assert!(migrate::revert(root, &lock).is_ok());
    assert_eq!(
        migrate::revert(root, &lock).unwrap_err().code(),
        "not_found"
    );
}

#[test]
fn migration_refuses_while_a_legacy_process_holds_the_store_open() {
    if std::process::Command::new("lsof")
        .arg("-v")
        .output()
        .is_err()
    {
        eprintln!("lsof not installed; skipping");
        return;
    }
    let d = tempfile::tempdir().unwrap();
    let root = d.path();
    legacy_fixture(root);
    // Stand-in for a running legacy TUI: a child process with tracker.db open.
    let mut child = std::process::Command::new("sleep")
        .arg("30")
        .stdin(std::fs::File::open(root.join("tracker/tracker.db")).unwrap())
        .spawn()
        .unwrap();
    let lock = ServerLock::acquire(root, None, None).unwrap();
    let res = migrate::migrate(root, &lock);
    child.kill().unwrap();
    child.wait().unwrap();
    let err = res.unwrap_err();
    assert_eq!(err.code(), "conflict");
    assert!(err.to_string().contains(&child.id().to_string()), "{err}");
    assert!(root.join("graph").is_dir() && !root.join(MARKER).exists());
    assert!(matches!(
        migrate::migrate(root, &lock).unwrap(),
        MigrationOutcome::Migrated { .. }
    ));
}

/// Manual check against a copy of a real data root:
/// `LP_REAL_COPY=/tmp/lp-copy cargo test -p lp-service real_copy -- --ignored --nocapture`.
#[test]
#[ignore = "needs LP_REAL_COPY pointing at a disposable copy of ~/.learnpuppy"]
fn real_copy_migrates_with_equal_counts_and_reverts() {
    let Some(root) = std::env::var_os("LP_REAL_COPY").map(std::path::PathBuf::from) else {
        panic!("set LP_REAL_COPY");
    };
    let before_tree = tree(&root);
    let before = store_counts(&root, &Stores::open(&root).unwrap()).unwrap();
    let lock = ServerLock::acquire(&root, None, None).unwrap();
    let MigrationOutcome::Migrated {
        workspace_id,
        moved,
        ..
    } = migrate::migrate(&root, &lock).unwrap()
    else {
        panic!("nothing migrated")
    };
    let (reg, _) = registry(&root);
    let h = reg.open(&workspace_id).unwrap();
    let after = store_counts(h.root(), h.stores()).unwrap();
    println!("moved {moved:?}\nbefore {before:?}\nafter  {after:?}");
    assert_eq!(before, after);
    drop(h);
    drop(reg);
    let report = migrate::revert(&root, &lock).unwrap();
    println!("reverted {:?}", report.restored);
    let after_tree: Vec<String> = tree(&root)
        .into_iter()
        .filter(|p| p != "state.json" && p != "server.lock")
        .collect();
    let before_tree: Vec<String> = before_tree
        .into_iter()
        .filter(|p| p != "state.json" && p != "server.lock")
        .collect();
    assert_eq!(after_tree, before_tree);
}

// ---------- registry ----------

#[test]
fn create_list_edit_and_names_are_unique_case_insensitively() {
    let d = tempfile::tempdir().unwrap();
    let (reg, rec) = registry(d.path());
    assert!(reg.list(false).unwrap().workspaces.is_empty());
    let a = reg
        .create(NewWorkspace {
            name: " Rust async ".into(),
            icon: Some("rocket".into()),
            hue: Some("teal".into()),
            description: Some("tokio".into()),
            sources: Some(Sources {
                allow: vec!["/Users/me/code/foo".into()],
                ..Sources::default()
            }),
        })
        .unwrap();
    let m = a.meta();
    assert!(meta::is_valid_id(&m.id));
    assert_eq!(
        (
            m.name.as_str(),
            m.slug.as_str(),
            m.icon.as_str(),
            m.hue.as_str()
        ),
        ("Rust async", "rust-async", "rocket", "teal")
    );
    assert_eq!(m.schema, SCHEMA);
    assert!(a.root().join("graph").is_dir() && a.root().join("artifacts").is_dir());

    for dup in ["rust ASYNC", "  Rust async"] {
        let e = reg.create(named(dup)).unwrap_err();
        assert!(matches!(e, WorkspaceError::NameTaken(_)), "{e}");
        assert_eq!(e.code(), "conflict");
    }
    assert_eq!(reg.create(named("")).unwrap_err().code(), "validation");
    assert_eq!(
        reg.create(NewWorkspace {
            icon: Some("🚀".into()),
            ..named("Emoji")
        })
        .unwrap_err()
        .code(),
        "validation"
    );
    // A failed create leaves nothing behind.
    assert_eq!(
        std::fs::read_dir(paths::workspaces_dir(d.path()))
            .unwrap()
            .count(),
        1
    );

    let b = reg.create(named("Distributed systems")).unwrap();
    let b_id = b.id().to_owned();
    assert_eq!(
        reg.update(
            &b_id,
            WorkspacePatch {
                name: Some("RUST async".into()),
                ..WorkspacePatch::default()
            }
        )
        .unwrap_err()
        .code(),
        "conflict"
    );
    // Renaming to a different case of its own name is fine.
    let m = reg
        .update(
            &b_id,
            WorkspacePatch {
                name: Some("distributed Systems".into()),
                hue: Some("rose".into()),
                ..WorkspacePatch::default()
            },
        )
        .unwrap();
    assert_eq!(
        (m.name.as_str(), m.hue.as_str()),
        ("distributed Systems", "rose")
    );
    assert_eq!(b.meta().hue, "rose", "open handle sees the edit");
    assert_eq!(reg.get(&b_id).unwrap(), m);

    reg.touch(a.id()).unwrap();
    let list = reg.list(false).unwrap();
    assert_eq!(list.workspaces.len(), 2);
    assert_eq!(
        list.workspaces[0].meta.id,
        a.id(),
        "most recently opened first"
    );
    assert_eq!(
        State::load_or_init(d.path())
            .unwrap()
            .last_workspace
            .as_deref(),
        Some(a.id())
    );
    assert_eq!(
        rec.kinds(),
        [
            "workspace.created",
            "workspace.created",
            "workspace.updated"
        ]
    );
    assert_eq!(reg.get("ws_nope").unwrap_err().code(), "not_found");
    assert_eq!(reg.get("../etc").unwrap_err().code(), "not_found");
}

#[test]
fn list_stats_are_counted_and_cached() {
    let d = tempfile::tempdir().unwrap();
    let reg = WorkspaceRegistry::with_config(
        d.path(),
        Arc::new(NoopSink),
        RegistryConfig {
            stats_ttl: Duration::from_secs(3600),
            ..RegistryConfig::default()
        },
    )
    .unwrap();
    let h = reg.create(named("Stats")).unwrap();
    populate(h.root(), "dev1");
    let s = reg.stats(h.id()).unwrap();
    // card_1 was graded in 2025 so it is due again; card_2 never is.
    assert_eq!((s.nodes, s.chains, s.chats, s.due_cards), (3, 1, 2, 1));
    assert!(s.last_activity_at.is_some());
    h.stores().chats.save_chat(&chat("c_more", 1)).unwrap();
    assert_eq!(reg.stats(h.id()).unwrap().chats, 2, "cached");
    reg.invalidate_stats(h.id());
    let list = reg.list(false).unwrap();
    assert_eq!(list.workspaces[0].stats.as_ref().unwrap().chats, 3);
}

#[test]
fn archived_workspaces_are_read_only_and_hidden_by_default() {
    let d = tempfile::tempdir().unwrap();
    let (reg, _) = registry(d.path());
    let h = reg.create(named("Old course")).unwrap();
    let id = h.id().to_owned();
    assert!(h.ensure_writable().is_ok());
    let m = reg.archive(&id).unwrap();
    assert!(m.archived && h.is_archived());
    let e = h.ensure_writable().unwrap_err();
    assert_eq!(e.code(), "workspace_archived");
    let e = reg
        .update(
            &id,
            WorkspacePatch {
                description: Some("x".into()),
                ..WorkspacePatch::default()
            },
        )
        .unwrap_err();
    assert_eq!(e.code(), "workspace_archived");
    assert!(reg.list(false).unwrap().workspaces.is_empty());
    assert_eq!(reg.list(true).unwrap().workspaces.len(), 1);
    // Reopening still works (read-only), and reads are allowed.
    assert!(reg.open(&id).unwrap().is_archived());
    assert!(reg.stats(&id).is_ok());

    // An archived name does not block a new live one; restoring then suffixes.
    reg.create(named("old COURSE")).unwrap();
    let m = reg.restore(&id).unwrap();
    assert!(!m.archived);
    assert_eq!(m.name, "Old course (2)");
    assert!(h.ensure_writable().is_ok());
}

#[test]
fn delete_needs_the_name_cancels_work_and_round_trips_through_trash() {
    let d = tempfile::tempdir().unwrap();
    let (reg, rec) = registry(d.path());
    let h = reg.create(named("Scratch")).unwrap();
    let id = h.id().to_owned();
    populate(h.root(), "dev1");
    reg.touch(&id).unwrap();
    let running = h.cancel_token();

    assert_eq!(reg.delete(&id, "scratch").unwrap_err().code(), "validation");
    assert!(!running.is_cancelled());
    let entry = reg.delete(&id, "Scratch").unwrap();
    assert!(running.is_cancelled(), "running turns/jobs aborted");
    assert_eq!(
        (entry.id.as_str(), entry.name.as_str()),
        (id.as_str(), "Scratch")
    );
    assert!(entry.path.starts_with(paths::trash_dir(d.path())));
    assert!(!paths::workspace_dir(d.path(), &id).exists());
    assert!(reg.list(true).unwrap().workspaces.is_empty());
    assert_eq!(reg.open(&id).err().unwrap().code(), "not_found");
    assert!(reg.open_ids().is_empty());
    assert_eq!(State::load_or_init(d.path()).unwrap().last_workspace, None);
    assert_eq!(reg.trash_list().unwrap(), std::slice::from_ref(&entry));
    assert!(rec.kinds().contains(&"workspace.deleted".to_owned()));

    // Name taken while in the trash ⇒ restored with a suffix, same id, data intact.
    reg.create(named("SCRATCH")).unwrap();
    let m = reg.trash_restore(&entry.key).unwrap();
    assert_eq!(
        (m.id.as_str(), m.name.as_str()),
        (id.as_str(), "Scratch (2)")
    );
    assert!(reg.trash_list().unwrap().is_empty());
    let h = reg.open(&id).unwrap();
    assert_eq!(h.stores().graph.nodes().unwrap().len(), 3);
    assert_eq!(h.stores().chats.list_chats().unwrap().len(), 2);

    // Purge.
    let e1 = reg.delete(&id, "Scratch (2)").unwrap();
    assert_eq!(
        reg.trash_restore("1-ws_nothere").unwrap_err().code(),
        "not_found"
    );
    assert_eq!(
        reg.trash_purge("../../etc").unwrap_err().code(),
        "not_found"
    );
    reg.trash_purge(&e1.key).unwrap();
    assert!(!e1.path.exists() && reg.trash_list().unwrap().is_empty());
    let other = reg.list(false).unwrap().workspaces[0].meta.clone();
    reg.delete(&other.id, &other.name).unwrap();
    assert_eq!(reg.trash_empty().unwrap(), 1);
}

#[test]
fn damaged_workspaces_are_listed_and_repaired() {
    let d = tempfile::tempdir().unwrap();
    let (reg, _) = registry(d.path());
    let h = reg.create(named("Fine")).unwrap();
    populate(h.root(), "dev1");
    let id = h.id().to_owned();
    drop(h);
    reg.close_idle(Instant::now() + Duration::from_secs(3600));
    std::fs::write(
        paths::workspace_dir(d.path(), &id).join("workspace.json"),
        "{ not json",
    )
    .unwrap();
    // A directory with no metadata at all is damaged too.
    let bare = paths::workspace_dir(d.path(), "ws_0000beef");
    std::fs::create_dir_all(&bare).unwrap();

    let list = reg.list(false).unwrap();
    assert!(list.workspaces.is_empty());
    assert_eq!(
        list.damaged
            .iter()
            .map(|w| w.id.as_str())
            .collect::<Vec<_>>(),
        {
            let mut v = vec![id.as_str(), "ws_0000beef"];
            v.sort();
            v
        }
    );
    assert!(list
        .damaged
        .iter()
        .all(|w| w.path.is_dir() && !w.reason.is_empty()));
    assert_eq!(reg.open(&id).err().unwrap().code(), "internal");

    let m = reg.repair(&id).unwrap();
    assert_eq!((m.id.as_str(), m.name.as_str()), (id.as_str(), id.as_str()));
    assert_eq!(
        reg.repair(&id).unwrap_err().code(),
        "conflict",
        "not damaged any more"
    );
    let h = reg.open(&id).unwrap();
    assert_eq!(h.stores().graph.nodes().unwrap().len(), 3, "data kept");
    // A damaged workspace can also be deleted by confirming its id.
    reg.delete("ws_0000beef", "ws_0000beef").unwrap();
    let list = reg.list(false).unwrap();
    assert_eq!((list.workspaces.len(), list.damaged.len()), (1, 0));
}

#[test]
fn handles_open_lazily_close_when_idle_and_respect_the_lru_cap() {
    let d = tempfile::tempdir().unwrap();
    let reg = WorkspaceRegistry::with_config(
        d.path(),
        Arc::new(NoopSink),
        RegistryConfig {
            max_open: 2,
            ..RegistryConfig::default()
        },
    )
    .unwrap();
    let ids: Vec<String> = ["A", "B", "C"]
        .iter()
        .map(|n| reg.create(named(n)).unwrap().id().to_owned())
        .collect();
    // Creating C evicted the least recently used (A).
    assert_eq!(reg.open_ids().len(), 2);
    assert!(!reg.open_ids().contains(&ids[0]));

    // A busy handle is never evicted or idle-closed.
    let b = reg.open(&ids[1]).unwrap();
    let guard = b.busy();
    reg.open(&ids[0]).unwrap();
    assert!(reg.open_ids().contains(&ids[1]) && reg.open_ids().contains(&ids[0]));
    let later = Instant::now() + Duration::from_secs(11 * 60);
    assert_eq!(reg.close_idle(later), [ids[0].clone()]);
    assert_eq!(reg.open_ids(), [ids[1].clone()]);
    drop(guard);
    assert!(reg.close_idle(Instant::now()).is_empty(), "just used");
    assert_eq!(
        reg.close_idle(later + Duration::from_secs(11 * 60)),
        [ids[1].clone()]
    );
    // Reopening yields a working handle again.
    assert!(reg
        .open(&ids[1])
        .unwrap()
        .stores()
        .graph
        .nodes()
        .unwrap()
        .is_empty());
}

#[test]
fn chat_locks_allow_one_turn_per_chat() {
    let d = tempfile::tempdir().unwrap();
    let (reg, rec) = registry(d.path());
    let h = reg.create(named("Locks")).unwrap();
    let l1 = h.chat_lock("c_1");
    let held = l1.clone().try_lock_owned().unwrap();
    assert!(
        h.chat_lock("c_1").try_lock_owned().is_err(),
        "turn_in_progress"
    );
    assert!(h.chat_lock("c_2").try_lock_owned().is_ok());
    drop(held);
    assert!(h.chat_lock("c_1").try_lock_owned().is_ok());
    h.emit("graph.changed", serde_json::json!({}));
    let last = rec.0.lock().last().cloned().unwrap();
    assert_eq!(
        (last.workspace.as_deref(), last.kind.as_str()),
        (Some(h.id()), "graph.changed")
    );
}

// ---------- export / import ----------

#[test]
fn export_then_import_preserves_everything_under_a_new_id() {
    let d = tempfile::tempdir().unwrap();
    let (reg, rec) = registry(d.path());
    let src = reg
        .create(NewWorkspace {
            icon: Some("atom".into()),
            ..named("Rust async")
        })
        .unwrap();
    populate(src.root(), "dev1");
    std::fs::write(src.root().join("artifacts/big.svg"), "<svg/>").unwrap();
    let before = store_counts(src.root(), src.stores()).unwrap();

    let zip = d.path().join("rust-async.zip");
    let report = reg.export(src.id(), &zip).unwrap();
    assert!(report.bytes > 0 && zip.is_file());
    assert_eq!(report.manifest.counts, before);
    assert_eq!(report.manifest.source_id, src.id());
    let paths_in: Vec<&str> = report
        .manifest
        .files
        .iter()
        .map(|f| f.path.as_str())
        .collect();
    assert!(
        paths_in.contains(&"tracker/cards.jsonl") && paths_in.contains(&"tracker/sync-dev1.jsonl")
    );
    assert!(
        paths_in
            .iter()
            .all(|p| !p.ends_with(".db") && !p.starts_with("artifacts") && !p.starts_with("jobs")),
        "{paths_in:?}"
    );

    let imp = reg.import(&zip).unwrap();
    assert_ne!(imp.meta.id, src.id());
    assert_eq!(imp.source_id, src.id());
    assert_eq!(imp.meta.name, "Rust async (2)", "name collision suffix");
    assert_eq!(imp.meta.slug, "rust-async-2");
    assert_eq!(imp.meta.icon, "atom");
    assert_eq!(imp.counts, before);

    // Read back through a fresh handle: rebuilt indexes work.
    let h = reg.open(&imp.meta.id).unwrap();
    assert_eq!(store_counts(h.root(), h.stores()).unwrap(), before);
    assert_eq!(
        h.stores()
            .graph
            .search_propositions("ownership", 10)
            .unwrap()
            .len(),
        3
    );
    assert_eq!(
        h.stores().tracker.search_transcripts("borrow", 5).unwrap(),
        ["sess_1"]
    );
    assert_eq!(
        h.stores()
            .tracker
            .cursor("claude:/x.jsonl")
            .unwrap()
            .map(|c| c.size_bytes),
        Some(10)
    );
    assert_eq!(
        h.stores().tracker.cards().unwrap(),
        src.stores().tracker.cards().unwrap()
    );
    assert_eq!(
        h.stores().tracker.reviews().unwrap(),
        src.stores().tracker.reviews().unwrap()
    );
    assert_eq!(
        h.stores().chats.load_chat("c_1a17ef24da3").unwrap(),
        src.stores().chats.load_chat("c_1a17ef24da3").unwrap()
    );
    assert!(!h.root().join("artifacts/big.svg").exists());
    assert!(!h.root().join(".tracker-rows").exists());
    assert_eq!(reg.list(false).unwrap().workspaces.len(), 2);
    assert_eq!(
        rec.kinds().last().map(String::as_str),
        Some("workspace.created")
    );
    // Nothing staged is left behind.
    assert!(std::fs::read_dir(paths::workspaces_dir(d.path()))
        .unwrap()
        .flatten()
        .all(|e| !e.file_name().to_string_lossy().starts_with('.')));
}

/// Rewrites one archive entry; `None` drops it.
type ZipEdit = dyn Fn(&str, Vec<u8>) -> Option<Vec<u8>>;

fn rewrite_zip(src: &Path, dest: &Path, edit: impl Fn(&str, Vec<u8>) -> Option<Vec<u8>>) {
    use std::io::{Read, Write};
    let mut a = zip::ZipArchive::new(std::fs::File::open(src).unwrap()).unwrap();
    let mut w = zip::ZipWriter::new(std::fs::File::create(dest).unwrap());
    for i in 0..a.len() {
        let mut f = a.by_index(i).unwrap();
        let name = f.name().unwrap().into_owned();
        let mut buf = vec![];
        f.read_to_end(&mut buf).unwrap();
        if let Some(bytes) = edit(&name, buf) {
            w.start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            w.write_all(&bytes).unwrap();
        }
    }
    w.finish().unwrap();
}

#[test]
fn import_rejects_tampered_or_foreign_archives_and_leaves_nothing() {
    let d = tempfile::tempdir().unwrap();
    let (reg, _) = registry(d.path());
    let src = reg.create(named("Src")).unwrap();
    populate(src.root(), "dev1");
    let zip = d.path().join("src.zip");
    reg.export(src.id(), &zip).unwrap();
    let bad = d.path().join("bad.zip");

    let cases: Vec<(&str, Box<ZipEdit>)> = vec![
        (
            "checksum",
            Box::new(|n: &str, mut b: Vec<u8>| {
                if n == "graph/nodes.jsonl" {
                    b[10] ^= 1;
                }
                Some(b)
            }),
        ),
        (
            "missing",
            Box::new(|n: &str, b| (n != "chats/c_1a17ef24da3.json").then_some(b)),
        ),
        (
            "no manifest",
            Box::new(|n: &str, b| (n != "manifest.json").then_some(b)),
        ),
        (
            "schema",
            Box::new(|n: &str, b: Vec<u8>| {
                Some(if n == "manifest.json" {
                    let mut m: serde_json::Value = serde_json::from_slice(&b).unwrap();
                    m["schema"] = 99.into();
                    serde_json::to_vec(&m).unwrap()
                } else {
                    b
                })
            }),
        ),
        (
            "traversal",
            Box::new(|n: &str, b: Vec<u8>| {
                Some(if n == "manifest.json" {
                    let mut m: serde_json::Value = serde_json::from_slice(&b).unwrap();
                    m["files"][0]["path"] = "../escape.json".into();
                    serde_json::to_vec(&m).unwrap()
                } else {
                    b
                })
            }),
        ),
    ];
    for (label, edit) in cases {
        rewrite_zip(&zip, &bad, edit);
        let e = reg.import(&bad).unwrap_err();
        assert_eq!(e.code(), "validation", "{label}: {e}");
    }
    std::fs::write(&bad, b"not a zip").unwrap();
    assert_eq!(reg.import(&bad).unwrap_err().code(), "validation");
    assert_eq!(reg.list(true).unwrap().workspaces.len(), 1);
    assert_eq!(
        std::fs::read_dir(paths::workspaces_dir(d.path()))
            .unwrap()
            .count(),
        1
    );
    assert!(!d.path().join("escape.json").exists());
}

// ---------- storage errors ----------

#[test]
fn disk_full_is_a_typed_storage_full_error() {
    let enospc = std::io::Error::from_raw_os_error(28);
    assert_eq!(WorkspaceError::from(enospc).code(), "storage_full");
    let store = StoreError::Backend("No space left on device (os error 28)".into());
    assert_eq!(WorkspaceError::from(store).code(), "storage_full");
    let sqlite = StoreError::Backend("database or disk is full".into());
    let chained = anyhow::Error::new(sqlite).context("opening tracker");
    assert_eq!(WorkspaceError::from(chained).code(), "storage_full");
    let other = anyhow::anyhow!("boom");
    assert_eq!(WorkspaceError::from(other).code(), "internal");

    // Where the platform has a full device, prove a real write surfaces it.
    if Path::new("/dev/full").exists() {
        use std::io::Write;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .open("/dev/full")
            .unwrap();
        let e = f.write_all(b"x").and_then(|_| f.sync_all()).unwrap_err();
        assert_eq!(WorkspaceError::from(e).code(), "storage_full");
    }
}

#[test]
fn a_failed_metadata_write_keeps_the_previous_file() {
    use std::os::unix::fs::PermissionsExt;
    let d = tempfile::tempdir().unwrap();
    let (reg, _) = registry(d.path());
    let h = reg.create(named("Atomic")).unwrap();
    let dir = h.root().to_owned();
    let before = std::fs::read(dir.join("workspace.json")).unwrap();
    // A directory we cannot create the temp file in stands in for a full disk.
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o500)).unwrap();
    let res = reg.update(
        h.id(),
        WorkspacePatch {
            description: Some("never lands".into()),
            ..WorkspacePatch::default()
        },
    );
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o700)).unwrap();
    assert!(res.is_err());
    assert_eq!(std::fs::read(dir.join("workspace.json")).unwrap(), before);
    assert_eq!(reg.get(h.id()).unwrap().description, "");
}
