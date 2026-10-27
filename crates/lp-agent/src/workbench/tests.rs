use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;
use lp_cli_driver::engine::{
    DriverError, FauxEngine, StreamDelta, StructuredOutput, StructuredRequest, ToolCall,
    TurnRequest, TurnResponse,
};
use lp_core::graph::{GraphNode, Provenance};
use lp_core::ports::{GraphStore, JobStatus};
use lp_store::{FsGraphStore, FsJobStore};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::{json, Value};

use super::*;
use crate::loops::{Call, Kind, Msg};

struct FakeWeb;

#[async_trait]
impl Web for FakeWeb {
    async fn search(&self, q: &str) -> Result<Vec<Hit>, String> {
        Ok(vec![Hit {
            title: format!("About {q}"),
            url: "https://example.test/raft".into(),
            snippet: "leader leases".into(),
        }])
    }
    async fn fetch(&self, _: &str) -> Result<String, String> {
        Ok("<html><style>p{}</style><body><p>Leases bound stale reads.</p></body></html>".into())
    }
}

struct FakePdf;

impl Pdf for FakePdf {
    fn pages(&self, _: &str) -> Result<Vec<String>, String> {
        Ok(vec![
            "intro".into(),
            "raft elects a leader".into(),
            "appendix".into(),
        ])
    }
}

struct Rig {
    engine: Arc<FauxEngine>,
    jobs: Arc<FsJobStore>,
    wb: Arc<Workbench<FauxEngine, FsJobStore>>,
    dir: tempfile::TempDir,
}

impl Rig {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let engine = Arc::new(FauxEngine::new());
        let jobs = Arc::new(FsJobStore::open(dir.path().join("jobs")).unwrap());
        let wb = Workbench::new(
            engine.clone(),
            jobs.clone(),
            Arc::new(FakeWeb),
            Arc::new(FakePdf),
            dir.path().join("artifacts"),
        );
        Self {
            engine,
            jobs,
            wb,
            dir,
        }
    }

    /// The model calls `name`; returns the launched job's handle and id.
    async fn dispatch(&self, s: &mut Session, name: &str, args: Value) -> (JoinHandle<()>, String) {
        let call = Call {
            id: format!("c{}", s.messages.len() + s.requests.len()),
            name: name.into(),
            args,
        };
        assert!(matches!(exec(s, &call), Some(Ok(Effect::Ack(_)))));
        let id = job_id(&s.id, &call.id);
        (self.wb.launch(s).unwrap().remove(0), id)
    }
}

use crate::loops::Effect;

fn submit(rig: &Rig, args: Value) {
    rig.engine.faux_tool_call("submit", args);
}

fn svg() -> Value {
    json!({"svg": "<svg></svg>", "ascii_fallback": "[a]-[b]", "notes": "n"})
}

fn teach() -> Session {
    Session::new("c1", Kind::Teach)
}

fn steers(s: &Session) -> Vec<&Msg> {
    s.messages
        .iter()
        .filter(|m| matches!(m, Msg::Steer { .. }))
        .collect()
}

#[tokio::test]
async fn research_acks_now_and_steers_later_with_artifact() {
    let rig = Rig::new();
    rig.engine.faux_turn(vec![
        StreamDelta::ToolCall(ToolCall {
            id: "a".into(),
            name: "search".into(),
            input: json!({"query": "raft"}),
        }),
        StreamDelta::ToolCall(ToolCall {
            id: "b".into(),
            name: "fetch".into(),
            input: json!({"url": "https://example.test/raft"}),
        }),
    ]);
    submit(
        &rig,
        json!({"brief": "Leases bound stale reads.", "sources": [{"title": "Raft", "url": "https://example.test/raft"}]}),
    );
    let mut s = teach();
    let (h, id) = rig
        .dispatch(&mut s, "research", json!({"question": "raft leases"}))
        .await;
    assert!(steers(&s).is_empty());
    h.await.unwrap();

    assert!(rig.wb.deliver(&mut s).unwrap());
    let Msg::Steer { id: sid, text } = steers(&s)[0].clone() else {
        unreachable!()
    };
    assert_eq!(sid, format!("steer_{id}"));
    assert!(text.contains("Leases bound stale reads."));
    let md = std::fs::read_to_string(rig.dir.path().join(format!("artifacts/{id}.md"))).unwrap();
    assert!(md.contains("[Raft](https://example.test/raft)"));
    assert!(md.contains("## Fetched\n\n- https://example.test/raft\n"));
    // Exactly once.
    assert!(!rig.wb.deliver(&mut s).unwrap());
}

