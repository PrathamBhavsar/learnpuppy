//! End-to-end flows over the real `learnpuppy` binary and `Backend`, with a `FauxEngine`
//! and a tempdir home: no real CLIs, no network.

use std::io::Write;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lp_agent::extract::{Pipeline, SourceMessage};
use lp_agent::loops::{Kind, Session};
use lp_cli_driver::accounts::{AccountPool, Strategy};
use lp_cli_driver::engine::FauxEngine;
use lp_core::plan::Phase;
use lp_core::ports::{CodingSession, TrackerStore};
use lp_ingest::distill::{distill_session, generate_decision_cards};
use lp_tui::app::{Command as Cmd, DataSource, ViewSnapshot};
use lp_tui::backend::Backend;
use lp_tui::session::run_to_rest;
use lp_tui::state::{now_ms, Settings, Stores};
use lp_tui::views::classroom::{Answer, Prompt};
use lp_tui::views::graph::{render, GraphUi};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

fn write(path: &Path, text: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// Runs the binary against `home`, returning stdout.
fn learnpuppy(home: &Path, args: &[&str], stdin: &str) -> String {
    let mut child = Command::new(env!("CARGO_BIN_EXE_learnpuppy"))
        .args(args)
        .env("HOME", home)
        .env("LEARNPUPPY_HOME", home.join("lp"))
        .env_remove("CLAUDE_CONFIG_DIR")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(stdin.as_bytes())
        .unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(
        out.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap()
}

fn backend(home: &Path, faux: &Arc<FauxEngine>) -> Backend<FauxEngine> {
    let root = home.join("lp");
    let faux = faux.clone();
    Backend::new(
        root.clone(),
        home.to_owned(),
        Stores::open(&root).unwrap(),
        Settings::default(),
        Arc::new(AccountPool::new(vec![], Strategy::Priority)),
        move |_, _| faux.clone(),
    )
}

/// Polls snapshots until `done` holds; the Backend publishes turn results asynchronously.
fn wait_for(b: &Backend<FauxEngine>, done: impl Fn(&ViewSnapshot) -> bool) -> ViewSnapshot {
    let end = Instant::now() + Duration::from_secs(10);
    loop {
        let snap = b.snapshot();
        if done(&snap) {
            return snap;
        }
        assert!(
            Instant::now() < end,
            "timed out: {:?}",
            snap.classroom.transcript
        );
        std::thread::sleep(Duration::from_millis(10));
    }
}

#[test]
fn discovers_two_claude_codex_and_agy_accounts() {
    let d = tempfile::tempdir().unwrap();
    let h = d.path();
    // The default `~/.claude` keeps its config next to it as `~/.claude.json`.
    write(
        &h.join(".claude.json"),
        r#"{"oauthAccount":{"emailAddress":"a@x.com"}}"#,
    );
    write(&h.join(".claude/.keep"), "");
    write(
        &h.join(".claude-accounts/work/.claude.json"),
        r#"{"oauthAccount":{"emailAddress":"b@x.com"}}"#,
    );
    // JWT payload `{"email":"cx@x.com"}`.
    write(
        &h.join(".codex/auth.json"),
        r#"{"tokens":{"id_token":"h.eyJlbWFpbCI6ImN4QHguY29tIn0.s"}}"#,
    );
    write(
        &h.join(".gemini/oauth_creds.json"),
        r#"{"refresh_token":"r","expiry_date":1}"#,
    );
    write(
        &h.join(".gemini/google_accounts.json"),
        r#"{"active":"g@x.com"}"#,
    );

    let rows: Value =
        serde_json::from_str(&learnpuppy(h, &["auth", "status", "--json"], "")).unwrap();
    let ids: Vec<&str> = rows
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        ids,
        [
            "claude:a@x.com",
            "claude:b@x.com",
            "codex:cx@x.com",
            "agy:default"
        ]
    );
    assert_eq!(rows[3]["email"], "g@x.com");
    assert!(rows
        .as_array()
        .unwrap()
        .iter()
        .all(|r| r["auth_expired"] == false));

    let text = learnpuppy(h, &["auth", "status"], "");
    assert_eq!(text.lines().count(), 4);
}

const PLAN: &str = "graph TD\n  own[\"Ownership — who frees a value\"]\n  bor[\"Borrowing — refs\"]\n  own --> bor";

fn quiz(extra: Value) -> Value {
    let mut v = json!({
        "question": "Which is a Rust ownership rule?",
        "options": [
            {"label": "One owner", "value": "a"},
            {"label": "Two owners", "value": "b"}
        ],
        "correctAnswer": "a",
        "explanation": "Each value has exactly one owner."
    });
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    v
}

fn answer(value: &str) -> Cmd {
    Cmd::Answer(Answer {
        value: value.into(),
        confidence: 4,
        reading_thinking: String::new(),
        thinking: String::new(),
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn classroom_probe_plan_teach_extract_graph() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().join("lp");
    let faux = Arc::new(FauxEngine::new());

    // Probe turn: the teacher asks one question, then the chat is saved for the Backend to resume.
    faux.faux_tool_call("probe", quiz(json!({})));
    let mut s = Session::new("c1", Kind::Teach);
    s.push_user("teach me ownership");
    let (tx, _rx) = tokio::sync::mpsc::channel(512);
    let engines = {
        let faux = faux.clone();
        move |_: &str| faux.clone()
    };
    run_to_rest(&engines, d.path(), &mut s, &tx, &CancellationToken::new())
        .await
        .unwrap();
    s.save(Stores::open(&root).unwrap().chats.as_ref(), 1)
        .unwrap();

    let b = backend(d.path(), &faux);
    let snap = b.snapshot().classroom;
    assert_eq!(snap.phase, Phase::Probe);
    assert!(matches!(snap.prompt, Some(Prompt::Quiz { .. })));

    // Answering the probe runs probe_done -> handoff -> plan_init -> the first graded quiz.
    faux.faux_tool_call("probe_done", json!({}));
    faux.faux_text("Probe wrapped up.");
    faux.faux_tool_call("plan_init", json!({"mermaid": PLAN}));
    faux.faux_tool_call("quiz", quiz(json!({"type": "check", "nodeId": "own"})));
    b.apply(answer("a"));
    let cl = wait_for(&b, |s| {
        s.classroom.phase == Phase::Teach && s.classroom.prompt.is_some()
    })
    .classroom;
    assert_eq!(cl.plan.as_ref().unwrap().nodes.len(), 2);
    assert!(matches!(cl.prompt, Some(Prompt::Quiz { .. })));

    // Answering the quiz clears the dock.
    faux.faux_text("Exactly: one owner.");
    b.apply(answer("a"));
    let cl = wait_for(&b, |s| {
        s.classroom.prompt.is_none()
            && s.classroom
                .transcript
                .iter()
                .any(|t| t.contains("Exactly: one owner."))
    })
    .classroom;
    assert_eq!(cl.phase, Phase::Teach);

    // Extraction folds the taught passage into the graph; the Graph view lists its chain.
    faux.faux_structured(json!({
        "propositions": [
            {"id": "p1", "name": "Single owner", "statement": "Each value has one owner"},
            {"id": "p2", "name": "Move on assign", "statement": "Assignment moves ownership"}
        ],
        "chains": [{"premises": ["p1"], "conclusion": "p2"}]
    }));
    faux.faux_structured(json!({"links": []}));
    let stores = Stores::open(&root).unwrap();
    let passage = SourceMessage {
        id: "m1".into(),
        text: "Each value has one owner, so assignment moves ownership.".into(),
    };
    let n = Pipeline::new(faux.clone(), stores.graph.clone())
        .extract("c1", &[passage], CancellationToken::new())
        .await
        .unwrap();
    assert_eq!(n, 1);

    let graph = b.snapshot().graph;
    assert_eq!(graph.snap.nodes.len(), 2);
    assert_eq!(graph.snap.chains.len(), 1);
    let mut term = Terminal::new(TestBackend::new(100, 30)).unwrap();
    term.draw(|f| render(f, f.area(), &GraphUi::default(), &graph))
        .unwrap();
    let screen = term.backend().to_string();
    assert!(
        screen.contains("Chains") && screen.contains("1 chains"),
        "{screen}"
    );
    let stats = learnpuppy(d.path(), &["graph", "stats"], "");
    assert!(
        stats.contains("nodes 2") && stats.contains("chains 1"),
        "{stats}"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn ingest_distill_cards_then_fsrs_review() {
    let d = tempfile::tempdir().unwrap();
    let h = d.path();
    // `ingest --repo` canonicalizes, so the transcript's cwd must be canonical too (/var -> /private/var).
    let repo = h.join("code/tracker");
    std::fs::create_dir_all(&repo).unwrap();
    let repo = repo.canonicalize().unwrap();
    let cwd = repo.to_str().unwrap();
    let t0 = "2026-05-01T10:00:00Z";
    write(
        &h.join(".claude/projects/p/s1.jsonl"),
        &format!(
            r#"{{"type":"user","cwd":"{cwd}","gitBranch":"main","timestamp":"{t0}","message":{{"content":"why copy the agy db?"}}}}
{{"type":"assistant","timestamp":"{t0}","message":{{"content":[{{"type":"text","text":"WAL files are live."}}]}}}}
"#
        ),
    );
    let out = learnpuppy(h, &["ingest", "--repo", cwd], "");
    assert!(out.contains("files 1  sessions 1"), "{out}");
    // Cursors make a second scan a no-op.
    let out = learnpuppy(h, &["ingest"], "");
    assert!(out.contains("files 0  sessions 0"), "{out}");

    // The tracker has no session read-back, so distill the same session by value.
    let session = CodingSession {
        id: "claude:s1".into(),
        source: "claude".into(),
        cwd: cwd.into(),
        git_branch: Some("main".into()),
        started_at: 0,
        transcript: "U: why copy the agy db?\nA: WAL files are live.\n".into(),
    };
    let faux = Arc::new(FauxEngine::new());
    faux.faux_structured(json!({"decisions": [{
        "title": "Snapshot copy for WAL ingestion",
        "what": "agy.rs copies the db before reading.",
        "why": "The live WAL-mode db may be mid-write.",
        "basis": "stated",
        "alternatives": [{"option": "open in place", "rejected_because": "locks the writer"}],
        "risk": "copy can be torn",
        "files": ["src/agy.rs"],
        "commit_sha": null
    }]}));
    faux.faux_structured(json!({"cards": [{
        "front": "Why does the ingester copy a WAL-mode database before reading it?",
        "right": "The live file may be mid-write",
        "wrong": [
            "SQLite cannot be opened read-only at all",
            "Copying makes the queries run much faster",
            "The WAL file is always encrypted on disk"
        ],
        "rationale": "A live WAL db can change under a reader."
    }]}));
    let stores = Stores::open(&h.join("lp")).unwrap();
    let decisions = distill_session(
        faux.as_ref(),
        stores.tracker.as_ref(),
        &session,
        &[],
        CancellationToken::new(),
    )
    .await
    .unwrap();
    let report = generate_decision_cards(
        faux.as_ref(),
        stores.tracker.as_ref(),
        &decisions,
        now_ms(),
        CancellationToken::new(),
    )
    .await
    .unwrap();
    assert_eq!(report.saved, 1, "{:?}", report.rejected);

    // The Digest view sees the decision and a due 4-choice card.
    let b = backend(h, &faux);
    let digest = b.snapshot().digest;
    assert_eq!(digest.decisions.len(), 1);
    assert_eq!(digest.decisions[0].tool, "claude");
    assert!(digest.decisions[0].stated);
    assert_eq!(digest.due.len(), 1);
    assert_eq!(digest.due[0].options.len(), 4);
    assert_eq!(digest.streak, 0);

    // Headless review: pick the right option, grade it good.
    let right = digest.due[0].correct + 1;
    let out = learnpuppy(h, &["review"], &format!("{right}\n3\n"));
    assert!(
        out.contains("correct:") && out.contains("reviewed 1"),
        "{out}"
    );
    let digest = b.snapshot().digest;
    assert!(digest.due.is_empty());
    assert_eq!(digest.streak, 1);
    assert_eq!(stores.tracker.reviews().unwrap().len(), 1);
}
