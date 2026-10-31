//! Router tests over `tower::ServiceExt::oneshot` on temp data roots (docs/web/03 §Server tests).

use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use axum::response::Response;
use axum::Router;
use http_body_util::BodyExt;
use lp_cli_driver::accounts::{AccountPool, DiscoverRoots, Strategy};
use lp_server::app::{router, AppState, EngineMode, ServerConfig};
use lp_server::assets::{Assets, CSP};
use lp_server::directory::DiskWorkspaces;
use lp_server::error::{ErrorCode, ErrorEnvelope};
use lp_service::events::{Event, EventHub, Scope};
use serde_json::Value;
use tower::ServiceExt;

const PORT: u16 = 7878;
const TOKEN: &str = "launch-token";
const HOST: &str = "127.0.0.1:7878";
const ORIGIN: &str = "http://127.0.0.1:7878";

struct Env {
    _dir: tempfile::TempDir,
    root: std::path::PathBuf,
    state: AppState,
}

impl Env {
    fn app(&self) -> Router {
        router(self.state.clone())
    }
}

fn write_ws(root: &Path, id: &str, name: &str, opened: i64) {
    let dir = lp_service::paths::workspace_dir(root, id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("workspace.json"),
        serde_json::json!({"id": id, "name": name, "slug": name.to_lowercase(), "icon": "brain",
                           "hue": "violet", "archived": false, "last_opened_at": opened})
        .to_string(),
    )
    .unwrap();
}

fn env_with(token: Option<&str>, hub: EventHub, heartbeat: Duration, dist: Option<&str>) -> Env {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("data");
    std::fs::create_dir_all(&root).unwrap();
    let home = dir.path().join("home");
    let profile = home.join(".claude");
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::write(
        profile.join(".claude.json"),
        r#"{"oauthAccount":{"emailAddress":"me@x.com"}}"#,
    )
    .unwrap();
    let pool = lp_service::accounts::discover_pool(&DiscoverRoots {
        home,
        ..DiscoverRoots::default()
    });
    let dist_dir = dir.path().join("dist");
    if let Some(index) = dist {
        std::fs::create_dir_all(dist_dir.join("assets")).unwrap();
        std::fs::write(dist_dir.join("index.html"), index).unwrap();
        std::fs::write(dist_dir.join("assets/index-3f9a1c.js"), "console.log(1)").unwrap();
        std::fs::write(dist_dir.join("favicon.svg"), "<svg/>").unwrap();
    }
    let state = AppState::new(
        ServerConfig {
            port: PORT,
            token: token.map(str::to_owned),
            engine: EngineMode::Faux,
            data_root: root.clone(),
            heartbeat,
        },
        Arc::new(hub),
        Arc::new(DiskWorkspaces::new(root.clone())),
        pool,
        Assets::Dir(dist_dir),
        "dev-test".into(),
    );
    Env {
        _dir: dir,
        root,
        state,
    }
}

fn env() -> Env {
    env_with(
        Some(TOKEN),
        EventHub::with_options(64, 0),
        Duration::from_secs(15),
        Some("<!doctype html><title>LearnPuppy</title>"),
    )
}

fn get(path: &str) -> axum::http::request::Builder {
    Request::get(path).header(header::HOST, HOST)
}

fn authed(b: axum::http::request::Builder) -> axum::http::request::Builder {
    b.header(header::COOKIE, format!("lp_session={TOKEN}"))
}

async fn send(app: Router, req: Request<Body>) -> Response {
    app.oneshot(req).await.unwrap()
}

async fn json(res: Response) -> Value {
    let bytes = res.into_body().collect().await.unwrap().to_bytes();
    serde_json::from_slice(&bytes).unwrap()
}

async fn envelope(res: Response) -> ErrorEnvelope {
    let rid = res
        .headers()
        .get("x-request-id")
        .unwrap()
        .to_str()
        .unwrap()
        .to_owned();
    let env: ErrorEnvelope = serde_json::from_value(json(res).await).unwrap();
    assert_eq!(env.error.request_id, rid, "envelope echoes X-Request-Id");
    env
}