#[tokio::test]
async fn rejected_submit_is_fed_back_and_retried() {
    let rig = Rig::new();
    submit(
        &rig,
        json!({"svg": "<div/>", "ascii_fallback": "x", "notes": "n"}),
    );
    submit(&rig, svg());
    let mut s = teach();
    let (h, id) = rig
        .dispatch(&mut s, "visual", json!({"description": "a to b"}))
        .await;
    h.await.unwrap();
    let job = rig.jobs.claim_steers("c1").unwrap().remove(0);
    assert_eq!(job.status, JobStatus::Done);
    assert_eq!(job.result.unwrap()["ascii_fallback"], "[a]-[b]");
    assert!(rig.dir.path().join(format!("artifacts/{id}.svg")).exists());
}

#[tokio::test]
async fn gives_up_after_four_turns_and_delivers_the_error() {
    let rig = Rig::new();
    for _ in 0..4 {
        rig.engine.faux_text("thinking");
    }
    let mut s = teach();
    let (h, _) = rig
        .dispatch(&mut s, "visual", json!({"description": "a to b"}))
        .await;
    h.await.unwrap();
    assert!(rig.wb.deliver(&mut s).unwrap());
    let Msg::Steer { text, .. } = steers(&s)[0] else {
        unreachable!()
    };
    assert!(text.contains("error") && text.contains("no valid submit after 4 turns"));
}

#[tokio::test]
async fn pdf_tracks_pages_read_versus_consulted() {
    let rig = Rig::new();
    rig.engine.faux_turn(vec![
        StreamDelta::ToolCall(ToolCall {
            id: "a".into(),
            name: "search".into(),
            input: json!({"query": "leader"}),
        }),
        StreamDelta::ToolCall(ToolCall {
            id: "b".into(),
            name: "read".into(),
            input: json!({"page": 3}),
        }),
    ]);
    // Submitting before reading anything useful is impossible; page 3 was read, so this passes.
    submit(&rig, json!({"brief": "Raft elects a leader."}));
    let mut s = teach();
    let (h, _) = rig
        .dispatch(
            &mut s,
            "pdf",
            json!({"path": "raft.pdf", "question": "who leads?"}),
        )
        .await;
    h.await.unwrap();
    let r = rig
        .jobs
        .claim_steers("c1")
        .unwrap()
        .remove(0)
        .result
        .unwrap();
    assert_eq!(r["pagesRead"], json!([3]));
    assert_eq!(r["pagesConsulted"], json!([2, 3]));
}

#[tokio::test]
async fn pdf_submit_without_reading_is_rejected() {
    let rig = Rig::new();
    submit(&rig, json!({"brief": "guess"}));
    for _ in 0..3 {
        rig.engine.faux_text("hm");
    }
    let mut s = teach();
    let (h, _) = rig
        .dispatch(&mut s, "pdf", json!({"path": "raft.pdf", "question": "q"}))
        .await;
    h.await.unwrap();
    let r = rig
        .jobs
        .claim_steers("c1")
        .unwrap()
        .remove(0)
        .result
        .unwrap();
    assert!(r["error"]
        .as_str()
        .unwrap()
        .contains("read at least one page"));
}

