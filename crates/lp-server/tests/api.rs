//! Endpoint tests for the read and simple-write API (docs/web/03), over `tower::oneshot` on a
//! seeded fixture root with the scripted engine: one happy path and at least one error per
//! endpoint, conditional GETs, idempotent grading, the no-answer-leak rule and archived-workspace
//! mutations.

use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::response::Response;
use axum::Router;
use http_body_util::BodyExt;
use lp_cli_driver::accounts::DiscoverRoots;
use lp_core::course::{Course, Item, ItemKind, Lesson};
use lp_core::ports::{GraphStore, TrackerStore};
use lp_server::app::{router, AppState, EngineMode, ServerConfig};
use lp_server::assets::Assets;
use lp_server::directory::{RegistryDirectory, WorkspaceDirectory};
use lp_server::seed::{self, Fixture, ARCHIVED_WS, DAMAGED_WS, MAIN_WS, SECOND_WS};
use lp_service::events::EventHub;
use lp_service::paths;
use serde_json::{json, Value};
use tower::ServiceExt;

const HOST: &str = "127.0.0.1:7878";
const ORIGIN: &str = "http://127.0.0.1:7878";
const TOKEN: &str = "t0ken";

struct Api {
    _dir: tempfile::TempDir,
    root: std::path::PathBuf,
    state: AppState,
}

fn api_over(root_setup: impl FnOnce(&std::path::Path)) -> Api {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("data");
    root_setup(&root);
    let hub = Arc::new(EventHub::new());
    let directory = RegistryDirectory::open(root.clone(), hub.clone(), root.clone(), None).unwrap();
    directory.core().migrate_on_start();
    let pool = lp_service::accounts::discover_pool(&DiscoverRoots {
        home: root.clone(),
        ..DiscoverRoots::default()
    });
    let state = AppState::new(
        ServerConfig {
            port: 7878,
            token: Some(TOKEN.into()),
            engine: EngineMode::Faux,
            data_root: root.clone(),
            heartbeat: Duration::from_secs(15),
        },
        hub,
        Arc::new(directory),
        pool,
        Assets::Dir(dir.path().join("dist")),
        seed::FIXTURE_DEVICE.into(),
    );
    Api {
        _dir: dir,
        root,
        state,
    }
}

fn api() -> Api {
    api_over(|root| {
        seed::seed(Fixture::Small, root).unwrap();
    })
}

struct Reply {
    status: StatusCode,
    headers: axum::http::HeaderMap,
    text: String,
}

impl Reply {
    fn json(&self) -> Value {
        serde_json::from_str(&self.text).unwrap_or_else(|e| panic!("not json ({e}): {}", self.text))
    }
    fn etag(&self) -> String {
        self.headers[header::ETAG].to_str().unwrap().to_owned()
    }
    fn code(&self) -> String {
        self.json()["error"]["code"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }
}

impl Api {
    fn app(&self) -> Router {
        router(self.state.clone())
    }

    async fn call(
        &self,
        method: Method,
        path: &str,
        body: Option<Value>,
        inm: Option<&str>,
    ) -> Reply {
        let mut b = Request::builder()
            .method(method.clone())
            .uri(format!("/api/v1{path}"))
            .header(header::HOST, HOST)
            .header(header::COOKIE, format!("lp_session={TOKEN}"));
        if method != Method::GET {
            b = b.header(header::ORIGIN, ORIGIN).header("x-lp-csrf", "1");
        }
        if let Some(t) = inm {
            b = b.header(header::IF_NONE_MATCH, t);
        }
        let req = match body {
            Some(v) => b
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(v.to_string())),
            None => b.body(Body::empty()),
        }
        .unwrap();
        let res: Response = self.app().oneshot(req).await.unwrap();
        let (parts, body) = res.into_parts();
        let text = String::from_utf8(body.collect().await.unwrap().to_bytes().to_vec()).unwrap();
        Reply {
            status: parts.status,
            headers: parts.headers,
            text,
        }
    }

    async fn get(&self, path: &str) -> Reply {
        self.call(Method::GET, path, None, None).await
    }
    async fn post(&self, path: &str, body: Value) -> Reply {
        self.call(Method::POST, path, Some(body), None).await
    }
    async fn post_empty(&self, path: &str) -> Reply {
        self.call(Method::POST, path, None, None).await
    }
    async fn patch(&self, path: &str, body: Value) -> Reply {
        self.call(Method::PATCH, path, Some(body), None).await
    }
    async fn put(&self, path: &str, body: Value) -> Reply {
        self.call(Method::PUT, path, Some(body), None).await
    }
    async fn delete(&self, path: &str) -> Reply {
        self.call(Method::DELETE, path, None, None).await
    }

    fn stores(&self, ws: &str) -> lp_service::stores::Stores {
        lp_service::stores::Stores::open(&paths::workspace_dir(&self.root, ws)).unwrap()
    }
}

fn ws(path: &str) -> String {
    format!("/workspaces/{MAIN_WS}{path}")
}

// ------------------------------------------------------------------ auth