// ---------------------------------------------------------------- health / bootstrap

#[tokio::test]
async fn health_needs_no_session_but_checks_host() {
    let e = env();
    let res = send(e.app(), get("/api/v1/health").body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert!(res.headers().contains_key("x-request-id"));
    let body = json(res).await;
    assert_eq!(body["ok"], true);
    assert_eq!(body["engine"], "faux");
    assert_eq!(body["version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(body["dataDir"], e.root.display().to_string());

    let req = Request::get("/api/v1/health")
        .header(header::HOST, "evil.example:7878")
        .body(Body::empty())
        .unwrap();
    let res = send(e.app(), req).await;
    assert_eq!(res.status(), StatusCode::MISDIRECTED_REQUEST);
    assert_eq!(envelope(res).await.error.code, ErrorCode::BadHost);
}

#[tokio::test]
async fn bootstrap_lists_workspaces_accounts_and_device() {
    let e = env();
    write_ws(&e.root, "ws_a", "Alpha", 10);
    write_ws(&e.root, "ws_b", "Beta", 20);
    std::fs::write(
        lp_service::paths::state_json(&e.root),
        r#"{"last_workspace":"ws_a"}"#,
    )
    .unwrap();
    let res = send(
        e.app(),
        authed(get("/api/v1/bootstrap"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    let b = json(res).await;
    assert_eq!(b["deviceId"], "dev-test");
    assert_eq!(b["engine"], "faux");
    assert_eq!(b["dev"], false);
    assert_eq!(b["lastWorkspaceId"], "ws_a");
    let ids: Vec<_> = b["workspaces"]
        .as_array()
        .unwrap()
        .iter()
        .map(|w| w["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["ws_b", "ws_a"]);
    assert_eq!(b["accounts"][0]["id"], "claude:me@x.com");
    assert_eq!(b["accounts"][0]["status"], "ready");
    assert!(b["accounts"][0].get("configDir").is_none());
    assert_eq!(b["banners"], serde_json::json!([]));
}

#[tokio::test]
async fn bootstrap_with_an_empty_pool_and_no_workspaces() {
    let mut e = env();
    e.state.pool = Arc::new(AccountPool::new(Vec::new(), Strategy::Priority));
    let res = send(
        e.app(),
        authed(get("/api/v1/bootstrap"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    let b = json(res).await;
    assert_eq!(b["workspaces"], serde_json::json!([]));
    assert_eq!(b["accounts"], serde_json::json!([]));
    assert_eq!(b["lastWorkspaceId"], Value::Null);
}

// ---------------------------------------------------------------- auth matrix

#[tokio::test]
async fn api_without_session_is_401() {
    let e = env();
    let res = send(
        e.app(),
        get("/api/v1/bootstrap").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    let env = envelope(res).await;
    assert_eq!(env.error.code, ErrorCode::AuthRequired);
    assert!(!env.error.retryable);

    let req = get("/api/v1/bootstrap")
        .header(header::COOKIE, "lp_session=wrong")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(e.app(), req).await.status(), StatusCode::UNAUTHORIZED);
    // SSE too.
    let res = send(e.app(), get("/api/v1/events").body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn wrong_host_is_421_everywhere() {
    let e = env();
    for (path, host) in [
        ("/api/v1/bootstrap", "localhost:9999"),
        ("/", "attacker.test"),
        ("/assets/index-3f9a1c.js", "127.0.0.1"),
    ] {
        let req = authed(Request::get(path).header(header::HOST, host))
            .body(Body::empty())
            .unwrap();
        let res = send(e.app(), req).await;
        assert_eq!(
            res.status(),
            StatusCode::MISDIRECTED_REQUEST,
            "{path} {host}"
        );
    }
    let req = authed(Request::get("/api/v1/bootstrap").header(header::HOST, "localhost:7878"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(e.app(), req).await.status(), StatusCode::OK);
}

fn mutation(origin: Option<&str>, csrf: Option<&str>) -> Request<Body> {
    let mut b = authed(Request::post("/api/v1/bootstrap").header(header::HOST, HOST));
    if let Some(o) = origin {
        b = b.header(header::ORIGIN, o);
    }
    if let Some(c) = csrf {
        b = b.header("x-lp-csrf", c);
    }
    b.body(Body::empty()).unwrap()
}

#[tokio::test]
async fn mutations_need_same_origin_and_csrf_header() {
    let e = env();
    for (origin, csrf) in [
        (Some("http://evil.test"), Some("1")),
        (Some("http://localhost:7878"), Some("1")),
        (Some("null"), Some("1")),
        (None, Some("1")),
        (Some(ORIGIN), None),
        (Some(ORIGIN), Some("0")),
    ] {
        let res = send(e.app(), mutation(origin, csrf)).await;
        assert_eq!(res.status(), StatusCode::FORBIDDEN, "{origin:?} {csrf:?}");
        assert_eq!(envelope(res).await.error.code, ErrorCode::Forbidden);
    }
    // Both present: the guard lets it through to the router (no POST route here).
    let res = send(e.app(), mutation(Some(ORIGIN), Some("1"))).await;
    assert_eq!(res.status(), StatusCode::METHOD_NOT_ALLOWED);
    assert_eq!(envelope(res).await.error.code, ErrorCode::MethodNotAllowed);
}

#[tokio::test]
async fn no_cors_headers_ever() {
    let e = env();
    let req = authed(get("/api/v1/bootstrap").header(header::ORIGIN, "http://evil.test"))
        .body(Body::empty())
        .unwrap();
    let res = send(e.app(), req).await;
    assert!(res
        .headers()
        .keys()
        .all(|k| !k.as_str().starts_with("access-control-")));
}

#[tokio::test]
async fn launch_token_becomes_a_cookie_and_leaves_the_url() {
    let e = env();
    let res = send(
        e.app(),
        get(&format!("/?t={TOKEN}")).body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert_eq!(res.headers()[header::LOCATION], "/");
    let cookie = res.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .to_owned();
    assert!(
        cookie.starts_with(&format!("lp_session={TOKEN};")),
        "{cookie}"
    );
    for attr in ["HttpOnly", "SameSite=Strict", "Path=/"] {
        assert!(cookie.contains(attr), "{cookie} lacks {attr}");
    }

    // The cookie it set opens the API.
    let req = get("/api/v1/bootstrap")
        .header(header::COOKIE, cookie.split(';').next().unwrap())
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(e.app(), req).await.status(), StatusCode::OK);

    // A wrong token is dropped from the URL without a cookie.
    let res = send(e.app(), get("/?t=nope").body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::SEE_OTHER);
    assert!(res.headers().get(header::SET_COOKIE).is_none());
}

#[tokio::test]
async fn dev_mode_skips_only_the_token() {
    let e = env_with(
        None,
        EventHub::with_options(8, 0),
        Duration::from_secs(15),
        None,
    );
    let res = send(
        e.app(),
        get("/api/v1/bootstrap").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(json(res).await["dev"], true);
    let req = Request::get("/api/v1/bootstrap")
        .header(header::HOST, "evil.test:7878")
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        send(e.app(), req).await.status(),
        StatusCode::MISDIRECTED_REQUEST
    );
    let req = Request::post("/api/v1/bootstrap")
        .header(header::HOST, HOST)
        .header(header::ORIGIN, "http://evil.test")
        .header("x-lp-csrf", "1")
        .body(Body::empty())
        .unwrap();
    assert_eq!(send(e.app(), req).await.status(), StatusCode::FORBIDDEN);
}

// ---------------------------------------------------------------- errors

#[tokio::test]
async fn unknown_api_paths_get_the_json_envelope() {
    let e = env();
    for path in ["/api/v1/nope", "/api/v2/health", "/api"] {
        let res = send(e.app(), authed(get(path)).body(Body::empty()).unwrap()).await;
        assert_eq!(res.status(), StatusCode::NOT_FOUND, "{path}");
        let env = envelope(res).await;
        assert_eq!(env.error.code, ErrorCode::NotFound);
        assert!(env.error.message.contains(path));
    }
    let res = send(
        e.app(),
        authed(get("/api/v1/events?workspace=../x"))
            .body(Body::empty())
            .unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::BAD_REQUEST);
    let env = envelope(res).await;
    assert_eq!(env.error.code, ErrorCode::Validation);
    assert_eq!(env.error.details.unwrap()["fields"][0]["path"], "workspace");
}

// ---------------------------------------------------------------- SPA / CSP

fn assert_csp(res: &Response) {
    assert_eq!(res.headers()["content-security-policy"], CSP);
    assert_eq!(res.headers()["x-content-type-options"], "nosniff");
}

#[tokio::test]
async fn spa_files_fallback_and_headers() {
    let e = env();
    let res = send(e.app(), get("/").body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_csp(&res);
    assert_eq!(res.headers()[header::CACHE_CONTROL], "no-cache");
    assert!(res.headers()[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .starts_with("text/html"));

    // Client routes get index.html.
    let res = send(e.app(), get("/ws/ws_a/graph").body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_csp(&res);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    assert!(String::from_utf8_lossy(&body).contains("<title>LearnPuppy</title>"));

    // Hashed assets are immutable.
    let res = send(
        e.app(),
        get("/assets/index-3f9a1c.js").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::OK);
    assert_eq!(
        res.headers()[header::CACHE_CONTROL],
        "public, max-age=31536000, immutable"
    );
    assert!(res.headers()[header::CONTENT_TYPE]
        .to_str()
        .unwrap()
        .contains("javascript"));

    let res = send(e.app(), get("/favicon.svg").body(Body::empty()).unwrap()).await;
    assert_eq!(res.headers()[header::CACHE_CONTROL], "no-cache");
    assert_eq!(res.headers()[header::CONTENT_TYPE], "image/svg+xml");

    // Missing files with an extension are 404, not index.html.
    let res = send(
        e.app(),
        get("/assets/gone-1.js").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(res.status(), StatusCode::NOT_FOUND);
    assert_csp(&res);
}

#[tokio::test]
async fn missing_build_shows_a_clear_page() {
    let e = env_with(
        Some(TOKEN),
        EventHub::with_options(8, 0),
        Duration::from_secs(15),
        None,
    );
    let res = send(e.app(), get("/").body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_csp(&res);
    let body = res.into_body().collect().await.unwrap().to_bytes();
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("web UI is not built"));
    assert!(text.contains("bun run --cwd web build"));
    // The API is unaffected.
    let res = send(e.app(), get("/api/v1/health").body(Body::empty()).unwrap()).await;
    assert_eq!(res.status(), StatusCode::OK);
}

// ---------------------------------------------------------------- SSE

/// Reads SSE frames until `n` events (not comments) arrived or `timeout` passes.
struct SseReader {
    body: Body,
    buf: String,
}

#[derive(Debug, Default, Clone, PartialEq)]
struct Frame {
    id: Option<u64>,
    event: String,
    data: Value,
    comment: Option<String>,
}

impl SseReader {
    fn new(res: Response) -> Self {
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CONTENT_TYPE], "text/event-stream");
        Self {
            body: res.into_body(),
            buf: String::new(),
        }
    }

    async fn frame(&mut self) -> Frame {
        loop {
            if let Some(end) = self.buf.find("\n\n") {
                let raw: String = self.buf.drain(..end + 2).collect();
                let mut f = Frame::default();
                for line in raw.lines() {
                    if let Some(c) = line.strip_prefix(':') {
                        f.comment = Some(c.trim().to_owned());
                    } else if let Some(v) = line.strip_prefix("id:") {
                        f.id = Some(v.trim().parse().unwrap());
                    } else if let Some(v) = line.strip_prefix("event:") {
                        f.event = v.trim().to_owned();
                    } else if let Some(v) = line.strip_prefix("data:") {
                        f.data = serde_json::from_str(v.trim()).unwrap();
                    }
                }
                return f;
            }
            let frame = tokio::time::timeout(Duration::from_secs(3), self.body.frame())
                .await
                .expect("sse frame within 3s")
                .expect("stream open")
                .unwrap();
            if let Ok(data) = frame.into_data() {
                self.buf.push_str(std::str::from_utf8(&data).unwrap());
            }
        }
    }

    /// The next event frame, skipping heartbeat comments.
    async fn event(&mut self) -> Frame {
        loop {
            let f = self.frame().await;
            if f.comment.is_none() {
                return f;
            }
        }
    }
}

async fn open_sse(e: &Env, query: &str, last: Option<&str>) -> SseReader {
    let mut b = authed(get(&format!("/api/v1/events{query}")));
    if let Some(l) = last {
        b = b.header("last-event-id", l);
    }
    SseReader::new(send(e.app(), b.body(Body::empty()).unwrap()).await)
}

fn graph(v: u64) -> Event {
    Event::GraphChanged { version: v }
}

fn ws(id: &str) -> Scope {
    Scope::Workspace(id.into())
}

#[tokio::test]
async fn sse_hello_then_live_events_in_order_filtered_by_workspace() {
    let e = env();
    let hub = e.state.hub.clone();
    hub.publish(Scope::Global, Event::AccountsChanged); // before connect: not replayed
    let mut s = open_sse(&e, "?workspace=ws_a", None).await;
    let hello = s.event().await;
    assert_eq!(hello.event, "hello");
    assert_eq!(
        hello.id,
        Some(1),
        "fresh stream's hello carries the current id"
    );
    assert_eq!(hello.data["version"], env!("CARGO_PKG_VERSION"));
    assert!(hello.data["serverTime"].as_i64().unwrap() > 0);

    hub.publish(ws("ws_b"), graph(9));
    hub.publish(ws("ws_a"), graph(1));
    hub.publish(Scope::Global, Event::AccountsChanged);
    e.state.pump.publish(
        ws("ws_a"),
        Event::ChatRest {
            chat_id: "c".into(),
            view_version: 3,
        },
    );
    let got: Vec<Frame> = vec![s.event().await, s.event().await, s.event().await];
    let summary: Vec<_> = got.iter().map(|f| (f.id, f.event.as_str())).collect();
    assert_eq!(
        summary,
        [
            (Some(3), "graph.changed"),
            (Some(4), "accounts.changed"),
            (Some(5), "chat.rest")
        ]
    );
    assert_eq!(got[0].data, serde_json::json!({"version": 1}));
    assert_eq!(got[1].data, serde_json::json!({}));
    assert_eq!(
        got[2].data,
        serde_json::json!({"chatId": "c", "viewVersion": 3})
    );
}

#[tokio::test]
async fn sse_global_stream_sees_only_global_events() {
    let e = env();
    let mut s = open_sse(&e, "", None).await;
    assert_eq!(s.event().await.event, "hello");
    e.state.hub.publish(ws("ws_a"), graph(1));
    e.state.hub.publish(Scope::Global, Event::ReviewChanged);
    let f = s.event().await;
    assert_eq!((f.id, f.event.as_str()), (Some(2), "review.changed"));
}

#[tokio::test]
async fn sse_replays_from_last_event_id() {
    let e = env();
    let hub = e.state.hub.clone();
    for v in 1..=4 {
        hub.publish(ws("ws_a"), graph(v));
    }
    hub.publish(ws("ws_b"), graph(99));
    hub.publish(Scope::Global, Event::AccountsChanged);
    let mut s = open_sse(&e, "?workspace=ws_a", Some("2")).await;
    let hello = s.event().await;
    assert_eq!((hello.event.as_str(), hello.id), ("hello", None));
    let replay: Vec<_> = [s.event().await, s.event().await, s.event().await]
        .into_iter()
        .map(|f| (f.id.unwrap(), f.event))
        .collect();
    assert_eq!(
        replay,
        [
            (3, "graph.changed".to_owned()),
            (4, "graph.changed".to_owned()),
            (6, "accounts.changed".to_owned())
        ]
    );
    // Live continues after the replay without duplicates.
    hub.publish(ws("ws_a"), graph(5));
    let f = s.event().await;
    assert_eq!((f.id, f.data["version"].as_u64()), (Some(7), Some(5)));
}

#[tokio::test]
async fn sse_resyncs_when_the_ring_overflowed_or_the_id_is_unknown() {
    let e = env_with(
        Some(TOKEN),
        EventHub::with_options(4, 100),
        Duration::from_secs(15),
        None,
    );
    for v in 1..=10 {
        e.state.hub.publish(ws("ws_a"), graph(v));
    }
    // Event 102 fell out of the 4-slot ring.
    let mut s = open_sse(&e, "?workspace=ws_a", Some("102")).await;
    assert_eq!(s.event().await.event, "hello");
    let r = s.event().await;
    assert_eq!((r.event.as_str(), r.id), ("resync", Some(110)));
    assert_eq!(r.data, serde_json::json!({}));

    // Ids from a previous server run and garbage ids resync too.
    for last in ["5", "not-a-number", "999999"] {
        let mut s = open_sse(&e, "?workspace=ws_a", Some(last)).await;
        s.event().await;
        assert_eq!(s.event().await.event, "resync", "{last}");
    }

    // Still inside the ring: plain replay.
    let mut s = open_sse(&e, "?workspace=ws_a", Some("108")).await;
    s.event().await;
    assert_eq!(s.event().await.id, Some(109));
}

#[tokio::test]
async fn sse_lagging_stream_gets_resync() {
    let e = env_with(
        Some(TOKEN),
        EventHub::with_options(4, 0),
        Duration::from_secs(15),
        None,
    );
    let mut s = open_sse(&e, "?workspace=ws_a", None).await;
    assert_eq!(s.event().await.event, "hello");
    // More events than the broadcast buffer holds before the stream reads any.
    for v in 1..=20 {
        e.state.hub.publish(ws("ws_a"), graph(v));
    }
    let r = s.event().await;
    assert_eq!((r.event.as_str(), r.id), ("resync", Some(20)));
    e.state.hub.publish(ws("ws_a"), graph(21));
    assert_eq!(s.event().await.id, Some(21));
}

#[tokio::test]
async fn sse_sends_heartbeat_comments() {
    let e = env_with(
        Some(TOKEN),
        EventHub::with_options(8, 0),
        Duration::from_millis(50),
        None,
    );
    let mut s = open_sse(&e, "", None).await;
    assert_eq!(s.frame().await.event, "hello");
    let hb = s.frame().await;
    assert_eq!(hb.comment.as_deref(), Some("heartbeat"));
    assert_eq!(s.frame().await.comment.as_deref(), Some("heartbeat"));
}

#[tokio::test]
async fn sse_coalesces_deltas_and_ends_on_shutdown() {
    let e = env();
    let mut s = open_sse(&e, "?workspace=ws_a", None).await;
    s.event().await;
    for t in ["a", "b", "c"] {
        e.state
            .pump
            .delta("ws_a", "c1", "t1", lp_service::events::DeltaKind::Text, t);
    }
    e.state.pump.publish(
        ws("ws_a"),
        Event::ChatRest {
            chat_id: "c1".into(),
            view_version: 1,
        },
    );
    let first = s.event().await;
    assert_eq!(first.event, "chat.delta");
    assert_eq!(
        (first.data["seq"].as_u64(), first.data["text"].as_str()),
        (Some(1), Some("a"))
    );
    let second = s.event().await;
    assert_eq!(
        (second.data["seq"].as_u64(), second.data["text"].as_str()),
        (Some(2), Some("bc"))
    );
    assert_eq!(s.event().await.event, "chat.rest");

    e.state.shutdown.cancel();
    let end = tokio::time::timeout(Duration::from_secs(3), s.body.frame())
        .await
        .unwrap();
    assert!(end.is_none(), "stream ends on shutdown");
}
