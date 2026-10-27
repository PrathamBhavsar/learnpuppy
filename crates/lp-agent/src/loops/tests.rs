use std::collections::HashMap;
use std::path::Path;

use lp_cli_driver::engine::{FauxEngine, StreamDelta};
use lp_core::fsrs::TurnUsage;
use lp_core::plan::Phase;
use lp_core::ports::{ChatRecord, ChatStore, ChatSummary, StoreError, StoreResult};
use parking_lot::Mutex;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::*;

#[derive(Default)]
struct MemChats(Mutex<HashMap<String, ChatRecord>>);

impl ChatStore for MemChats {
    fn load_chat(&self, id: &str) -> StoreResult<ChatRecord> {
        self.0
            .lock()
            .get(id)
            .cloned()
            .ok_or_else(|| StoreError::NotFound(id.into()))
    }
    fn save_chat(&self, chat: &ChatRecord) -> StoreResult<()> {
        self.0.lock().insert(chat.id.to_string(), chat.clone());
        Ok(())
    }
    fn append_turn_usage(&self, _: &str, _: TurnUsage) -> StoreResult<()> {
        Ok(())
    }
    fn append_extraction_usage(&self, _: &str, _: TurnUsage) -> StoreResult<()> {
        Ok(())
    }
    fn list_chats(&self) -> StoreResult<Vec<ChatSummary>> {
        Ok(Vec::new())
    }
}

struct Rig {
    engine: FauxEngine,
    tx: mpsc::Sender<StreamDelta>,
    _rx: mpsc::Receiver<StreamDelta>,
}

impl Rig {
    fn new() -> Self {
        let (tx, _rx) = mpsc::channel(256);
        Self {
            engine: FauxEngine::new(),
            tx,
            _rx,
        }
    }

    async fn turn(&self, s: &mut Session) -> Result<TurnOutcome, TurnError> {
        run_turn(
            &self.engine,
            Path::new("."),
            s,
            &self.tx,
            &CancellationToken::new(),
        )
        .await
    }
}

fn quiz_args(extra: Value) -> Value {
    let mut v = json!({
        "question": "Which is a Rust ownership rule?",
        "options": [
            {"label": "One owner", "value": "a"},
            {"label": "Two owners", "value": "b"},
            {"label": "No owner", "value": "c"}
        ],
        "correctAnswer": "a",
        "explanation": "Each value has exactly one owner."
    });
    v.as_object_mut()
        .unwrap()
        .extend(extra.as_object().unwrap().clone());
    v
}

const PLAN: &str = "graph TD\n  own[\"Ownership — who frees a value\"]\n  bor[\"Borrowing — refs\"]\n  own --> bor";

fn last_call_id(s: &Session) -> String {
    s.calls().last().unwrap().id.clone()
}

#[tokio::test]
async fn probe_handoff_plan_teach_lifecycle() {
    let rig = Rig::new();
    let mut s = Session::new("c1", Kind::Teach);
    s.push_user("teach me ownership");

    // Probe: one question ends the turn.
    rig.engine.faux_tool_call("probe", quiz_args(json!({})));
    let out = rig.turn(&mut s).await.unwrap();
    assert!(out.pending_question);
    assert_eq!(s.phase(), Phase::Probe);

    // Answer, then probe_done succeeds and queues the handoff steer.
    let id = last_call_id(&s);
    s.answer(&id, "selected: a; confidence: 4");
    rig.engine.faux_tool_call("probe_done", json!({}));
    rig.engine.faux_text("Probe wrapped up.");
    rig.turn(&mut s).await.unwrap();
    assert_eq!(s.phase(), Phase::Plan);
    assert!(s.messages.iter().any(|m| matches!(
        m, Msg::ToolResult { name, ok: true, text, .. }
            if name == "probe_done" && text == lp_core::plan::PROBE_DONE_ACK_TEXT)));

    // Handoff wake: steer lands in the transcript once, and the teacher takes over.
    assert!(s.wake());
    assert!(!s.wake());
    assert!(matches!(s.messages.last(), Some(Msg::Steer { id, .. }) if id == "steer_probe_c1"));
    rig.engine
        .faux_tool_call("plan_init", json!({"mermaid": PLAN}));
    rig.engine
        .faux_tool_call("quiz", quiz_args(json!({"type": "check", "nodeId": "own"})));
    let out = rig.turn(&mut s).await.unwrap();
    assert!(out.pending_question);
    assert_eq!(s.phase(), Phase::Teach);
    assert_eq!(s.plan.as_ref().unwrap().nodes.len(), 2);

    // Answer, then the next step moves to the next node.
    let id = last_call_id(&s);
    s.answer(&id, "selected: a");
    rig.engine.faux_tool_call(
        "quiz",
        quiz_args(json!({"type": "predict", "nodeId": "bor"})),
    );
    let out = rig.turn(&mut s).await.unwrap();
    assert!(out.pending_question);
    let prompt_agent = s.agent(Path::new("."));
    let role = prompt_agent.role_prompt(&s);
    assert!(role.contains("Current: bor. Visited: own. Upcoming: ."));
}

