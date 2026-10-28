//! Desktop & E2E integration suite for `learnpuppy-app` (`lp-app`).
//!
//! Verifies:
//! 1. SSR-level component rendering and state transitions across `Shell`, `GraphCanvasView`,
//!    `ClassroomView`, `DigestFsrsView`, and `Accounts` using `FauxBackend` and `DesktopBackend`.
//! 2. Subprocess smoke verification of `learnpuppy-app` in `LP_E2E_PORT` / `--e2e-port` web-harness
//!    mode, confirming HTTP 200 liveview mount and WebSocket interpreter glue.

use std::collections::BTreeSet;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use dioxus::prelude::*;
use lp_app::backend::{AccountSlot, DesktopBackend, FauxBackend};
use lp_app::state::{QuickAction, View};
use lp_app::views::classroom::{
    compute_dag_coordinates, evaluate_question_submission, PlanNodeStatus,
};
use lp_app::views::digest_fsrs::{
    default_deck_cards, default_digest_sessions, format_interval_ms, fsrs_state_label,
    rating_label, rating_slug,
};
use lp_app::{AppWithBackend, AppWithBackendProps};
use lp_core::fsrs::{preview, review, Rating, State as FsrsState};
use lp_core::graph::{GraphChain, GraphNode, Provenance};
use lp_core::plan::{
    derive_plan_statuses, heal_encoded_arguments, parse_plan, CorrectAnswer, QuizInput, QuizOption,
    ToolCall,
};
use lp_core::ports::GraphStore;
use lp_store::FsGraphStore;

fn sample_slots() -> Vec<AccountSlot> {
    vec![
        AccountSlot {
            id: "claude:pratham.bhavsar501@gmail.com".into(),
            provider: "claude".into(),
            email: Some("pratham.bhavsar501@gmail.com".into()),
            tier: Some("max_5x".into()),
            config_dir: None,
            status: "Ready".into(),
            requests: 2,
        },
        AccountSlot {
            id: "claude:q97159745@gmail.com".into(),
            provider: "claude".into(),
            email: Some("q97159745@gmail.com".into()),
            tier: Some("pro".into()),
            config_dir: None,
            status: "Ready".into(),
            requests: 0,
        },
        AccountSlot {
            id: "codex:default".into(),
            provider: "codex".into(),
            email: Some("pratham.bhavsar501@gmail.com".into()),
            tier: Some("pro".into()),
            config_dir: None,
            status: "Ready".into(),
            requests: 1,
        },
        AccountSlot {
            id: "agy:default".into(),
            provider: "agy".into(),
            email: Some("pratham.bhavsar501@gmail.com".into()),
            tier: None,
            config_dir: None,
            status: "Ready".into(),
            requests: 0,
        },
    ]
}

async fn render_until(
    view: View,
    accounts_open: bool,
    backend: Arc<dyn lp_app::backend::Backend>,
    markers: &[&str],
) -> String {
    let mut dom = VirtualDom::new_with_props(
        AppWithBackend,
        AppWithBackendProps {
            backend,
            initial_view: view,
            accounts_open,
        },
    );
    dom.rebuild_in_place();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let html = dioxus::ssr::render(&dom);
            if markers.iter().all(|m| html.contains(m)) {
                return html;
            }
            dom.wait_for_work().await;
            dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
        }
    })
    .await
    .unwrap_or_else(|_| panic!("timed out waiting for markers `{markers:?}` in view {view:?}"))
}