#[tokio::test]
async fn new_routes_need_a_session() {
    let a = api();
    let req = Request::get(format!("/api/v1/workspaces/{MAIN_WS}/graph"))
        .header(header::HOST, HOST)
        .body(Body::empty())
        .unwrap();
    let res = a.app().oneshot(req).await.unwrap();
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

// ------------------------------------------------------------------ workspaces

#[tokio::test]
async fn lists_workspaces_with_stats_and_damaged_ones() {
    let a = api();
    let r = a.get("/workspaces").await;
    assert_eq!(r.status, StatusCode::OK);
    let j = r.json();
    let names: Vec<_> = j["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["name"].as_str().unwrap())
        .collect();
    assert_eq!(
        names,
        ["Rust async", "Distributed systems"],
        "archived hidden by default"
    );
    assert_eq!(j["items"][0]["stats"]["nodes"], 40);
    assert_eq!(j["items"][0]["stats"]["chats"], 5);
    assert_eq!(j["items"][0]["sources"]["sinceDays"], 30);
    assert_eq!(j["damaged"][0]["id"], DAMAGED_WS);

    let all = a.get("/workspaces?archived=true").await.json();
    assert_eq!(all["items"].as_array().unwrap().len(), 3);
    let bad = a.get("/workspaces?archived=maybe").await;
    assert_eq!(
        (bad.status, bad.code().as_str()),
        (StatusCode::BAD_REQUEST, "validation")
    );
}

#[tokio::test]
async fn creates_reads_edits_archives_and_restores_a_workspace() {
    let a = api();
    let mut sub = a.state.hub.subscribe(None, None);
    let r = a
        .post(
            "/workspaces",
            json!({"name": "Compilers", "icon": "cpu", "hue": "teal"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED);
    let id = r.json()["id"].as_str().unwrap().to_owned();
    assert!(id.starts_with("ws_"));
    assert!(paths::workspace_dir(&a.root, &id)
        .join("workspace.json")
        .is_file());
    let ev = tokio::time::timeout(Duration::from_secs(2), sub.live.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(format!("{ev:?}").contains("WorkspaceCreated"), "{ev:?}");

    let dup = a.post("/workspaces", json!({"name": "compilers"})).await;
    assert_eq!(
        (dup.status, dup.code().as_str()),
        (StatusCode::CONFLICT, "conflict")
    );
    assert_eq!(dup.json()["error"]["details"]["fields"][0]["path"], "name");
    let bad = a
        .post("/workspaces", json!({"name": "", "icon": "nope"}))
        .await;
    assert_eq!(bad.status, StatusCode::BAD_REQUEST);

    let got = a.get(&format!("/workspaces/{id}")).await;
    assert_eq!(got.json()["name"], "Compilers");
    assert_eq!(got.json()["stats"]["nodes"], 0);
    assert_eq!(
        a.get("/workspaces/ws_nope0000").await.status,
        StatusCode::NOT_FOUND
    );

    let p = a
        .patch(
            &format!("/workspaces/{id}"),
            json!({"name": "Compilers 2", "description": "x"}),
        )
        .await;
    assert_eq!(p.json()["name"], "Compilers 2");
    assert_eq!(p.json()["slug"], "compilers-2");
    let e = a
        .patch(
            &format!("/workspaces/{id}"),
            json!({"overrides": {"profiles": {"teacher": {"provider": "gpt", "model": "m"}}}}),
        )
        .await;
    assert_eq!(e.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        e.json()["error"]["details"]["fields"][0]["path"],
        "overrides.profiles.teacher"
    );

    let arch = a.post_empty(&format!("/workspaces/{id}/archive")).await;
    assert_eq!(arch.json()["archived"], true);
    let listed = a.get("/workspaces").await.json();
    assert!(!listed["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|w| w["id"] == id));
    let back = a.post_empty(&format!("/workspaces/{id}/restore")).await;
    assert_eq!(back.json()["archived"], false);
}

#[tokio::test]
async fn archived_workspaces_refuse_mutations_with_409() {
    let a = api();
    let w = |p: &str| format!("/workspaces/{ARCHIVED_WS}{p}");
    let r = a.patch(&w(""), json!({"name": "Renamed"})).await;
    assert_eq!(
        (r.status, r.code().as_str()),
        (StatusCode::CONFLICT, "workspace_archived")
    );

    let chat = a.stores(ARCHIVED_WS);
    let card = chat.tracker.cards().unwrap()[0].id.to_string();
    let g = a
        .post(
            &w(&format!("/review/{card}/grade")),
            json!({"rating": 3, "clientId": "c1"}),
        )
        .await;
    assert_eq!(
        (g.status, g.code().as_str()),
        (StatusCode::CONFLICT, "workspace_archived")
    );
    let s = a.post_empty(&w("/review/sync")).await;
    assert_eq!(s.code(), "workspace_archived");
    let d = a.post_empty(&w("/digest/decisions/d_0000/cards")).await;
    assert_eq!(d.code(), "workspace_archived");
    // Reads still work.
    assert_eq!(a.get(&w("/graph")).await.status, StatusCode::OK);
    assert_eq!(a.get(&w("/review/stats")).await.status, StatusCode::OK);
}

#[tokio::test]
async fn deleting_needs_the_exact_name_then_trash_restores_and_purges() {
    let a = api();
    let w = format!("/workspaces/{SECOND_WS}");
    let wrong = a
        .call(
            Method::DELETE,
            &w,
            Some(json!({"confirmName": "nope"})),
            None,
        )
        .await;
    assert_eq!(
        (wrong.status, wrong.code().as_str()),
        (StatusCode::BAD_REQUEST, "validation")
    );
    assert!(paths::workspace_dir(&a.root, SECOND_WS).is_dir());

    let ok = a
        .call(
            Method::DELETE,
            &w,
            Some(json!({"confirmName": "Distributed systems"})),
            None,
        )
        .await;
    assert_eq!(ok.status, StatusCode::OK);
    assert!(ok.json()["bytes"].as_u64().unwrap() > 0);
    assert_eq!(a.get(&w).await.status, StatusCode::NOT_FOUND);

    let trash = a.get("/trash").await.json();
    let items = trash["items"].as_array().unwrap();
    assert_eq!(
        items.len(),
        2,
        "the fixture's trashed workspace plus this one"
    );
    let key = ok.json()["id"].as_str().unwrap().to_owned();
    let restored = a.post_empty(&format!("/trash/{key}/restore")).await;
    assert_eq!(restored.status, StatusCode::OK);
    assert_eq!(restored.json()["name"], "Distributed systems");

    let scratch = items.iter().find(|i| i["name"] == "Scratch notes").unwrap()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        a.delete(&format!("/trash/{scratch}")).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        a.delete(&format!("/trash/{scratch}")).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        a.post_empty("/trash/nonsense/restore").await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn repair_fixes_only_damaged_workspaces() {
    let a = api();
    let r = a
        .post_empty(&format!("/workspaces/{DAMAGED_WS}/repair"))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    assert_eq!(r.json()["name"], DAMAGED_WS);
    let again = a
        .post_empty(&format!("/workspaces/{DAMAGED_WS}/repair"))
        .await;
    assert_eq!(again.status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn bootstrap_reports_real_workspaces_accounts_and_the_migration_banner() {
    let a = api_over(|root| {
        std::fs::create_dir_all(root.join("graph")).unwrap();
        std::fs::write(root.join("graph/nodes.jsonl"), "").unwrap();
        std::fs::create_dir_all(root.join("chats")).unwrap();
    });
    let j = a.get("/bootstrap").await.json();
    assert_eq!(j["workspaces"].as_array().unwrap().len(), 1);
    assert_eq!(j["workspaces"][0]["name"], "Default");
    assert_eq!(j["banners"][0]["id"], "migrated-v1");
    assert!(a.root.join("workspaces").is_dir() && !a.root.join("graph").exists());

    let seeded = api();
    let j = seeded.get("/bootstrap").await.json();
    assert_eq!(j["workspaces"].as_array().unwrap().len(), 3);
    assert_eq!(j["lastWorkspaceId"], MAIN_WS);
    assert_eq!(j["accounts"].as_array().unwrap().len(), 2);
    assert!(j["banners"].as_array().unwrap().is_empty());
}

// ------------------------------------------------------------------ graph

#[tokio::test]
async fn graph_snapshot_carries_positions_clusters_and_an_etag() {
    let a = api();
    let r = a.get(&ws("/graph")).await;
    assert_eq!(r.status, StatusCode::OK);
    let j = r.json();
    assert_eq!(j["nodes"].as_array().unwrap().len(), 40);
    assert!(j["chains"].as_array().unwrap().len() > 30);
    assert!(!j["clusters"].as_array().unwrap().is_empty());
    assert_eq!(j["layoutPending"], false);
    assert_eq!(j["layoutVersion"], j["version"]);
    assert_eq!(j["positions"].as_object().unwrap().len(), 40);
    let n = &j["nodes"][0];
    for k in [
        "id",
        "name",
        "text",
        "kind",
        "sessionId",
        "provenance",
        "createdAt",
        "degree",
    ] {
        assert!(n.get(k).is_some(), "{k}");
    }

    let etag = r.etag();
    let again = a.call(Method::GET, &ws("/graph"), None, Some(&etag)).await;
    assert_eq!(again.status, StatusCode::NOT_MODIFIED);
    assert!(again.text.is_empty());
    assert_eq!(again.etag(), etag);
    let stale = a
        .call(Method::GET, &ws("/graph"), None, Some("\"other\""))
        .await;
    assert_eq!(stale.status, StatusCode::OK);

    assert_eq!(
        a.get("/workspaces/ws_nope0000/graph").await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn graph_layout_is_computed_in_the_background_then_cached() {
    let a = api();
    let pos = paths::workspace_dir(&a.root, MAIN_WS).join("graph/positions.json");
    std::fs::remove_file(&pos).unwrap();
    let mut sub = a.state.hub.subscribe(Some(MAIN_WS), None);

    let first = a.get(&ws("/graph")).await;
    let j = first.json();
    assert_eq!(
        (j["layoutPending"].as_bool(), j["positions"].is_null()),
        (Some(true), true)
    );
    assert_eq!(
        a.get(&ws("/graph")).await.etag(),
        first.etag(),
        "same state, same tag"
    );

    // `graph.changed` arrives once positions.json is on disk.
    let ev = loop {
        let ev = tokio::time::timeout(Duration::from_secs(20), sub.live.recv())
            .await
            .unwrap()
            .unwrap();
        if format!("{ev:?}").contains("GraphChanged") {
            break ev;
        }
    };
    assert!(
        format!("{ev:?}").contains(&j["version"].to_string()),
        "{ev:?}"
    );
    assert!(pos.is_file());
    let ready = a.get(&ws("/graph")).await;
    assert_eq!(ready.json()["layoutPending"], false);
    assert_eq!(ready.json()["positions"].as_object().unwrap().len(), 40);
    assert_ne!(ready.etag(), first.etag());
}

#[tokio::test]
async fn graph_changes_bump_the_version_and_drop_the_stale_layout() {
    let a = api();
    let v1 = a.get(&ws("/graph")).await.json()["version"].clone();
    let s = a.stores(MAIN_WS);
    s.graph
        .append_node(&lp_core::graph::GraphNode {
            id: "n_extra".into(),
            name: "Extra".into(),
            statement: "An extra proposition.".into(),
            aliases: vec![],
            created_at: chrono::DateTime::UNIX_EPOCH,
            provenance: vec![],
        })
        .unwrap();
    let r = a.get(&ws("/graph")).await.json();
    assert_ne!(r["version"], v1);
    assert_eq!(r["nodes"].as_array().unwrap().len(), 41);
    assert_eq!(
        r["layoutPending"], true,
        "old positions belong to the old version"
    );
}

#[tokio::test]
async fn node_detail_search_and_chain_table() {
    let a = api();
    let g = a.get(&ws("/graph")).await.json();
    let chain = &g["chains"][0];
    let conclusion = chain["to"].as_str().unwrap().to_owned();

    let d = a.get(&ws(&format!("/graph/nodes/{conclusion}"))).await;
    assert_eq!(d.status, StatusCode::OK);
    let dj = d.json();
    assert_eq!(dj["id"], conclusion.as_str());
    assert!(!dj["incoming"].as_array().unwrap().is_empty());
    assert!(!dj["provenanceDetail"].as_array().unwrap().is_empty());
    for k in ["outgoing", "related", "cards", "aliases", "mastery"] {
        assert!(dj.get(k).is_some(), "{k}");
    }
    let missing = a.get(&ws("/graph/nodes/n_missing")).await;
    assert_eq!(
        (missing.status, missing.code().as_str()),
        (StatusCode::NOT_FOUND, "not_found")
    );

    let name = g["nodes"][3]["name"].as_str().unwrap();
    let word = name
        .split(|c: char| !c.is_alphanumeric())
        .find(|w| w.len() >= 3)
        .expect("node name has an alphanumeric word")
        .to_lowercase();
    let s = a.get(&ws(&format!("/graph/search?q={word}&limit=5"))).await;
    assert_eq!(s.status, StatusCode::OK, "{}", s.text);
    assert!(
        !s.json()["items"].as_array().unwrap().is_empty(),
        "word={word:?} name={name:?} resp={}",
        s.text
    );
    assert!(s.json()["items"].as_array().unwrap().len() <= 5);
    let empty = a.get(&ws("/graph/search?q=%20")).await;
    assert_eq!(
        (empty.status, empty.code().as_str()),
        (StatusCode::BAD_REQUEST, "validation")
    );

    let t = a
        .get(&ws(&format!("/graph/chains?selection={conclusion}")))
        .await;
    assert_eq!(t.status, StatusCode::OK);
    let tj = t.json();
    assert!(tj["total"].as_u64().unwrap() >= 1);
    assert_eq!(tj["selection"][0], conclusion.as_str());
    assert!(!tj["groups"][0]["chains"][0]["nodes"]
        .as_array()
        .unwrap()
        .is_empty());
    let none = a.get(&ws("/graph/chains")).await.json();
    assert_eq!(none["total"], 0);
}

// ------------------------------------------------------------------ chats

#[tokio::test]
async fn lists_chats_with_phase_pending_and_recovered_jobs() {
    let a = api();
    let r = a.get(&ws("/chats")).await;
    assert_eq!(r.status, StatusCode::OK);
    let j = r.json();
    assert_eq!(j["total"], 5);
    let by = |id: &str| {
        j["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|c| c["id"] == id)
            .unwrap()
            .clone()
    };
    assert_eq!(by("chat_probe")["phase"], "probe");
    assert_eq!(by("chat_plan")["phase"], "plan");
    assert_eq!(by("chat_teach")["phase"], "teach");
    assert_eq!(by("chat_practice")["phase"], "practice");
    assert_eq!(by("chat_done")["phase"], "done");
    assert_eq!(by("chat_teach")["pending"], true);
    assert_eq!(
        by("chat_teach")["jobs"],
        0,
        "the pre-restart job was recovered"
    );
    assert_eq!(by("chat_done")["pending"], false);
    let times: Vec<_> = j["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["updatedAt"].as_i64().unwrap())
        .collect();
    assert!(
        times.windows(2).all(|w| w[0] >= w[1]),
        "newest first: {times:?}"
    );

    let page = a.get(&ws("/chats?limit=2")).await.json();
    assert_eq!(page["items"].as_array().unwrap().len(), 2);
    assert_eq!(page["nextCursor"], "2");
    let next = a.get(&ws("/chats?limit=2&cursor=2")).await.json();
    assert_eq!(next["items"].as_array().unwrap().len(), 2);
    assert_eq!(
        a.get(&ws("/chats?cursor=zzz")).await.status,
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn classroom_view_has_the_documented_shape() {
    let a = api();
    let r = a.get(&ws("/chats/chat_teach")).await;
    assert_eq!(r.status, StatusCode::OK);
    let j = r.json();
    for k in [
        "id",
        "kind",
        "title",
        "phase",
        "viewVersion",
        "messages",
        "pending",
        "plan",
        "probeProgress",
        "verdicts",
        "jobs",
        "turn",
    ] {
        assert!(j.get(k).is_some(), "{k} in {j}");
    }
    assert_eq!(j["phase"], "teach");
    assert_eq!(j["turn"]["state"], "idle");
    assert_eq!(j["probeProgress"]["max"], 8);
    assert_eq!(j["plan"]["currentId"], "n1");
    let statuses: Vec<_> = j["plan"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| n["status"].as_str().unwrap())
        .collect();
    assert_eq!(statuses, ["current", "upcoming", "upcoming"]);
    assert_eq!(j["pending"]["tool"], "quiz");
    assert_eq!(j["pending"]["options"].as_array().unwrap().len(), 3);
    assert_eq!(
        j["verdicts"].as_array().unwrap().len(),
        1,
        "the answered probe"
    );
    assert_eq!(j["jobs"].as_array().unwrap().len(), 5);
    assert!(j["jobs"]
        .as_array()
        .unwrap()
        .iter()
        .any(|x| x["id"] == "job_research_run" && x["status"] == "done"));

    let done = a.get(&ws("/chats/chat_done")).await.json();
    assert!(done["pending"].is_null());
    assert!(done["plan"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .all(|n| n["status"] != "upcoming"));
    let practice = a.get(&ws("/chats/chat_practice")).await.json();
    assert_eq!(practice["pending"]["tool"], "problem");
    assert_eq!(practice["pending"]["problem"]["mode"], "composition");

    assert_eq!(
        a.get(&ws("/chats/nope")).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        a.get(&ws("/chats/..%2Fsettings")).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn pending_questions_never_leak_the_answer_over_the_wire() {
    let a = api();
    for (chat, secrets) in [
        ("chat_teach", ["correctAnswer", "final owner runs the drop"]),
        (
            "chat_probe",
            ["correctAnswer", "executor decides; the future only reports"],
        ),
        ("chat_practice", ["modelSolution", "Poll each until Ready"]),
    ] {
        let r = a.get(&ws(&format!("/chats/{chat}"))).await;
        assert!(
            r.json()["pending"].is_object(),
            "{chat} has a pending question"
        );
        for s in secrets {
            assert!(!r.text.contains(s), "{chat} leaked {s}");
        }
        let pending = r.json()["pending"].clone();
        let keys: Vec<_> = pending.as_object().unwrap().keys().cloned().collect();
        assert!(
            !keys.iter().any(|k| k.to_lowercase().contains("correct")),
            "{keys:?}"
        );
        for o in pending["options"].as_array().into_iter().flatten() {
            let ok: Vec<_> = o.as_object().unwrap().keys().cloned().collect();
            assert!(
                !ok.iter().any(|k| k.to_lowercase().contains("correct")),
                "{ok:?}"
            );
        }
    }
    // The chat list summarises without any question text at all.
    let list = a.get(&ws("/chats")).await;
    assert!(!list.text.contains("final owner runs the drop"));
}

#[tokio::test]
async fn renaming_and_deleting_chats() {
    let a = api();
    let r = a
        .patch(
            &ws("/chats/chat_done"),
            json!({"title": "  Send bounds, revisited  "}),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK);
    assert_eq!(r.json()["title"], "Send bounds, revisited");
    let view = a.get(&ws("/chats/chat_done")).await.json();
    assert_eq!(view["title"], "Send bounds, revisited");
    let list = a.get(&ws("/chats")).await.json();
    assert_eq!(
        list["items"][0]["id"], "chat_done",
        "renaming touches updatedAt"
    );

    let empty = a
        .patch(&ws("/chats/chat_done"), json!({"title": "   "}))
        .await;
    assert_eq!(
        (empty.status, empty.code().as_str()),
        (StatusCode::BAD_REQUEST, "validation")
    );
    let long = a
        .patch(&ws("/chats/chat_done"), json!({"title": "x".repeat(121)}))
        .await;
    assert_eq!(long.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        a.patch(&ws("/chats/nope"), json!({"title": "t"}))
            .await
            .status,
        StatusCode::NOT_FOUND
    );

    let del = a.delete(&ws("/chats/chat_done")).await;
    assert_eq!(del.status, StatusCode::NO_CONTENT);
    let dir = paths::workspace_dir(&a.root, MAIN_WS).join("chats");
    assert!(!dir.join("chat_done.json").exists());
    assert!(dir.join(".trash/chat_done.json").is_file());
    assert_eq!(
        a.get(&ws("/chats/chat_done")).await.status,
        StatusCode::NOT_FOUND
    );
    assert_eq!(a.get(&ws("/chats")).await.json()["total"], 4);
    assert_eq!(
        a.delete(&ws("/chats/chat_done")).await.status,
        StatusCode::NOT_FOUND
    );

    let w = format!("/workspaces/{ARCHIVED_WS}/chats/x");
    assert_eq!(a.delete(&w).await.code(), "workspace_archived");
}

// ------------------------------------------------------------------ review

#[tokio::test]
async fn review_queue_withholds_answers() {
    let a = api();
    let r = a.get(&ws("/review/queue?limit=3")).await;
    assert_eq!(r.status, StatusCode::OK);
    let j = r.json();
    assert_eq!(j["items"].as_array().unwrap().len(), 3);
    assert!(j["dueNow"].as_u64().unwrap() >= 3);
    assert!(j["now"].as_i64().unwrap() > 1_700_000_000_000);
    let c = &j["items"][0];
    assert_eq!(c["options"].as_array().unwrap().len(), 4);
    assert_eq!(c["options"][0]["key"], "a");
    assert_eq!(c["kind"], "decision");
    for banned in ["correct", "rationale", "explanation"] {
        assert!(!r.text.to_lowercase().contains(banned), "{banned} in queue");
    }
    let dues: Vec<_> = j["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["due"].as_i64().unwrap())
        .collect();
    assert!(dues.windows(2).all(|w| w[0] <= w[1]), "oldest due first");
    assert_eq!(
        a.get("/workspaces/ws_nope0000/review/queue").await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn answer_reveals_correctness_without_changing_the_schedule() {
    let a = api();
    let card = a.get(&ws("/review/queue?limit=1")).await.json()["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let before = a.stores(MAIN_WS).tracker.reviews().unwrap().len();
    let detail = a.get(&ws(&format!("/review/cards/{card}"))).await.json();
    let right = detail["correctKey"].as_str().unwrap().to_owned();

    let r = a
        .post(
            &ws(&format!("/review/{card}/answer")),
            json!({"choiceKey": right, "clientId": "a1", "confidence": 5, "latencyMs": 4000}),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    let j = r.json();
    assert_eq!(j["correct"], true);
    assert_eq!(j["correctKey"], right.as_str());
    assert!(!j["explanation"].as_str().unwrap().is_empty());
    assert!(j["rating"].as_u64().unwrap() >= 3);
    assert_eq!(j["preview"].as_array().unwrap().len(), 4);
    assert!(j["next"]["intervalDays"].as_f64().unwrap() >= 0.0);

    let wrong = ["a", "b", "c", "d"]
        .into_iter()
        .find(|k| *k != right)
        .unwrap();
    let w = a
        .post(
            &ws(&format!("/review/{card}/answer")),
            json!({"choiceKey": wrong, "clientId": "a2"}),
        )
        .await;
    assert_eq!(w.json()["correct"], false);
    assert_eq!(w.json()["rating"], 1);
    assert_eq!(
        a.stores(MAIN_WS).tracker.reviews().unwrap().len(),
        before,
        "answer never grades"
    );

    let bad = a
        .post(
            &ws(&format!("/review/{card}/answer")),
            json!({"choiceKey": "z", "clientId": "a3"}),
        )
        .await;
    assert_eq!(
        (bad.status, bad.code().as_str()),
        (StatusCode::BAD_REQUEST, "validation")
    );
    let conf = a
        .post(
            &ws(&format!("/review/{card}/answer")),
            json!({"choiceKey": "a", "clientId": "a4", "confidence": 9}),
        )
        .await;
    assert_eq!(conf.status, StatusCode::BAD_REQUEST);
    let none = a
        .post(
            &ws("/review/card_nope/answer"),
            json!({"choiceKey": "a", "clientId": "a5"}),
        )
        .await;
    assert_eq!(none.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn grading_is_idempotent_by_client_id_and_atomic() {
    let a = api();
    let card = a.get(&ws("/review/queue?limit=1")).await.json()["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let t = a.stores(MAIN_WS).tracker;
    let (reviews0, reps0) = (
        t.reviews().unwrap().len(),
        t.card(&card).unwrap().unwrap().card.reps,
    );
    let path = ws(&format!("/review/{card}/grade"));

    let first = a.post(&path, json!({"rating": 3, "clientId": "g-1"})).await;
    assert_eq!(first.status, StatusCode::OK, "{}", first.text);
    let again = a.post(&path, json!({"rating": 3, "clientId": "g-1"})).await;
    assert_eq!(again.text, first.text, "a retry returns the first result");
    assert_eq!(t.reviews().unwrap().len(), reviews0 + 1, "graded once");
    assert_eq!(t.card(&card).unwrap().unwrap().card.reps, reps0 + 1);
    let j = first.json();
    assert_eq!(j["card"]["id"], card.as_str());
    assert_eq!(j["card"]["reps"], reps0 + 1);
    assert_eq!(j["preview"].as_array().unwrap().len(), 4);

    // The same client id on another card is a different mutation.
    let other = a.get(&ws("/review/queue?limit=2")).await.json()["items"][1]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let o = a
        .post(
            &ws(&format!("/review/{other}/grade")),
            json!({"rating": 2, "clientId": "g-1"}),
        )
        .await;
    assert_eq!(o.status, StatusCode::OK);
    assert_ne!(o.json()["card"]["id"], first.json()["card"]["id"]);
    // A new client id grades again.
    let second = a.post(&path, json!({"rating": 4, "clientId": "g-2"})).await;
    assert_eq!(second.json()["card"]["reps"], reps0 + 2);

    // The grade is logged for sync too.
    let sync_file = paths::workspace_dir(&a.root, MAIN_WS)
        .join(format!("tracker/sync-{}.jsonl", seed::FIXTURE_DEVICE));
    assert!(std::fs::read_to_string(sync_file)
        .unwrap()
        .contains(&format!("\"card_id\":\"{card}\"")));

    for bad in [
        json!({"rating": 0, "clientId": "x"}),
        json!({"rating": 5, "clientId": "x"}),
        json!({"rating": 3, "clientId": ""}),
    ] {
        let r = a.post(&path, bad).await;
        assert_eq!(
            (r.status, r.code().as_str()),
            (StatusCode::BAD_REQUEST, "validation")
        );
    }
    let gone = a
        .post(
            &ws("/review/card_nope/grade"),
            json!({"rating": 3, "clientId": "n1"}),
        )
        .await;
    assert_eq!(gone.status, StatusCode::NOT_FOUND);
    let wrong_body = a.post(&path, json!({"rating": "good"})).await;
    assert_eq!(wrong_body.status, StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn concurrent_retries_of_one_grade_apply_it_once() {
    let a = api();
    let card = a.get(&ws("/review/queue?limit=1")).await.json()["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let before = a.stores(MAIN_WS).tracker.reviews().unwrap().len();
    let path = ws(&format!("/review/{card}/grade"));
    let body = json!({"rating": 3, "clientId": "race"});
    let rs = futures::future::join_all((0..6).map(|_| a.post(&path, body.clone()))).await;
    assert!(rs.iter().all(|r| r.status == StatusCode::OK));
    assert!(rs.windows(2).all(|w| w[0].text == w[1].text));
    assert_eq!(
        a.stores(MAIN_WS).tracker.reviews().unwrap().len(),
        before + 1
    );
}

#[tokio::test]
async fn preview_stats_browse_detail_and_sync() {
    let a = api();
    let card = a.get(&ws("/review/queue?limit=1")).await.json()["items"][0]["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let p = a.get(&ws(&format!("/review/preview/{card}"))).await;
    let labels: Vec<_> = p.json()["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["label"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(labels, ["again", "hard", "good", "easy"]);
    assert_eq!(
        a.get(&ws("/review/preview/card_nope")).await.status,
        StatusCode::NOT_FOUND
    );

    let s = a.get(&ws("/review/stats")).await;
    assert_eq!(s.status, StatusCode::OK);
    let sj = s.json();
    assert_eq!(sj["forecast"].as_array().unwrap().len(), 14);
    for k in [
        "dueNow",
        "dueToday",
        "newToday",
        "streak",
        "reviewedToday",
        "retention30d",
        "forecastStart",
        "now",
    ] {
        assert!(sj.get(k).is_some(), "{k}");
    }
    let etag = s.etag();
    let cached = a
        .call(Method::GET, &ws("/review/stats"), None, Some(&etag))
        .await;
    assert_eq!(cached.status, StatusCode::NOT_MODIFIED);
    // Grading changes the numbers, so the tag moves.
    a.post(
        &ws(&format!("/review/{card}/grade")),
        json!({"rating": 3, "clientId": "st-1"}),
    )
    .await;
    let moved = a
        .call(Method::GET, &ws("/review/stats"), None, Some(&etag))
        .await;
    assert_eq!(moved.status, StatusCode::OK);
    assert_ne!(moved.etag(), etag);
    assert!(moved.json()["reviewedToday"].as_u64() >= sj["reviewedToday"].as_u64());

    let all = a.get(&ws("/review/cards?limit=5")).await.json();
    assert_eq!(all["total"], 24);
    assert_eq!(all["items"].as_array().unwrap().len(), 5);
    assert_eq!(all["nextCursor"], "5");
    assert!(
        all["items"][0].get("rationale").is_none(),
        "lists carry no answers"
    );
    let new = a.get(&ws("/review/cards?state=new")).await.json();
    assert!(new["total"].as_u64().unwrap() > 0);
    assert!(new["items"]
        .as_array()
        .unwrap()
        .iter()
        .all(|c| c["state"] == "new"));
    let q = a.get(&ws("/review/cards?q=WHY%20DID")).await.json();
    assert_eq!(q["total"], 24, "case-insensitive substring over the front");
    assert_eq!(
        a.get(&ws("/review/cards?state=sleepy")).await.code(),
        "validation"
    );

    let d = a.get(&ws(&format!("/review/cards/{card}"))).await.json();
    assert!(d["correctKey"].is_string() && d["rationale"].is_string());
    assert!(!d["history"].as_array().unwrap().is_empty());
    assert_eq!(
        a.get(&ws("/review/cards/card_nope")).await.status,
        StatusCode::NOT_FOUND
    );

    let sync = a.post_empty(&ws("/review/sync")).await;
    assert_eq!(sync.status, StatusCode::OK);
    assert_eq!(sync.json()["cards"], 24);
    assert!(sync.json()["reviews"].as_u64().unwrap() > 0);
}

// ------------------------------------------------------------------ digest

#[tokio::test]
async fn digest_groups_by_day_and_session() {
    let a = api();
    let r = a.get(&ws("/digest")).await;
    assert_eq!(r.status, StatusCode::OK);
    let j = r.json();
    assert_eq!(j["totalDecisions"], 8);
    let days = j["days"].as_array().unwrap();
    assert!(days.len() >= 2);
    let day_names: Vec<_> = days
        .iter()
        .map(|d| d["day"].as_str().unwrap().to_owned())
        .collect();
    assert!(
        day_names.windows(2).all(|w| w[0] >= w[1]),
        "newest day first: {day_names:?}"
    );
    let s = &days[0]["sessions"][0];
    for k in ["sessionId", "source", "project", "summary", "decisions"] {
        assert!(s.get(k).is_some(), "{k}");
    }
    let d = &s["decisions"][0];
    for k in ["id", "title", "basis", "alternatives", "chosen", "cardIds"] {
        assert!(d.get(k).is_some(), "{k}");
    }
    assert!(!d["cardIds"].as_array().unwrap().is_empty());

    let claude = a.get(&ws("/digest?source=claude")).await.json();
    assert!(claude["totalDecisions"].as_u64().unwrap() < 8);
    let none = a.get(&ws("/digest?to=2001-01-01")).await.json();
    assert_eq!(none["totalDecisions"], 0);
    let ms = a
        .get(&ws(&format!("/digest?from={}", seed::FIXTURE_NOW + 1)))
        .await
        .json();
    assert_eq!(ms["totalDecisions"], 0);
    for q in ["from=yesterday", "source=emacs"] {
        assert_eq!(
            a.get(&ws(&format!("/digest?{q}"))).await.code(),
            "validation",
            "{q}"
        );
    }
}

#[tokio::test]
async fn decision_detail_lists_cards_and_session_and_regenerates_cards() {
    let a = api();
    let r = a.get(&ws("/digest/decisions/d_0000")).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    let j = r.json();
    assert_eq!(j["decision"]["id"], "d_0000");
    assert!(j["session"]["cwd"]
        .as_str()
        .unwrap()
        .starts_with("/Users/ada/code/"));
    assert_eq!(
        j["commits"],
        json!([]),
        "the session is outside the allow-list"
    );
    assert!(j["commitsError"].is_null());
    assert!(!j["cards"].as_array().unwrap().is_empty());
    assert_eq!(
        a.get(&ws("/digest/decisions/nope")).await.code(),
        "not_found"
    );

    a.state.faux().faux_structured(json!({"cards": [
        {"front": "Why bound the channel?", "right": "Overload must be visible", "wrong": ["It is faster", "It is smaller", "It is encrypted"], "rationale": "r"},
        {"front": "Same option twice?", "right": "x", "wrong": ["x", "y", "z"], "rationale": "r"}
    ]}));
    let before = a.stores(MAIN_WS).tracker.cards().unwrap().len();
    let g = a.post_empty(&ws("/digest/decisions/d_0000/cards")).await;
    assert_eq!(g.status, StatusCode::OK, "{}", g.text);
    let gj = g.json();
    assert_eq!(gj["saved"], 1);
    assert_eq!(gj["rejected"][0]["front"], "Same option twice?");
    assert!(gj["rejected"][0]["reason"]
        .as_str()
        .unwrap()
        .contains("distinct"));
    assert_eq!(a.stores(MAIN_WS).tracker.cards().unwrap().len(), before + 1);
    let after = a.get(&ws("/digest/decisions/d_0000")).await.json();
    assert!(after["cards"].as_array().unwrap().len() > j["cards"].as_array().unwrap().len());

    assert_eq!(
        a.post_empty(&ws("/digest/decisions/nope/cards"))
            .await
            .code(),
        "not_found"
    );
    // Nothing scripted is left: the engine fails and the API says so.
    let dead = a.post_empty(&ws("/digest/decisions/d_0001/cards")).await;
    assert_eq!(
        (dead.status, dead.code().as_str()),
        (StatusCode::SERVICE_UNAVAILABLE, "engine_unavailable")
    );
}

// ------------------------------------------------------------------ accounts and settings

#[tokio::test]
async fn accounts_list_carries_no_secrets_and_supports_conditional_get() {
    let a = api();
    let r = a.get("/accounts").await;
    assert_eq!(r.status, StatusCode::OK);
    let j = r.json();
    let ids: Vec<_> = j["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(ids, ["claude:ada@example.com", "claude:ada@work.example"]);
    let item = &j["items"][0];
    assert_eq!(
        (item["provider"].as_str(), item["status"].as_str()),
        (Some("claude"), Some("ready"))
    );
    for banned in [
        "secret",
        "keychain",
        "oauth",
        "accesstoken",
        "password",
        "/data/",
    ] {
        assert!(
            !r.text.to_lowercase().contains(&banned.to_lowercase()),
            "{banned} in {}",
            r.text
        );
    }
    assert!(
        !r.text.contains(a.root.to_str().unwrap()),
        "no config dir paths"
    );

    let etag = r.etag();
    let cached = a.call(Method::GET, "/accounts", None, Some(&etag)).await;
    assert_eq!(cached.status, StatusCode::NOT_MODIFIED);
}

#[tokio::test]
async fn accounts_can_be_disabled_rescanned_and_added() {
    let a = api();
    let mut sub = a.state.hub.subscribe(None, None);
    let off = a
        .patch(
            "/accounts/claude:ada@work.example",
            json!({"disabled": true}),
        )
        .await;
    assert_eq!(off.status, StatusCode::OK, "{}", off.text);
    assert_eq!(off.json()["status"], "disabled");
    let ev = tokio::time::timeout(Duration::from_secs(2), sub.live.recv())
        .await
        .unwrap()
        .unwrap();
    assert!(format!("{ev:?}").contains("AccountsChanged"));
    assert!(lp_service::settings::Settings::load(&a.root)
        .unwrap()
        .disabled_accounts
        .contains(&"claude:ada@work.example".to_owned()));
    let on = a
        .patch(
            "/accounts/claude:ada@work.example",
            json!({"disabled": false}),
        )
        .await;
    assert_eq!(on.json()["status"], "ready");
    assert_eq!(
        a.patch("/accounts/claude:nobody", json!({"disabled": true}))
            .await
            .code(),
        "not_found"
    );
    assert_eq!(
        a.patch("/accounts/claude:ada@work.example", json!({}))
            .await
            .status,
        StatusCode::BAD_REQUEST
    );

    // A profile that appears after startup is found by rescan.
    let extra = a.root.join("elsewhere");
    std::fs::create_dir_all(&extra).unwrap();
    std::fs::write(
        extra.join(".claude.json"),
        r#"{"oauthAccount":{"emailAddress":"new@x.com","userRateLimitTier":"default_claude_pro"}}"#,
    )
    .unwrap();
    let add = a
        .post("/accounts/claude", json!({"path": extra.to_str().unwrap()}))
        .await;
    assert_eq!(add.status, StatusCode::OK, "{}", add.text);
    assert_eq!(add.json()["email"], "new@x.com");
    let listed = a.get("/accounts").await.json();
    let claude = listed["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["provider"] == "claude")
        .count();
    assert_eq!(claude, 3, "{listed}");
    let empty = a.root.join("empty");
    std::fs::create_dir_all(&empty).unwrap();
    let bad = a
        .post("/accounts/claude", json!({"path": empty.to_str().unwrap()}))
        .await;
    assert_eq!(
        (bad.status, bad.code().as_str()),
        (StatusCode::BAD_REQUEST, "validation")
    );
    let missing = a
        .post("/accounts/claude", json!({"path": "/no/such/dir"}))
        .await;
    assert_eq!(missing.status, StatusCode::BAD_REQUEST);

    let rs = a.post_empty("/accounts/rescan").await;
    assert_eq!(rs.status, StatusCode::OK);
    let claude_rows = rs.json()["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|i| i["provider"] == "claude")
        .count();
    assert_eq!(claude_rows, 3);
    assert_eq!(rs.json()["added"], json!([]));
}

#[tokio::test]
async fn settings_round_trip_with_catalog_validation() {
    let a = api();
    let cat = a.get("/settings/catalog").await.json();
    let providers: Vec<_> = cat["providers"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["id"].as_str().unwrap().to_owned())
        .collect();
    assert!(providers.contains(&"claude".to_owned()));
    let claude_models = cat["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["id"] == "claude")
        .unwrap()["models"]
        .as_array()
        .unwrap()
        .clone();
    assert!(!claude_models.is_empty());
    assert!(cat["efforts"].as_array().unwrap().contains(&json!("high")));

    let mut s = a.get("/settings").await.json();
    assert_eq!(s["concurrencyCap"], 2);
    s["profiles"]["teacher"] = json!({"provider": "claude", "model": claude_models[0], "effort": "high", "fallback": true});
    s["pins"]["teacher"] = json!("claude:ada@example.com");
    s["concurrencyCap"] = json!(4);
    s["ui"]["theme"] = json!("light");
    let put = a.put("/settings", s.clone()).await;
    assert_eq!(put.status, StatusCode::OK, "{}", put.text);
    assert_eq!(put.json(), a.get("/settings").await.json());
    assert_eq!(put.json()["profiles"]["teacher"]["effort"], "high");
    assert_eq!(put.json()["concurrencyCap"], 4);
    // Unrelated stored settings survive.
    assert_eq!(
        lp_service::settings::Settings::load(&a.root)
            .unwrap()
            .device_id,
        seed::FIXTURE_DEVICE
    );

    let mut bad = s.clone();
    bad["profiles"]["teacher"]["model"] = json!("gpt-99");
    bad["pins"]["prober"] = json!("claude:ghost@nowhere");
    bad["concurrencyCap"] = json!(99);
    let r = a.put("/settings", bad).await;
    assert_eq!(
        (r.status, r.code().as_str()),
        (StatusCode::BAD_REQUEST, "validation")
    );
    let paths_: Vec<_> = r.json()["error"]["details"]["fields"]
        .as_array()
        .unwrap()
        .iter()
        .map(|f| f["path"].as_str().unwrap().to_owned())
        .collect();
    for p in ["profiles.teacher.model", "pins.prober", "concurrencyCap"] {
        assert!(paths_.contains(&p.to_owned()), "{p} in {paths_:?}");
    }
    assert_eq!(
        a.get("/settings").await.json()["concurrencyCap"],
        4,
        "a rejected PUT changes nothing"
    );
    assert_eq!(
        a.put("/settings", json!({"profiles": 3})).await.status,
        StatusCode::BAD_REQUEST
    );
}

// ------------------------------------------------------------------ global search

#[tokio::test]
async fn global_search_fans_out_across_live_workspaces() {
    let a = api();
    // Open both workspaces first: the 300 ms deadline is for searching, not for a cold start.
    for id in [MAIN_WS, SECOND_WS] {
        a.state.workspaces.core().registry.open(id).unwrap();
    }
    let g = a.get(&ws("/graph")).await.json();
    let word = g["nodes"][5]["name"]
        .as_str()
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .to_lowercase();
    let r = a.get(&format!("/search?q={word}&limit=20")).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    let j = r.json();
    let items = j["items"].as_array().unwrap();
    assert!(!items.is_empty());
    assert!(
        items.iter().all(|i| i["workspaceId"] != ARCHIVED_WS),
        "archived workspaces are not searched"
    );
    assert!(items.iter().any(|i| i["workspaceId"] == MAIN_WS));
    for k in ["workspaceId", "workspaceName", "node", "score"] {
        assert!(items[0].get(k).is_some(), "{k}");
    }
    assert!(items[0]["node"].get("text").is_some() && items[0]["node"].get("kind").is_some());
    let scores: Vec<_> = items.iter().map(|i| i["score"].as_f64().unwrap()).collect();
    assert!(scores.windows(2).all(|w| w[0] >= w[1]));
    assert_eq!(j["timedOut"], json!([]));
    assert!(items.len() <= 20);

    let empty = a.get("/search?q=").await;
    assert_eq!(
        (empty.status, empty.code().as_str()),
        (StatusCode::BAD_REQUEST, "validation")
    );
    assert_eq!(a.get("/search").await.status, StatusCode::BAD_REQUEST);
    assert_eq!(
        lp_server::routes::search::WORKSPACE_DEADLINE,
        Duration::from_millis(300)
    );
}

// ------------------------------------------------------------------ test hooks

#[cfg(feature = "test-hooks")]
#[tokio::test]
async fn the_test_clock_pins_what_is_due() {
    let a = api();
    let at = |ms: i64| async move { ms };
    let _ = at;
    let set = a
        .post("/__test/clock", json!({"now": seed::FIXTURE_NOW}))
        .await;
    assert_eq!(set.json()["now"], seed::FIXTURE_NOW);
    let s = a.get(&ws("/review/stats")).await.json();
    assert_eq!(s["now"], seed::FIXTURE_NOW);
    let q = a.get(&ws("/review/queue")).await.json();
    assert_eq!(q["now"], seed::FIXTURE_NOW);
    assert!(
        q["dueNow"].as_u64().unwrap() < 24,
        "not everything is overdue at the fixture clock"
    );

    // Far in the future everything is due.
    a.post(
        "/__test/clock",
        json!({"now": seed::FIXTURE_NOW + 400 * 86_400_000}),
    )
    .await;
    assert_eq!(a.get(&ws("/review/queue")).await.json()["dueNow"], 24);
    // Released, the clock is real again.
    let real = a.post("/__test/clock", json!({"now": null})).await.json()["now"]
        .as_i64()
        .unwrap();
    assert!(real > seed::FIXTURE_NOW + 300 * 86_400_000 || real > 1_700_000_000_000);
}

#[cfg(not(feature = "test-hooks"))]
#[tokio::test]
async fn the_clock_hook_does_not_exist_in_normal_builds() {
    let a = api();
    let r = a.post("/__test/clock", json!({"now": 1})).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn settings_test_pings_a_profile_without_erroring_on_failure() {
    let a = api();
    a.state.faux().faux_text("pong");
    let ok = a
        .post(
            "/settings/test",
            json!({"provider": "claude", "model": "sonnet", "effort": "low"}),
        )
        .await;
    assert_eq!(ok.status, 200);
    assert_eq!(ok.json()["ok"], true);
    assert_eq!(ok.json()["message"], "pong");
    // Nothing scripted: the failure is reported in the body.
    let bad = a
        .post(
            "/settings/test",
            json!({"provider": "claude", "model": "nope"}),
        )
        .await;
    assert_eq!(bad.status, 200);
    assert_eq!(bad.json()["ok"], false);
    assert!(!bad.json()["message"].as_str().unwrap().is_empty());
    let unknown = a
        .post("/settings/test", json!({"provider": "gpt", "model": "x"}))
        .await;
    assert_eq!(unknown.status, 400);
}

// ------------------------------------------------------------------ courses

fn course_lesson(id: &str) -> Value {
    let items: Vec<Value> = (0..6)
        .map(|i| {
            if i < 4 {
                json!({"type":"mcq","q":format!("Which statement about {id} number {i} holds?"),
                    "options":["red apple","blue plum","pink pear","gray kiwi"],
                    "answer":"red apple","why":format!("because {id} {i}"),"tests":0})
            } else {
                json!({"type":"recall","q":format!("State fact {i} of {id}."),
                    "answer":format!("answer {id} {i}"),"accept":["a fact"],
                    "why":format!("because {id} {i}"),"tests":1})
            }
        })
        .collect();
    json!({"id":id,"title":format!("Title {id}"),"objective":"obj",
        "explain":"red apple and the fact are explained here",
        "key_facts":["red apple","the fact"],"fixes":[],"items":items})
}

fn queue_course_models(a: &Api) {
    let strands: Vec<Value> = (0..3)
        .map(|si| {
            let items: Vec<Value> = (1..=3)
                .map(|l| {
                    json!({"type":"mcq","q":format!("Which statement about strand {si} level {l} holds?"),
                        "options":["red apple","blue plum","pink pear","gray kiwi"],
                        "answer":"red apple","why":format!("placement why s{si} l{l}"),"level":l})
                })
                .collect();
            json!({"name":format!("Strand {si}"),"items":items})
        })
        .collect();
    a.state
        .faux()
        .faux_text(&json!({ "strands": strands }).to_string());
    let lessons: Vec<Value> = (0..10).map(|i| course_lesson(&format!("b0l{i}"))).collect();
    a.state
        .faux()
        .faux_text(&json!({"learner":"knows little","lessons":lessons}).to_string());
}

fn no_secret_keys(v: &Value) {
    match v {
        Value::Object(m) => {
            for (k, v) in m {
                assert!(
                    !["answer", "accept", "why"].contains(&k.as_str()),
                    "leaked `{k}`"
                );
                no_secret_keys(v);
            }
        }
        Value::Array(a) => a.iter().for_each(no_secret_keys),
        _ => {}
    }
}

#[tokio::test]
async fn a_course_runs_from_goal_to_feedback_and_usage() {
    let a = api();
    queue_course_models(&a);
    let r = a
        .post(
            &ws("/courses"),
            json!({"goal": "learn spanish", "why": "to read other code"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::CREATED, "{}", r.text);
    let created = r.json();
    no_secret_keys(&created);
    let id = created["id"].as_str().unwrap().to_owned();
    let items = created["items"].as_array().unwrap();
    // The strand probe opens with the middle level of each of the three strands.
    assert_eq!(items.len(), 3);

    // A reload before submitting: the same questions, no secrets.
    let again = a.get(&ws(&format!("/courses/{id}/placement"))).await;
    assert_eq!(again.status, StatusCode::OK, "{}", again.text);
    no_secret_keys(&again.json());
    assert_eq!(again.json()["items"], created["items"]);

    // The first question wrong on purpose, so there is a miss to show; later rounds are answered
    // with whatever comes first. The probe asks at most six questions in all.
    let mut round = items.clone();
    let mut graded_total = 0;
    for turn in 0..3 {
        let answers: Vec<Value> = round
            .iter()
            .enumerate()
            .map(|(n, i)| {
                let text = if turn == 0 && n == 0 {
                    "definitely wrong".to_owned()
                } else {
                    i["options"][0].as_str().unwrap_or("x").to_owned()
                };
                json!({"item": i["id"], "text": text, "guessed": false})
            })
            .collect();
        let r = a
            .post(
                &ws(&format!("/courses/{id}/placement")),
                json!({"answers": answers}),
            )
            .await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text);
        // The graded answers are revealed; the questions still to come never carry a key.
        no_secret_keys(&r.json()["next"]);
        graded_total += r.json()["graded"].as_array().unwrap().len();
        round = r.json()["next"].as_array().unwrap().clone();
        if round.is_empty() {
            break;
        }
    }
    assert!(round.is_empty(), "the probe must end within three rounds");
    assert!((3..=6).contains(&graded_total), "{graded_total}");
    let after = a.get(&ws(&format!("/courses/{id}/placement"))).await.json();
    assert_eq!(after["items"], json!([]));

    let r = a.get(&ws(&format!("/courses/{id}/misses"))).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    assert!(!r.json()["misses"].as_array().unwrap().is_empty());

    let mut state = String::new();
    for _ in 0..200 {
        let r = a.get(&ws(&format!("/courses/{id}/status"))).await;
        assert_eq!(r.status, StatusCode::OK, "{}", r.text);
        state = r.json()["state"].as_str().unwrap().to_owned();
        if state != "writing" {
            break;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    assert_eq!(state, "ready");
    let list = a.get(&ws("/courses")).await.json();
    assert_eq!(list["courses"][0]["id"], id.as_str());

    let step = a
        .post_empty(&ws(&format!("/courses/{id}/next")))
        .await
        .json();
    assert_eq!(step["step"], "explain", "{step}");
    let lesson = step["lesson"]["id"].as_str().unwrap().to_owned();
    let r = a
        .post(
            &ws(&format!("/courses/{id}/seen")),
            json!({"lesson": lesson}),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{}", r.text);

    let step = a
        .post_empty(&ws(&format!("/courses/{id}/next")))
        .await
        .json();
    assert_eq!(step["step"], "ask", "{step}");
    no_secret_keys(&step);
    let item = &step["item"];
    let r = a
        .post(
            &ws(&format!("/courses/{id}/answer")),
            json!({"item": item["id"], "text": "red apple", "confidence": 3, "ms": 1200}),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    let fb = r.json();
    assert_eq!(fb["correct"], true);
    assert_eq!(fb["answer"], "red apple");

    let r = a
        .post(
            &ws(&format!("/courses/{id}/flag")),
            json!({"item": item["id"], "reason": "looks odd"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::NO_CONTENT, "{}", r.text);
    let r = a
        .post(
            &ws(&format!("/courses/{id}/self-grade")),
            json!({"item": item["id"], "grade": "sideways"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST);
    assert_eq!(r.code(), "validation");

    let u = a.get(&ws(&format!("/courses/{id}/usage"))).await;
    assert_eq!(u.status, StatusCode::OK, "{}", u.text);
    assert!(u.json()["strongCallsThisHour"].is_number());
}

#[tokio::test]
async fn course_calls_without_an_ai_share_the_engine_unavailable_shape() {
    let a = api();
    let r = a
        .post(
            &ws("/courses"),
            json!({"goal": "learn spanish", "why": "to read other code"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::SERVICE_UNAVAILABLE, "{}", r.text);
    assert_eq!(r.code(), "engine_unavailable");
    assert_eq!(r.json()["error"]["retryable"], true);
    assert_eq!(a.get(&ws("/courses")).await.json()["courses"], json!([]));
}

#[tokio::test]
async fn course_ids_are_scoped_to_their_workspace_and_cannot_traverse() {
    let a = api();
    queue_course_models(&a);
    let id = a
        .post(
            &ws("/courses"),
            json!({"goal": "learn spanish", "why": "to read other code"}),
        )
        .await
        .json()["id"]
        .as_str()
        .unwrap()
        .to_owned();
    let other = format!("/workspaces/{SECOND_WS}/courses/{id}/status");
    let r = a.get(&other).await;
    assert_eq!(r.status, StatusCode::NOT_FOUND, "{}", r.text);
    for bad in ["..%2F..%2Fsettings", "%2e%2e", "UPPER", "a.b"] {
        let r = a.get(&ws(&format!("/courses/{bad}/status"))).await;
        assert_eq!(r.status, StatusCode::NOT_FOUND, "{bad}: {}", r.text);
    }
    assert_eq!(
        a.get(&ws("/courses/nope/usage")).await.status,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn course_inputs_are_length_checked_at_the_boundary() {
    let a = api();
    for goal in ["".to_owned(), "x".repeat(501)] {
        let r = a
            .post(&ws("/courses"), json!({"goal": goal, "why": "because"}))
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text);
    }
    // Why this matters is required, and bounded like the goal.
    for why in ["".to_owned(), "  ".to_owned(), "x".repeat(501)] {
        let r = a
            .post(
                &ws("/courses"),
                json!({"goal": "learn spanish", "why": why}),
            )
            .await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text);
    }
    let r = a
        .post(
            &ws("/courses/c1/ask"),
            json!({"question": "q".repeat(2001)}),
        )
        .await;
    assert_eq!(r.status, StatusCode::BAD_REQUEST, "{}", r.text);
}

// ------------------------------------------------------------------ learning method (M8)

fn method_course(id: &str) -> Course {
    Course {
        id: id.into(),
        method_version: 1,
        lessons: vec![Lesson {
            id: "l00".into(),
            title: "Intro".into(),
            key_facts: vec!["dogs bark loudly".into()],
            items: vec![
                Item {
                    id: "l00-i0".into(),
                    kind: ItemKind::Mcq,
                    q: "q0?".into(),
                    answer: "a".into(),
                    options: vec!["a".into(), "b".into(), "c".into(), "d".into()],
                    why: "because".into(),
                    tests: Some(0),
                    ..Item::default()
                },
                Item {
                    id: "l00-i1".into(),
                    kind: ItemKind::Recall,
                    q: "q1?".into(),
                    answer: "a".into(),
                    must_name: vec!["dog".into()],
                    tests: Some(0),
                    ..Item::default()
                },
            ],
            ..Lesson::default()
        }],
        ..Course::default()
    }
}

fn gate_of(r: &Reply) -> String {
    r.json()["error"]["details"]["gate"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// Opens a bout through the real ready gate and returns its id.
async fn start_bout(a: &Api, course: &str) -> String {
    let r = a
        .post(
            &ws("/learning/bout/start"),
            json!({"course": course, "ready": {"phoneAway": true, "tabsClosed": true, "focusSeconds": 30}}),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    r.json()["bout"].as_str().expect("open bout").to_owned()
}

/// Every gate rejection the HTTP contract can actually reach is `409 conflict` with a stable
/// `details.gate` code (docs/web/03 §Conventions); F07's order gate and F20's cycle gate are
/// each exercised at least once here.
#[tokio::test]
async fn method_course_runs_from_plan_to_close_with_gated_steps() {
    let a = api();
    let c = method_course("mc1");
    a.stores(MAIN_WS).courses.save_course(&c).unwrap();

    // F07: no teaching before the plan is confirmed.
    let r = a
        .post(
            &ws("/courses/mc1/predict"),
            json!({"lesson": "l00", "guess": "x"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text);
    assert_eq!(r.code(), "conflict");
    assert_eq!(gate_of(&r), "out_of_order");

    let plan = a.get(&ws("/courses/mc1/plan")).await;
    assert_eq!(plan.status, StatusCode::OK, "{}", plan.text);
    assert_eq!(plan.json()["confirmed"], false);
    assert_eq!(plan.json()["lessons"][0]["id"], "l00");

    assert_eq!(
        a.post_empty(&ws("/courses/mc1/plan/confirm")).await.status,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        a.get(&ws("/courses/mc1/plan")).await.json()["confirmed"],
        true
    );

    // F23: no step before a bout is open; the ready gate opens one.
    let r = a.post_empty(&ws("/courses/mc1/step")).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text);
    assert_eq!(gate_of(&r), "bout_required");
    let bout = start_bout(&a, "mc1").await;
    assert!(bout.starts_with("b_"), "{bout}");

    let step = a.post_empty(&ws("/courses/mc1/step")).await.json();
    assert_eq!(step["step"], "explain", "{step}");

    // Out of order: answering before the explanation is marked seen.
    let r = a
        .post(
            &ws("/courses/mc1/quiz-answer"),
            json!({"item": "l00-i0", "lesson": "l00", "text": "a", "reasoning": "", "gap": false}),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text);
    assert_eq!(gate_of(&r), "out_of_order");

    assert_eq!(
        a.post(&ws("/courses/mc1/explain-seen"), json!({"lesson": "l00"}))
            .await
            .status,
        StatusCode::NO_CONTENT
    );

    let step = a.post_empty(&ws("/courses/mc1/step")).await.json();
    assert_eq!(step["step"], "notes", "{step}");

    assert_eq!(
        a.post(
            &ws("/courses/mc1/note"),
            json!({"lesson": "l00", "text": ""})
        )
        .await
        .status,
        StatusCode::NO_CONTENT
    );

    let step = a.post_empty(&ws("/courses/mc1/step")).await.json();
    assert_eq!(step["step"], "ask", "{step}");
    assert_eq!(step["id"], "l00-i0");
    assert_eq!(step["kind"], "mcq");
    assert_eq!(step["ask"], "new");
    no_secret_keys(&step);

    let r = a
        .post(
            &ws("/courses/mc1/quiz-answer"),
            json!({"item": "l00-i0", "lesson": "l00", "text": "a", "reasoning": "", "gap": false}),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    assert_eq!(r.json(), json!({"status": "graded", "correct": true}));

    // A single correct mcq already clears F14's mastery bar; the lesson is a goal lesson, so a
    // Feynman pass is required before Close.
    let step = a.post_empty(&ws("/courses/mc1/step")).await.json();
    assert_eq!(step["step"], "feynman", "{step}");

    let m = a.get(&ws("/courses/mc1/mastery")).await;
    assert_eq!(m.status, StatusCode::OK, "{}", m.text);
    assert!(m.json()["countLine"]
        .as_str()
        .unwrap()
        .contains("confirmed"));

    a.state
        .faux()
        .faux_text(r#"{"correct":true,"note":"nice"}"#);
    let r = a
        .post(
            &ws("/courses/mc1/feynman"),
            json!({"lesson": "l00", "explanation": "it just does"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    assert_eq!(r.json()["correct"], true);

    let step = a.post_empty(&ws("/courses/mc1/step")).await.json();
    assert_eq!(step["step"], "close", "{step}");

    let r = a
        .post(
            &ws("/courses/mc1/braindump"),
            json!({"text": "dogs often bark loudly at night"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    assert_eq!(r.json()["missed"], json!([]));

    let r = a.post_empty(&ws("/courses/mc1/close")).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    assert!(r.json()["countLine"]
        .as_str()
        .unwrap()
        .contains("confirmed"));

    let r = a.get(&ws("/courses/mc1/register")).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    assert_eq!(r.json()["rows"], json!([]));
}

/// F07: a plan that (still) has a cycle can never be confirmed.
#[tokio::test]
async fn plan_confirm_rejects_a_cyclic_plan() {
    let a = api();
    let mut c = method_course("mc2");
    c.lessons.push(Lesson {
        id: "l01".into(),
        depends_on: vec!["l00".into()],
        ..Lesson::default()
    });
    c.lessons[0].depends_on = vec!["l01".into()];
    a.stores(MAIN_WS).courses.save_course(&c).unwrap();

    let r = a.post_empty(&ws("/courses/mc2/plan/confirm")).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text);
    assert_eq!(gate_of(&r), "plan_has_cycle");
}

/// F20: a prerequisite is rejected rather than spliced into a plan that is already cyclic.
#[tokio::test]
async fn insert_prereq_rejects_a_cyclic_plan() {
    let a = api();
    let mut c = method_course("mc3");
    c.plan_confirmed_at = Some(1);
    c.lessons.push(Lesson {
        id: "l01".into(),
        depends_on: vec!["l00".into()],
        ..Lesson::default()
    });
    c.lessons[0].depends_on = vec!["l01".into()];
    a.stores(MAIN_WS).courses.save_course(&c).unwrap();

    a.state.faux().faux_text(
        r#"{"title":"Basics","objective":"o","explain":"e","key_facts":["f"],
           "items":[{"type":"mcq","q":"q?","options":["a","b","c","d"],"answer":"a"}]}"#,
    );
    let r = a
        .post(
            &ws("/courses/mc3/plan/prereq"),
            json!({"before": "l00", "missing": "basics"}),
        )
        .await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text);
    assert_eq!(gate_of(&r), "prereq_cycle");
}

/// F23: a closed bout locks teaching behind the gap, and the lock is the server's, not the UI's.
#[tokio::test]
async fn step_is_refused_between_bouts_and_when_the_day_is_spent() {
    let a = api();
    let c = method_course("lk1");
    a.stores(MAIN_WS).courses.save_course(&c).unwrap();
    a.post_empty(&ws("/courses/lk1/plan/confirm")).await;
    let bout = start_bout(&a, "lk1").await;
    // The UI reads these camelCase names; a snake_case field here once shipped unnoticed.
    let clock = a.get(&ws("/learning/clock")).await.json();
    assert_eq!(clock["status"]["status"], "active", "{clock}");
    assert!(clock["status"]["activeMinutes"].is_u64(), "{clock}");
    assert_eq!(
        a.post_empty(&ws("/courses/lk1/step")).await.status,
        StatusCode::OK
    );

    let r = a
        .post(&ws("/learning/bout/end"), json!({ "bout": bout }))
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    let r = a.post_empty(&ws("/courses/lk1/step")).await;
    assert_eq!(r.status, StatusCode::CONFLICT, "{}", r.text);
    assert_eq!(gate_of(&r), "gap");
    assert!(r.json()["error"]["details"]["remainingMinutes"].is_u64());

    // The logged override lifts the gap and teaching resumes after a fresh ready gate.
    let r = a
        .post(
            &ws("/learning/bout/override"),
            json!({ "reason": "exam tomorrow" }),
        )
        .await;
    assert!(r.status.is_success(), "{}", r.text);
    start_bout(&a, "lk1").await;
    assert_eq!(
        a.post_empty(&ws("/courses/lk1/step")).await.status,
        StatusCode::OK
    );
}

/// The plan ships its numbers as tunable settings; every one is listed, bounded and saved.
#[tokio::test]
async fn method_settings_list_validate_and_persist_per_workspace() {
    let a = api();
    let r = a.get(&ws("/learning/settings")).await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    let all = r.json()["settings"].as_array().unwrap().clone();
    assert_eq!(all.len(), lp_service::learning::SETTING_BOUNDS.len());
    let find = |v: &serde_json::Value, n: &str| {
        v["settings"]
            .as_array()
            .unwrap()
            .iter()
            .find(|s| s["name"] == n)
            .cloned()
            .unwrap()
    };
    let before = find(&r.json(), "attempts_before_parking");
    assert_eq!(
        (before["value"].as_u64(), before["default"].as_u64()),
        (Some(3), Some(3))
    );

    let r = a
        .post(
            &ws("/learning/settings"),
            json!({"name": "attempts_before_parking", "value": 5}),
        )
        .await;
    assert_eq!(r.status, StatusCode::OK, "{}", r.text);
    assert_eq!(find(&r.json(), "attempts_before_parking")["value"], 5);
    // Persisted, and the clock renders against the saved limits.
    let again = a.get(&ws("/learning/settings")).await.json();
    assert_eq!(find(&again, "attempts_before_parking")["value"], 5);
    a.post(
        &ws("/learning/settings"),
        json!({"name": "bout_limit_minutes", "value": 45}),
    )
    .await;
    assert_eq!(
        a.get(&ws("/learning/clock")).await.json()["boutLimitMinutes"],
        45
    );

    for bad in [
        json!({"name": "nope", "value": 1}),
        json!({"name": "attempts_before_parking", "value": 0}),
        json!({"name": "bout_limit_minutes", "value": 9999}),
        json!({"name": "error_band_high_pct", "value": 5}),
    ] {
        let r = a.post(&ws("/learning/settings"), bad.clone()).await;
        assert_eq!(r.status, StatusCode::BAD_REQUEST, "{bad}: {}", r.text);
    }
}