#[tokio::test]
async fn invalid_call_is_retried_with_the_problem_text() {
    let rig = Rig::new();
    let mut s = Session::new("c2", Kind::Teach);
    // probe must not carry nodeId; the model corrects itself on the next step.
    rig.engine
        .faux_tool_call("probe", quiz_args(json!({"nodeId": "own"})));
    rig.engine.faux_tool_call("probe", quiz_args(json!({})));
    let out = rig.turn(&mut s).await.unwrap();
    assert!(out.pending_question);
    assert!(s.messages.iter().any(|m| matches!(
        m, Msg::ToolResult { ok: false, text, .. } if text.contains("probe takes no type or nodeId"))));
}

#[tokio::test]
async fn gives_up_after_repeated_invalid_calls() {
    let rig = Rig::new();
    let mut s = Session::new("c3", Kind::Teach);
    for _ in 0..3 {
        rig.engine.faux_tool_call("nope", json!({}));
    }
    assert!(matches!(
        rig.turn(&mut s).await,
        Err(TurnError::TooManyInvalid)
    ));
}

#[tokio::test]
async fn probe_done_is_refused_while_a_question_is_unanswered() {
    let rig = Rig::new();
    let mut s = Session::new("c4", Kind::Teach);
    rig.engine.faux_tool_call("probe", quiz_args(json!({})));
    rig.turn(&mut s).await.unwrap();
    rig.engine.faux_tool_call("probe_done", json!({}));
    rig.engine.faux_tool_call("probe_done", json!({}));
    rig.engine.faux_tool_call("probe_done", json!({}));
    assert!(rig.turn(&mut s).await.is_err());
    assert_eq!(s.phase(), Phase::Probe);
}

#[tokio::test]
async fn quiz_is_unavailable_in_the_probe_phase() {
    let rig = Rig::new();
    let mut s = Session::new("c5", Kind::Teach);
    rig.engine
        .faux_tool_call("quiz", quiz_args(json!({"type": "check", "nodeId": "own"})));
    rig.engine.faux_tool_call("probe", quiz_args(json!({})));
    rig.turn(&mut s).await.unwrap();
    assert!(s.messages.iter().any(|m| matches!(
        m, Msg::ToolResult { ok: false, text, .. } if text.contains("unknown tool `quiz`"))));
}

#[tokio::test]
async fn practice_problem_conclude_alternate_and_reveal_solution() {
    let rig = Rig::new();
    let mut s = Session::new("p1", Kind::Practice);
    s.push_user("practice ownership");
    let problem = json!({"statement": "Why does `let b = a;` invalidate `a`?",
        "mode": "recall", "modelSolution": "Ownership moved to b."});
    rig.engine.faux_tool_call("problem", problem.clone());
    assert!(rig.turn(&mut s).await.unwrap().pending_question);

    // A second problem while one is open is refused by the gate.
    let id = last_call_id(&s);
    s.answer(&id, "because of a move");
    rig.engine.faux_tool_call("problem", problem.clone());
    rig.engine.faux_tool_call(
        "conclude",
        json!({"outcome": "solved", "verdict": "correct", "rung": null, "missClass": null, "retest": false}),
    );
    let out = rig.turn(&mut s).await.unwrap();
    assert!(!out.pending_question);
    assert!(s.messages.iter().any(|m| matches!(
        m, Msg::ToolResult { ok: false, text, .. } if text.contains("already open"))));
    assert!(s.messages.iter().any(|m| matches!(
        m, Msg::ToolResult { name, ok: true, text, .. }
            if name == "conclude" && text == "Model solution: Ownership moved to b.")));

    // Concluding again with no open problem is refused.
    s.push_user("[NEXT PROBLEM]");
    rig.engine.faux_tool_call(
        "conclude",
        json!({"outcome": "solved", "verdict": "x", "rung": null, "missClass": null, "retest": false}),
    );
    rig.engine.faux_text("ok");
    rig.turn(&mut s).await.unwrap();
    assert!(s.messages.iter().any(|m| matches!(
        m, Msg::ToolResult { ok: false, text, .. } if text.contains("no open problem"))));
}