#[tokio::test]
async fn shell_and_all_five_views_render_with_faux_backend_and_multi_account_slots() {
    let backend = Arc::new(FauxBackend {
        slots: sample_slots(),
        error: None,
    });

    // 1. Graph View + Accounts Modal open
    let graph_html = render_until(
        View::Graph,
        true,
        backend.clone(),
        &[
            r#"data-testid="accounts-list""#,
            r#"data-testid="graph-canvas""#,
        ],
    )
    .await;
    assert!(graph_html.contains(r#"data-testid="app-shell""#));
    assert!(graph_html.contains(r#"data-testid="navigation-rail""#));
    assert!(graph_html.contains(r#"data-testid="view-graph""#));
    assert!(graph_html.contains(r#"data-testid="graph-canvas""#));
    assert!(graph_html.contains(r#"data-testid="open-command-palette""#));
    assert!(graph_html.contains(r#"data-testid="open-accounts""#));
    assert!(graph_html.contains("pratham.bhavsar501@gmail.com"));
    assert!(graph_html.contains("q97159745@gmail.com"));
    assert_eq!(
        graph_html.matches(r#"data-testid="account-slot""#).count(),
        4
    );

    for nav_view in View::ALL {
        assert!(graph_html.contains(&format!(r#"data-testid="nav-{}""#, nav_view.id())));
    }
    for action in QuickAction::ALL {
        assert!(graph_html.contains(&format!(r#"data-testid="action-{}""#, action.id())));
    }

    // 2. Classroom View
    let classroom_html = render_until(
        View::Classroom,
        false,
        backend.clone(),
        &[r#"data-testid="question-dock""#],
    )
    .await;
    assert!(classroom_html.contains(r#"data-testid="view-classroom""#));
    assert!(classroom_html.contains(r#"data-testid="session-drawer""#));
    assert!(classroom_html.contains(r#"data-testid="confidence-slider""#));
    assert!(classroom_html.contains(r#"data-testid="graded-verdict""#));
    assert!(classroom_html.contains(r#"data-testid="plan-dag-svg""#));

    // 3. Plan View
    let plan_html = render_until(
        View::Plan,
        false,
        backend.clone(),
        &[r#"data-testid="view-plan""#],
    )
    .await;
    assert!(plan_html.contains(r#"data-testid="plan-dag-pane""#));
    assert!(plan_html.contains(r#"data-testid="dag-node-snapshot-isolation""#));

    // 4. FSRS Review View
    let review_html = render_until(
        View::Review,
        false,
        backend.clone(),
        &[r#"data-testid="fsrs-active-card""#],
    )
    .await;
    assert!(review_html.contains(r#"data-testid="view-review""#));
    assert!(review_html.contains(r#"data-testid="fsrs-grade-good""#));
    assert!(review_html.contains(r#"data-testid="fsrs-option-A""#));

    // 5. CLI Decision Digest View
    let digest_html = render_until(
        View::Digest,
        false,
        backend,
        &[r#"data-testid="digest-fold-into-graph""#],
    )
    .await;
    assert!(digest_html.contains(r#"data-testid="view-digest""#));
    assert!(digest_html.contains(r#"data-testid="digest-ingest-now""#));
    assert!(
        digest_html.contains(r#"data-testid="digest-decision-card-d_fsrs_deterministic_replay""#)
    );
}

#[tokio::test]
async fn desktop_backend_renders_populated_graph_nodes_junctions_and_clusters() {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("lp-e2e-desktop-{}-{nanos}", std::process::id()));
    let home = root.join("home");
    let data = root.join("data");
    std::fs::create_dir_all(data.join("graph")).unwrap();

    let store = FsGraphStore::open(data.join("graph")).unwrap();
    let created_at = "2026-10-28T00:00:00Z".parse().unwrap();
    let prov = vec![Provenance {
        chat_id: "sess-e2e".into(),
        message_id: "m1".into(),
    }];
    for (id, name, statement) in [
        ("n_alpha", "Alpha Invariant", "Alpha statement"),
        ("n_beta", "Beta Invariant", "Beta statement"),
        ("n_gamma", "Gamma Conclusion", "Gamma statement"),
    ] {
        store
            .append_node(&GraphNode {
                id: id.into(),
                name: name.into(),
                statement: statement.into(),
                aliases: vec![],
                created_at,
                provenance: prov.clone(),
            })
            .unwrap();
    }
    store
        .append_chain(&GraphChain {
            id: "arg_alpha_beta_gamma".into(),
            premises: vec!["n_alpha".into(), "n_beta".into()],
            conclusion: "n_gamma".into(),
            provenance: prov,
            created_at,
        })
        .unwrap();

    let backend = Arc::new(DesktopBackend::new(home, data, root.clone()));
    let html = render_until(
        View::Graph,
        false,
        backend,
        &[r#"data-testid="graph-node-n_alpha""#],
    )
    .await;

    assert!(html.contains(r#"data-testid="graph-node-n_beta""#));
    assert!(html.contains(r#"data-testid="graph-node-n_gamma""#));
    assert!(html.contains(r#"data-testid="graph-chain-junction""#));
    assert!(html.contains(r#"data-testid="graph-cluster-hull""#));
    assert!(html.contains("3 propositions · 1 reasoning chains"));

    let _ = std::fs::remove_dir_all(root);
}

#[test]
fn classroom_question_evaluation_and_plan_dag_derivation_roundtrip() {
    let quiz = QuizInput {
        question: "Which structure holds append-only graph truth?".into(),
        options: vec![
            QuizOption {
                label: "nodes.jsonl and chains.jsonl".into(),
                value: "jsonl_ledgers".into(),
            },
            QuizOption {
                label: "Transient browser localStorage".into(),
                value: "local_storage".into(),
            },
        ],
        correct_answer: CorrectAnswer::One("jsonl_ledgers".into()),
        explanation: "JSONL ledgers are the primary truth.".into(),
        multi_select: false,
        kind: Some("check".into()),
        node_id: Some("n1".into()),
    };

    let correct = evaluate_question_submission(&quiz, "A", 5, "notes", "reason");
    assert!(correct.is_correct);
    assert!(!correct.is_idk);
    assert_eq!(correct.status_slug(), "correct");
    assert_eq!(correct.confidence, 5);

    let wrong = evaluate_question_submission(&quiz, "B", 2, "", "");
    assert!(!wrong.is_correct);
    assert_eq!(wrong.status_slug(), "incorrect");

    let idk = evaluate_question_submission(&quiz, "I", 1, "", "");
    assert!(idk.is_idk);
    assert_eq!(idk.status_slug(), "idk");

    let mermaid = "graph TD\n  n1[\"Node 1\"]\n  n2[\"Node 2\"]\n  n1 --> n2";
    let tree = parse_plan(mermaid, 1).unwrap();
    let coords = compute_dag_coordinates(&tree);
    assert_eq!(coords.len(), 2);

    let calls = vec![ToolCall {
        name: "quiz".into(),
        args: heal_encoded_arguments(r#"{"nodeId":"n1"}"#.into()),
    }];
    let statuses = derive_plan_statuses(&tree, &calls);
    assert_eq!(
        PlanNodeStatus::from_statuses(&statuses, "n1"),
        PlanNodeStatus::Current
    );
    assert_eq!(
        PlanNodeStatus::from_statuses(&statuses, "n2"),
        PlanNodeStatus::Upcoming
    );
}

#[test]
fn fsrs_deck_and_digest_fixtures_grade_and_format_deterministically() {
    let mut cards = default_deck_cards();
    assert!(!cards.is_empty());
    let first = &mut cards[0];
    let now_ms = 1_767_258_000_000_i64;
    let ivls = preview(Some(&first.flashcard.card), now_ms);
    assert_eq!(ivls.len(), 4);

    let updated = review(&first.flashcard.card, now_ms + 60_000, Rating::Good);
    assert!(updated.due > now_ms);
    assert_eq!(fsrs_state_label(FsrsState::Review), "Review");
    assert_eq!(rating_label(Rating::Good), "Good");
    assert_eq!(rating_slug(Rating::Good), "good");
    assert!(!format_interval_ms(ivls[2]).is_empty());

    let sessions = default_digest_sessions();
    let sources: BTreeSet<&str> = sessions.iter().map(|s| s.session.source.as_str()).collect();
    assert!(sources.contains("claude"));
    assert!(sources.contains("codex"));
    assert!(sources.contains("agy"));
}

#[test]
fn learnpuppy_app_process_smoke_serves_e2e_liveview_over_tcp() {
    let bin = env!("CARGO_BIN_EXE_learnpuppy-app");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);

    let mut child = Command::new(bin)
        .env("LP_E2E_PORT", port.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("spawn learnpuppy-app in E2E web mode");

    let deadline = Instant::now() + Duration::from_secs(10);
    let mut response = String::new();
    while Instant::now() < deadline {
        if let Ok(mut stream) = TcpStream::connect(("127.0.0.1", port)) {
            let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
            let _ =
                stream.write_all(b"GET / HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n");
            let _ = stream.read_to_string(&mut response);
            if response.starts_with("HTTP/1.1 200") {
                break;
            }
            response.clear();
        }
        std::thread::sleep(Duration::from_millis(50));
    }

    let _ = child.kill();
    let _ = child.wait();

    assert!(
        response.starts_with("HTTP/1.1 200"),
        "expected HTTP 200 from learnpuppy-app E2E server, got: {response}"
    );
    assert!(response.contains("<title>LearnPuppy</title>"));
    assert!(response.contains(r#"id="main""#));
    assert!(response.contains("/ws"));
}