#[tokio::test]
async fn demo_yields_tui_spec_and_html_artifact() {
    let rig = Rig::new();
    submit(
        &rig,
        json!({
            "title": "Decay", "html": "<html><body>demo</body></html>",
            "sliders": [{"name": "rate", "min": 0.0, "max": 1.0, "default": 0.5}],
            "frames": [[1.0, 0.5], [1.0, 0.2]]
        }),
    );
    let mut s = teach();
    let (h, id) = rig
        .dispatch(&mut s, "demo", json!({"description": "decay"}))
        .await;
    h.await.unwrap();
    let r = rig
        .jobs
        .claim_steers("c1")
        .unwrap()
        .remove(0)
        .result
        .unwrap();
    let spec: TuiDemoSpec = serde_json::from_value(r["spec"].clone()).unwrap();
    assert_eq!(spec.frames.len(), 2);
    assert!(rig.dir.path().join(format!("artifacts/{id}.html")).exists());
}

#[tokio::test]
async fn demo_with_external_references_is_rejected() {
    let rig = Rig::new();
    let bad = json!({"title": "t", "html": "<html><script src=\"https://cdn.test/x.js\"></script></html>",
        "sliders": [{"name": "r", "min": 0.0, "max": 1.0, "default": 0.5}], "frames": [[1.0]]});
    for _ in 0..4 {
        submit(&rig, bad.clone());
    }
    let mut s = teach();
    let (h, _) = rig
        .dispatch(&mut s, "demo", json!({"description": "d"}))
        .await;
    h.await.unwrap();
    let r = rig
        .jobs
        .claim_steers("c1")
        .unwrap()
        .remove(0)
        .result
        .unwrap();
    assert!(r["error"].as_str().unwrap().contains("self-contained"));
}

/// Engine that answers every turn with a valid visual submit, tracking concurrent calls.
#[derive(Default)]
struct Slow {
    now: AtomicUsize,
    peak: AtomicUsize,
}

#[async_trait]
impl LlmEngine for Slow {
    async fn stream_turn(
        &self,
        _: TurnRequest,
        _: mpsc::Sender<StreamDelta>,
        _: CancellationToken,
    ) -> Result<TurnResponse, DriverError> {
        let n = self.now.fetch_add(1, Ordering::SeqCst) + 1;
        self.peak.fetch_max(n, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(30)).await;
        self.now.fetch_sub(1, Ordering::SeqCst);
        Ok(TurnResponse {
            text: String::new(),
            tool_calls: vec![ToolCall {
                id: "s".into(),
                name: "submit".into(),
                input: svg(),
            }],
        })
    }
    async fn complete_structured<T: DeserializeOwned + JsonSchema + Send>(
        &self,
        _: StructuredRequest,
        _: CancellationToken,
    ) -> Result<StructuredOutput<T>, DriverError> {
        Err(DriverError::FauxExhausted)
    }
}

#[tokio::test]
async fn parallel_jobs_share_three_model_permits() {
    let dir = tempfile::tempdir().unwrap();
    let engine = Arc::new(Slow::default());
    let jobs = Arc::new(FsJobStore::open(dir.path().join("jobs")).unwrap());
    let wb = Workbench::new(
        engine.clone(),
        jobs.clone(),
        Arc::new(FakeWeb),
        Arc::new(FakePdf),
        dir.path(),
    );
    let mut s = teach();
    for i in 0..6 {
        let call = Call {
            id: format!("c{i}"),
            name: "visual".into(),
            args: json!({"description": "d"}),
        };
        exec(&mut s, &call).unwrap().unwrap();
    }
    for h in wb.launch(&mut s).unwrap() {
        h.await.unwrap();
    }
    assert_eq!(engine.peak.load(Ordering::SeqCst), 3);
    assert!(wb.deliver(&mut s).unwrap());
    assert_eq!(steers(&s).len(), 6);
}

#[tokio::test]
async fn steer_waits_for_the_pending_question() {
    let rig = Rig::new();
    submit(&rig, svg());
    let mut s = teach();
    s.messages.push(Msg::Assistant {
        text: String::new(),
        calls: vec![Call {
            id: "q1".into(),
            name: "probe".into(),
            args: json!({}),
        }],
    });
    let (h, _) = rig
        .dispatch(&mut s, "visual", json!({"description": "d"}))
        .await;
    h.await.unwrap();

    assert!(!rig.wb.deliver(&mut s).unwrap());
    // Unclaimed: still pending in the store, not lost.
    s.answer("q1", "A");
    assert!(rig.wb.deliver(&mut s).unwrap());
    assert_eq!(steers(&s).len(), 1);
}