#[tokio::test]
async fn file_tools_work_and_stay_inside_the_workspace() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("notes.md"), "alpha\nbeta needle\n").unwrap();
    let call = |name: &str, args: Value| Call {
        id: "x".into(),
        name: name.into(),
        args,
    };
    let text = |r: Option<Result<Effect, String>>| match r {
        Some(Ok(Effect::Ack(t))) => t,
        Some(Err(e)) => e,
        _ => panic!("unexpected effect"),
    };
    let root = dir.path();
    assert_eq!(
        text(tools::file_tool(
            root,
            &call("read", json!({"path": "notes.md"}))
        )),
        "alpha\nbeta needle\n"
    );
    assert_eq!(
        text(tools::file_tool(root, &call("ls", json!({})))),
        "notes.md"
    );
    assert_eq!(
        text(tools::file_tool(
            root,
            &call("find", json!({"pattern": "notes"}))
        )),
        "notes.md"
    );
    assert_eq!(
        text(tools::file_tool(
            root,
            &call("grep", json!({"pattern": "needle"}))
        )),
        "notes.md:2: beta needle"
    );
    assert!(
        text(tools::file_tool(
            root,
            &call("read", json!({"path": "../../etc/passwd"}))
        ))
        .contains("outside the workspace")
            || text(tools::file_tool(
                root,
                &call("read", json!({"path": "../../etc/passwd"}))
            ))
            .contains("No such file")
    );
}

#[test]
fn compaction_triggers_past_20k_tokens_and_never_splits_a_quiz_pair() {
    let mut s = Session::new("k1", Kind::Teach);
    assert_eq!(s.compaction_cut(), None);
    s.push_user("x".repeat(40_000)); // ~10k tokens
    s.messages.push(Msg::Assistant {
        text: String::new(),
        calls: vec![Call {
            id: "q".into(),
            name: "quiz".into(),
            args: json!({}),
        }],
    });
    s.answer("q", "y".repeat(40_000)); // ~10k tokens
    s.push_user("z".repeat(40_000));
    let cut = s.compaction_cut().expect("over threshold");
    assert!(cut > 0 && !matches!(s.messages[cut], Msg::Answer { .. }));
    s.apply_compaction(cut, "earlier stuff".into());
    assert!(matches!(&s.messages[0], Msg::Compaction(t) if t == "earlier stuff"));
    // The summary alone is not re-compacted.
    assert_eq!(s.compaction_cut(), None);
}

#[test]
fn session_round_trips_through_the_chat_store() {
    let store = MemChats::default();
    let mut s = Session::new("r1", Kind::Practice);
    s.push_user("hi");
    s.save(&store, 10).unwrap();
    s.push_user("again");
    s.save(&store, 20).unwrap();
    assert_eq!(Session::load(&store, "r1").unwrap(), s);
    assert_eq!(store.load_chat("r1").unwrap().created_at, 10);
}

#[test]
fn file_tools_redact_refuse_hidden_paths_and_cap_reads() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    std::fs::write(root.join(".env"), "API_KEY=hunter2hunter2\n").unwrap();
    std::fs::write(root.join("a.txt"), "OPENAI_API_KEY=hunter2hunter2\n").unwrap();
    std::fs::create_dir(root.join("node_modules")).unwrap();
    std::fs::write(
        root.join("node_modules/b.txt"),
        "OPENAI_API_KEY=hunter2hunter2",
    )
    .unwrap();
    std::fs::write(root.join("big.txt"), "x".repeat(200_000)).unwrap();
    let run = |name: &str, args: Value| match tools::file_tool(
        root,
        &Call {
            id: "x".into(),
            name: name.into(),
            args,
        },
    ) {
        Some(Ok(Effect::Ack(t))) => t,
        Some(Err(e)) => e,
        _ => panic!("unexpected effect"),
    };
    assert!(run("read", json!({"path": ".env"})).contains("hidden"));
    std::os::unix::fs::symlink(root.join(".env"), root.join("link")).unwrap();
    assert!(run("read", json!({"path": "link"})).contains("hidden"));
    assert!(!run("read", json!({"path": "a.txt"})).contains("hunter2"));
    let hits = run("grep", json!({"pattern": "KEY"}));
    assert!(hits.contains("a.txt") && !hits.contains("hunter2") && !hits.contains("node_modules"));
    assert_eq!(run("read", json!({"path": "big.txt"})).len(), 20_000);
}