#[tokio::test]
async fn learner_abort_cancels_and_never_delivers() {
    let rig = Rig::new();
    let gate = rig.engine.faux_gated_turn(vec![
        StreamDelta::Text("a".into()),
        StreamDelta::ToolCall(ToolCall {
            id: "s".into(),
            name: "submit".into(),
            input: svg(),
        }),
    ]);
    let mut s = teach();
    let (h, id) = rig
        .dispatch(&mut s, "visual", json!({"description": "d"}))
        .await;
    tokio::time::sleep(Duration::from_millis(20)).await;
    rig.wb.abort(&id);
    h.await.unwrap();
    gate.release();

    assert!(!rig.wb.deliver(&mut s).unwrap());
    assert!(rig.jobs.interrupted_jobs().unwrap().is_empty());
}

#[tokio::test]
async fn recover_turns_interrupted_jobs_into_delivered_errors() {
    let rig = Rig::new();
    let rec = JobRecord {
        id: "job_dead".into(),
        chat_id: "c1".into(),
        kind: JobKind::Research,
        status: JobStatus::Running,
        delivery: DeliveryStatus::Pending,
        created_at: 0,
        result: None,
        usage: vec![],
    };
    rig.jobs.create_job(&rec).unwrap();
    assert_eq!(rig.wb.recover().unwrap(), 1);
    let mut s = teach();
    assert!(rig.wb.deliver(&mut s).unwrap());
    let Msg::Steer { text, .. } = steers(&s)[0] else {
        unreachable!()
    };
    assert!(text.contains("interrupted by a restart"));
}

#[tokio::test]
async fn claimed_steer_lost_before_save_is_redelivered_after_restart() {
    let rig = Rig::new();
    let rec = JobRecord {
        id: "job_c".into(),
        chat_id: "c1".into(),
        kind: JobKind::Research,
        status: JobStatus::Done,
        delivery: DeliveryStatus::Pending,
        created_at: 0,
        result: Some(json!("r")),
        usage: vec![],
    };
    rig.jobs.create_job(&rec).unwrap();
    // Crash window: claimed on disk, transcript never saved.
    let mut lost = teach();
    assert!(rig.wb.deliver(&mut lost).unwrap());
    assert!(rig.jobs.claim_steers("c1").unwrap().is_empty());

    rig.wb.recover().unwrap();
    let mut s = teach();
    assert!(rig.wb.deliver(&mut s).unwrap());
    assert_eq!(steers(&s).len(), 1);

    // Saved before the crash: recovery re-offers it, but the transcript already holds it.
    rig.wb.recover().unwrap();
    assert!(!rig.wb.deliver(&mut s).unwrap());
    assert_eq!(steers(&s).len(), 1);
}

#[tokio::test]
async fn pump_runs_a_continuation_turn_only_when_woken() {
    let rig = Rig::new();
    submit(&rig, svg());
    let mut s = teach();
    let (h, _) = rig
        .dispatch(&mut s, "visual", json!({"description": "d"}))
        .await;
    let (tx, _rx) = mpsc::channel(64);
    let cancel = CancellationToken::new();
    h.await.unwrap();

    rig.engine.faux_text("Here is your diagram.");
    let out = rig
        .wb
        .pump(Path::new("."), &mut s, &tx, &cancel)
        .await
        .unwrap();
    assert_eq!(out.unwrap().text, "Here is your diagram.");
    assert!(matches!(s.messages.last(), Some(Msg::Assistant { .. })));
    assert!(rig
        .wb
        .pump(Path::new("."), &mut s, &tx, &cancel)
        .await
        .unwrap()
        .is_none());
}

#[test]
fn dispatch_tools_validate_and_list_tools_read_the_graph() {
    let mut s = teach();
    let bad = Call {
        id: "x".into(),
        name: "pdf".into(),
        args: json!({"path": "a.pdf"}),
    };
    assert!(exec(&mut s, &bad).unwrap().is_err());
    assert!(s.requests.is_empty());
    assert!(exec(
        &mut s,
        &Call {
            id: "y".into(),
            name: "nope".into(),
            args: json!({})
        }
    )
    .is_none());

    let list = Call {
        id: "l".into(),
        name: "list_sessions".into(),
        args: json!({}),
    };
    assert!(exec(&mut s, &list).unwrap().is_err());

    let dir = tempfile::tempdir().unwrap();
    let graph = Arc::new(FsGraphStore::open(dir.path()).unwrap());
    graph
        .append_node(&GraphNode {
            id: "n_1".into(),
            name: "Ownership".into(),
            statement: "One owner".into(),
            aliases: vec![],
            created_at: Utc::now(),
            provenance: vec![Provenance {
                chat_id: "c9".into(),
                message_id: "m".into(),
            }],
        })
        .unwrap();
    let mut s = teach().with_graph(graph);
    let Ok(Effect::Ack(sessions)) = exec(&mut s, &list).unwrap() else {
        panic!()
    };
    assert_eq!(sessions, "c9");
    let props = Call {
        id: "p".into(),
        name: "list_propositions".into(),
        args: json!({"session": "c9"}),
    };
    let Ok(Effect::Ack(lines)) = exec(&mut s, &props).unwrap() else {
        panic!()
    };
    assert_eq!(lines, "n_1: Ownership - One owner");
}

#[test]
fn readable_drops_markup_scripts_and_styles() {
    let html = "<HTML><Script>var a = '<b>';</Script><p>Hello <b>big</b>\n world</p><style>x{}</style></HTML>";
    assert_eq!(readable(html), "Hello big world");
}

#[test]
fn readable_is_linear_on_many_scripts() {
    let html = "<script>a</script>x".repeat(20_000);
    let t = std::time::Instant::now();
    assert_eq!(readable(&html).len(), 20_000 * 2 - 1);
    assert!(t.elapsed().as_secs() < 2);
}

#[tokio::test]
async fn each_job_kind_consumes_only_its_own_roles_engine() {
    let dir = tempfile::tempdir().unwrap();
    let engines: HashMap<&str, Arc<FauxEngine>> = ROLES
        .into_iter()
        .map(|r| (r, Arc::new(FauxEngine::new())))
        .collect();
    let jobs = Arc::new(FsJobStore::open(dir.path().join("jobs")).unwrap());
    let wb = {
        let engines = engines.clone();
        Workbench::routed(
            move |r| engines[r].clone(),
            jobs.clone(),
            Arc::new(FakeWeb),
            Arc::new(FakePdf),
            dir.path().join("artifacts"),
        )
    };
    let tool = |role: &str, args: Value| engines[role].faux_tool_call("submit", args);
    tool(
        "research",
        json!({"brief": "b", "sources": [{"title": "t", "url": "https://example.test/x"}]}),
    );
    tool("visual", svg());
    tool(
        "demo",
        json!({"title": "t", "html": "<html></html>", "sliders": [{"name": "a", "min": 0.0, "max": 1.0, "default": 0.5}], "frames": [[1.0]]}),
    );
    engines["pdf"].faux_tool_call("read", json!({"page": 1}));
    tool("pdf", json!({"brief": "b"}));
    let mut s = teach();
    for (i, (name, args)) in [
        ("research", json!({"question": "q"})),
        ("visual", json!({"description": "d"})),
        ("demo", json!({"description": "d"})),
        ("pdf", json!({"path": "p.pdf", "question": "q"})),
    ]
    .into_iter()
    .enumerate()
    {
        let call = Call {
            id: format!("c{i}"),
            name: name.into(),
            args,
        };
        exec(&mut s, &call).unwrap().unwrap();
    }
    for h in wb.launch(&mut s).unwrap() {
        h.await.unwrap();
    }
    // A job on the wrong engine would hit an empty script or a foreign submit and deliver an error.
    let done = jobs.claim_steers("c1").unwrap();
    assert_eq!(done.len(), 4);
    for job in done {
        assert!(job.result.unwrap().get("error").is_none());
    }
}
