//! Chat commands (docs/web/03 §Learn): create a chat (its first turn starts at once), post a
//! turn (`user` | `answer` | `steer` | `resume`), cancel, and the Markdown export. Turns run in
//! `lp_service::chat::ChatRuntime`; progress arrives over SSE (`chat.delta`, `chat.toolCall`,
//! `chat.error`, `chat.rest`).

use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::{header, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use lp_core::ports::{ChatStore, GraphStore};
use lp_service::chat::{self, Engines, Rejected, TurnCtx};
use lp_service::settings::Settings;
use lp_service::workspace::WorkspaceHandle;

use super::common::{blocking, open_writable, open_ws};
use crate::app::AppState;
use crate::dto::chat::{
    session_of, CancelDto, ChatKind, ClassroomViewDto, CreateChat, CreatedChatDto, TurnAcceptedDto,
    TurnBody, TurnDto,
};
use crate::engine::RoleEngine;
use crate::error::{ApiError, ApiJson, ErrorCode, ErrorEnvelope, FieldError};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces/{ws}/chats", post(create))
        .route("/workspaces/{ws}/chats/{id}/turn", post(turn))
        .route("/workspaces/{ws}/chats/{id}/cancel", post(cancel))
        .route("/workspaces/{ws}/chats/{id}/export.md", get(export))
}

/// Most seed propositions one chat starts from.
const MAX_SEEDS: usize = 50;

/// Ids are file names; anything that could name another path is simply not found.
pub(crate) fn checked_id(what: &str, id: &str) -> Result<(), ApiError> {
    let ok = !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(ApiError::not_found(format!("no {what} {id}")))
    }
}

/// What a turn of workspace `h` runs with: role engines from the current settings, the CLI cap,
/// and the server shutdown token.
pub(crate) async fn turn_ctx(
    st: &AppState,
    h: Arc<WorkspaceHandle>,
) -> Result<TurnCtx<RoleEngine>, ApiError> {
    if st.config.engine == crate::app::EngineMode::Faux {
        if let Some(path) = std::env::var_os("LP_FAUX_SCRIPT") {
            st.chats
                .load_script(st.faux(), path.into())
                .await
                .map_err(|e| rejected(e, None))?;
        }
    }
    let root = st.config.data_root.clone();
    let settings = Arc::new(blocking(move || Ok(Settings::load(&root)?)).await?);
    let cap = usize::from(settings.concurrency());
    let (mode, faux, pool) = (st.config.engine, st.faux().clone(), st.pool.clone());
    let engines: Engines<RoleEngine> = Arc::new(move |role| {
        Arc::new(match mode {
            crate::app::EngineMode::Faux => RoleEngine::Faux(faux.clone()),
            crate::app::EngineMode::Real => {
                RoleEngine::Real(lp_service::engine::CliPoolEngine::for_role(
                    pool.clone(),
                    &settings,
                    role,
                    settings.pin(role),
                ))
            }
        })
    });
    Ok(TurnCtx {
        handle: h,
        engines,
        cap,
        shutdown: st.shutdown.clone(),
    })
}

/// Marks jobs a previous server left running as interrupted (once per workspace per process).
pub(crate) async fn ensure_recovered(
    st: &AppState,
    h: &Arc<WorkspaceHandle>,
) -> Result<(), ApiError> {
    // Textbook imports a dead server left running are repaired on the same occasion.
    if let Err(error) = st.textbook(h).await {
        tracing::warn!(workspace = h.id(), ?error, "textbook recovery failed");
    }
    if st.chats.recovered(h.id()) {
        return Ok(());
    }
    let ctx = turn_ctx(st, h.clone()).await?;
    st.chats.ensure(&ctx).await.map_err(|e| rejected(e, None))
}

/// The Classroom view of `id` with its live turn state. Blocking.
pub(crate) fn view_of(
    st: &AppState,
    h: &WorkspaceHandle,
    id: &str,
) -> Result<ClassroomViewDto, ApiError> {
    let rec = h.stores().chats.load_chat(id).map_err(|e| match e {
        lp_core::ports::StoreError::NotFound(_) => ApiError::not_found(format!("no chat {id}")),
        other => other.into(),
    })?;
    let jobs = h
        .stores()
        .jobs
        .all_report()
        .map(|(j, _)| j)
        .unwrap_or_default();
    let turn = TurnDto::from(&st.chats.status(h, id));
    Ok(ClassroomViewDto::new(&rec, &jobs, turn))
}

/// Maps a runtime rejection to the error envelope; `view` is embedded for `stale_question`.
pub(crate) fn rejected(e: Rejected, view: Option<ClassroomViewDto>) -> ApiError {
    let message = e.to_string();
    match e {
        Rejected::NotFound(_) => ApiError::not_found(message),
        Rejected::InProgress { turn_id } => ApiError::new(ErrorCode::TurnInProgress, message)
            .with_details(serde_json::json!({ "turnId": turn_id })),
        Rejected::StaleQuestion => {
            let err = ApiError::new(ErrorCode::StaleQuestion, message);
            match view.and_then(|v| serde_json::to_value(v).ok()) {
                Some(v) => err.with_details(serde_json::json!({ "view": v })),
                None => err,
            }
        }
        Rejected::PendingQuestion { call_id } => ApiError::new(ErrorCode::Conflict, message)
            .with_details(serde_json::json!({ "pendingCallId": call_id })),
        Rejected::NothingToResume | Rejected::Conflict(_) => {
            ApiError::new(ErrorCode::Conflict, message)
        }
        Rejected::Invalid { field, message } => ApiError::invalid(field, message),
        Rejected::Store(s) => s.into(),
    }
}

fn accepted<T: serde::Serialize>(body: T) -> Response {
    (StatusCode::ACCEPTED, Json(body)).into_response()
}

/// Starts a Teach or Practice chat; its first turn runs at once.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/chats",
    tag = "chats",
    params(("ws" = String, Path, description = "Workspace id")),
    request_body = CreateChat,
    responses(
        (status = 202, description = "Created; the first turn is running", body = CreatedChatDto),
        (status = 400, description = "Invalid topic or unknown seed nodes", body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
    )
)]
pub async fn create(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(body): ApiJson<CreateChat>,
) -> Result<Response, ApiError> {
    if body.seed_node_ids.len() > MAX_SEEDS {
        return Err(ApiError::invalid(
            "seedNodeIds",
            format!("at most {MAX_SEEDS} propositions"),
        ));
    }
    let h = open_writable(&st, &ws).await?;
    let ids = body.seed_node_ids.clone();
    let graph = h.stores().graph.clone();
    let seeds = blocking(move || {
        if ids.is_empty() {
            return Ok(Vec::new());
        }
        let nodes: HashMap<String, lp_core::graph::GraphNode> = graph
            .nodes()?
            .into_iter()
            .map(|n| (n.id.to_string(), n))
            .collect();
        let mut missing = Vec::new();
        let mut seeds = Vec::new();
        for (i, id) in ids.iter().enumerate() {
            match nodes.get(id) {
                Some(n) => seeds.push(chat::Seed {
                    id: id.clone(),
                    name: n.name.clone(),
                    statement: n.statement.clone(),
                }),
                None => missing.push(FieldError {
                    path: format!("seedNodeIds[{i}]"),
                    message: format!("no node {id}"),
                }),
            }
        }
        if missing.is_empty() {
            Ok(seeds)
        } else {
            Err(ApiError::validation(missing))
        }
    })
    .await?;
    let ctx = turn_ctx(&st, h).await?;
    let new = chat::NewChat {
        kind: match body.kind {
            ChatKind::Teach => chat::NewKind::Teach,
            ChatKind::Practice => chat::NewKind::Practice,
        },
        topic: body.topic,
        seeds,
    };
    let created = st
        .chats
        .create_chat(&ctx, new)
        .await
        .map_err(|e| rejected(e, None))?;
    Ok(accepted(CreatedChatDto {
        chat_id: created.chat_id,
        turn_id: created.turn_id,
    }))
}

/// Sends the learner's message, answer, steer or resume; the turn runs in the background.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/chats/{id}/turn",
    tag = "chats",
    params(
        ("ws" = String, Path, description = "Workspace id"),
        ("id" = String, Path, description = "Chat id"),
    ),
    request_body = TurnBody,
    responses(
        (status = 202, body = TurnAcceptedDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "`turn_in_progress`; `stale_question` with the fresh view in `details.view`; `conflict` while a question is pending or nothing to resume; `workspace_archived`", body = ErrorEnvelope),
    )
)]
pub async fn turn(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(body): ApiJson<TurnBody>,
) -> Result<Response, ApiError> {
    checked_id("chat", &id)?;
    let h = open_writable(&st, &ws).await?;
    let ctx = turn_ctx(&st, h.clone()).await?;
    let (client_id, input) = body.into_input();
    match st.chats.post_turn(&ctx, &id, &client_id, input).await {
        Ok(a) => Ok(accepted(TurnAcceptedDto::from(a))),
        Err(Rejected::StaleQuestion) => {
            let st2 = st.clone();
            let view = blocking(move || view_of(&st2, &h, &id)).await.ok();
            Err(rejected(Rejected::StaleQuestion, view))
        }
        Err(e) => Err(rejected(e, None)),
    }
}

/// Stops the running or queued turn; text streamed so far is kept, marked interrupted.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/chats/{id}/cancel",
    tag = "chats",
    params(
        ("ws" = String, Path, description = "Workspace id"),
        ("id" = String, Path, description = "Chat id"),
    ),
    responses((status = 200, body = CancelDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn cancel(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<CancelDto>, ApiError> {
    checked_id("chat", &id)?;
    let h = open_ws(&st, &ws).await?;
    let c = id.clone();
    blocking(move || {
        h.stores().chats.load_chat(&c).map_err(|e| match e {
            lp_core::ports::StoreError::NotFound(_) => ApiError::not_found(format!("no chat {c}")),
            other => other.into(),
        })?;
        Ok(h)
    })
    .await
    .map(|h| {
        let turn_id = st.chats.cancel(h.id(), &id);
        Json(CancelDto {
            cancelled: turn_id.is_some(),
            turn_id,
        })
    })
}

/// The chat as a Markdown transcript (download).
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/chats/{id}/export.md",
    tag = "chats",
    params(
        ("ws" = String, Path, description = "Workspace id"),
        ("id" = String, Path, description = "Chat id"),
    ),
    responses(
        (status = 200, content_type = "text/markdown", body = String),
        (status = 404, body = ErrorEnvelope),
    )
)]
pub async fn export(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    checked_id("chat", &id)?;
    let h = open_ws(&st, &ws).await?;
    let c = id.clone();
    let md = blocking(move || {
        let rec = h.stores().chats.load_chat(&c).map_err(|e| match e {
            lp_core::ports::StoreError::NotFound(_) => ApiError::not_found(format!("no chat {c}")),
            other => other.into(),
        })?;
        Ok(chat::export_markdown(&rec, &session_of(&rec)))
    })
    .await?;
    let mut res = md.into_response();
    let h = res.headers_mut();
    h.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/markdown; charset=utf-8"),
    );
    if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{id}.md\"")) {
        h.insert(header::CONTENT_DISPOSITION, v);
    }
    Ok(res)
}

/// Recover workbench jobs when the server starts, even if no browser opens a chat.
pub(crate) fn recover_on_boot(st: &AppState) {
    if !st.chats.start_boot() {
        return;
    }
    let st = st.clone();
    tokio::spawn(async move {
        let core = st.workspaces.core().clone();
        let workspaces = blocking(move || Ok(core.registry.list(false)?)).await;
        match workspaces {
            Ok(list) => {
                for workspace in list.workspaces {
                    let id = workspace.meta.id;
                    match open_ws(&st, &id).await {
                        Ok(handle) => {
                            if let Err(error) = ensure_recovered(&st, &handle).await {
                                tracing::warn!(workspace = id, ?error, "job recovery failed");
                            }
                        }
                        Err(error) => {
                            tracing::warn!(workspace = id, ?error, "workspace recovery failed")
                        }
                    }
                }
            }
            Err(error) => tracing::warn!(?error, "job recovery failed"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Method, Request};
    use http_body_util::BodyExt;
    use lp_cli_driver::accounts::{AccountPool, Strategy};
    use lp_cli_driver::engine::{StreamDelta, ToolCall};
    use lp_core::ports::{DeliveryStatus, JobKind, JobRecord, JobStatus, JobStore};
    use lp_service::chat::{RuntimeConfig, Script};
    use lp_service::events::EventHub;
    use serde_json::{json, Value};
    use std::time::Duration;
    use tower::ServiceExt;

    const HOST: &str = "127.0.0.1:7878";
    const TEACH: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../lp-service/tests/faux/teach.json"
    ));
    const PRACTICE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../lp-service/tests/faux/practice.json"
    ));
    const RATE: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../lp-service/tests/faux/rate-recover.json"
    ));
    const CRASH: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../lp-service/tests/faux/crash-resume.json"
    ));

    struct Pdf;
    impl lp_agent::workbench::Pdf for Pdf {
        fn pages(&self, _: &str) -> Result<Vec<String>, String> {
            Ok(vec!["One owner per value.".into()])
        }
    }

    struct Api {
        dir: tempfile::TempDir,
        st: AppState,
        ws: String,
    }
    impl Drop for Api {
        fn drop(&mut self) {
            self.st.shutdown.cancel();
        }
    }

    impl Api {
        async fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path().to_owned();
            let hub = Arc::new(EventHub::new());
            let directory = crate::directory::RegistryDirectory::open(
                root.clone(),
                hub.clone(),
                root.clone(),
                None,
            )
            .unwrap();
            let st = AppState::new(
                crate::app::ServerConfig {
                    port: 7878,
                    token: None,
                    engine: crate::app::EngineMode::Faux,
                    data_root: root,
                    heartbeat: Duration::from_secs(15),
                },
                hub,
                Arc::new(directory),
                Arc::new(AccountPool::new(vec![], Strategy::Priority)),
                crate::assets::Assets::Dir(dir.path().join("dist")),
                "test".into(),
            )
            .with_chat_config(RuntimeConfig {
                pdf: Arc::new(Pdf),
                ..RuntimeConfig::default()
            });
            let mut api = Self {
                dir,
                st,
                ws: String::new(),
            };
            let (status, v) = api
                .json(Method::POST, "/workspaces", json!({"name":"Classroom"}))
                .await;
            assert_eq!(status, StatusCode::CREATED, "{v}");
            api.ws = v["id"].as_str().unwrap().to_owned();
            api
        }
        fn path(&self, suffix: &str) -> String {
            format!("/workspaces/{}{suffix}", self.ws)
        }
        fn request(
            &self,
            method: Method,
            path: &str,
            content_type: &str,
            body: Body,
        ) -> Request<Body> {
            Request::builder()
                .method(method)
                .uri(format!("/api/v1{path}"))
                .header(header::HOST, HOST)
                .header(header::ORIGIN, format!("http://{HOST}"))
                .header("x-lp-csrf", "1")
                .header(header::CONTENT_TYPE, content_type)
                .body(body)
                .unwrap()
        }
        async fn raw(
            &self,
            method: Method,
            path: &str,
            content_type: &str,
            body: Body,
        ) -> Response {
            crate::app::router(self.st.clone())
                .oneshot(self.request(method, path, content_type, body))
                .await
                .unwrap()
        }
        async fn json(&self, method: Method, path: &str, body: Value) -> (StatusCode, Value) {
            let res = self
                .raw(
                    method,
                    path,
                    "application/json",
                    Body::from(body.to_string()),
                )
                .await;
            let status = res.status();
            let bytes = res.into_body().collect().await.unwrap().to_bytes();
            let data = serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&bytes)));
            (status, data)
        }
        async fn get(&self, suffix: &str) -> Value {
            let (status, v) = self
                .json(Method::GET, &self.path(suffix), Value::Null)
                .await;
            assert_eq!(status, StatusCode::OK, "{v}");
            v
        }
        async fn post(&self, suffix: &str, v: Value) -> (StatusCode, Value) {
            self.json(Method::POST, &self.path(suffix), v).await
        }
        async fn create(&self, kind: &str) -> String {
            let (status, v) = self
                .post("/chats", json!({"kind":kind,"topic":"Ownership"}))
                .await;
            assert_eq!(status, StatusCode::ACCEPTED, "{v}");
            v["chatId"].as_str().unwrap().to_owned()
        }
        async fn state(&self, chat: &str, want: &str) -> Value {
            tokio::time::timeout(Duration::from_secs(8), async {
                loop {
                    let v = self.get(&format!("/chats/{chat}")).await;
                    if v["turn"]["state"] == want {
                        return v;
                    }
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
            })
            .await
            .unwrap_or_else(|_| panic!("chat {chat} never became {want}"))
        }
        async fn answer(&self, chat: &str, call: &Value, client: &str, answer: Value) -> Value {
            let (status, v) = self
                .post(
                    &format!("/chats/{chat}/turn"),
                    json!({"type":"answer","callId":call,"clientId":client,"answer":answer}),
                )
                .await;
            assert_eq!(status, StatusCode::ACCEPTED, "{v}");
            v
        }
        fn script(&self, text: &str) {
            Script::parse(text)
                .unwrap()
                .apply(self.st.faux(), self.st.chats.faults());
        }
        fn stores(&self) -> lp_service::stores::Stores {
            lp_service::stores::Stores::open(&lp_service::paths::workspace_dir(
                self.dir.path(),
                &self.ws,
            ))
            .unwrap()
        }
        async fn events(&self) -> Body {
            let path = format!("/events?workspace={}", self.ws);
            let res = self
                .raw(Method::GET, &path, "application/json", Body::empty())
                .await;
            assert_eq!(res.status(), StatusCode::OK);
            res.into_body()
        }
    }

    async fn through_rest(body: &mut Body) -> Vec<(Option<u64>, String, Value)> {
        through_event(body, "chat.rest").await
    }

    async fn through_event(body: &mut Body, target: &str) -> Vec<(Option<u64>, String, Value)> {
        tokio::time::timeout(Duration::from_secs(8), async {
            let mut out = Vec::new();
            let mut pending = String::new();
            loop {
                let frame = body.frame().await.unwrap().unwrap();
                if let Ok(data) = frame.into_data() {
                    pending.push_str(std::str::from_utf8(&data).unwrap());
                }
                while let Some(end) = pending.find("\n\n") {
                    let frame = pending[..end].to_owned();
                    pending.drain(..end + 2);
                    let mut id = None;
                    let mut event = String::new();
                    let mut data = Value::Null;
                    for line in frame.lines() {
                        if let Some(s) = line.strip_prefix("id: ") {
                            id = s.parse().ok();
                        }
                        if let Some(s) = line.strip_prefix("event: ") {
                            event = s.to_owned();
                        }
                        if let Some(s) = line.strip_prefix("data: ") {
                            data = serde_json::from_str(s).unwrap();
                        }
                    }
                    let rest = event == target;
                    out.push((id, event, data));
                    if rest {
                        return out;
                    }
                }
            }
        })
        .await
        .expect("SSE did not reach chat.rest")
    }

    #[tokio::test]
    async fn full_probe_plan_teach_over_http_and_sse() {
        let api = Api::new().await;
        api.script(TEACH);
        let mut events = api.events().await;
        let chat = api.create("teach").await;
        let first = through_rest(&mut events).await;
        let names: Vec<_> = first.iter().map(|e| e.1.as_str()).collect();
        assert!(names.contains(&"chat.created") && names.contains(&"chat.delta"));
        assert!(
            names.iter().position(|n| *n == "chat.delta")
                < names.iter().position(|n| *n == "chat.toolCall")
        );
        assert_eq!(names.last(), Some(&"chat.rest"));
        let probe = api.state(&chat, "idle").await;
        assert_eq!(probe["pending"]["tool"], "probe");
        assert_eq!(probe["probeProgress"], json!({"asked":1,"max":8}));
        assert!(!probe.to_string().contains("PROBE_SECRET"));
        api.answer(
            &chat,
            &probe["pending"]["callId"],
            "answer-probe",
            json!({"choice":["a"]}),
        )
        .await;
        let second = through_rest(&mut events).await;
        let teach = api.state(&chat, "idle").await;
        assert_eq!(teach["phase"], "teach");
        assert_eq!(teach["pending"]["tool"], "quiz");
        assert_eq!(teach["plan"]["nodes"][0]["status"], "current");
        assert_eq!(teach["plan"]["nodes"][1]["status"], "upcoming");
        assert!(!teach.to_string().contains("QUIZ_SECRET"));
        assert!(teach["viewVersion"].as_u64() > probe["viewVersion"].as_u64());
        api.answer(
            &chat,
            &teach["pending"]["callId"],
            "answer-quiz",
            json!({"choice":["a"],"confidence":5}),
        )
        .await;
        let third = through_rest(&mut events).await;
        let last = api.state(&chat, "idle").await;
        assert_eq!(last["pending"]["tool"], "explain");
        assert_eq!(last["plan"]["nodes"][0]["status"], "visited");
        assert_eq!(last["plan"]["nodes"][1]["status"], "current");
        assert!(last.to_string().contains("QUIZ_SECRET"));
        let ids: Vec<_> = first
            .iter()
            .chain(&second)
            .chain(&third)
            .filter_map(|e| e.0)
            .collect();
        assert!(ids.windows(2).all(|w| w[0] < w[1]));
        for events in [&first, &second, &third] {
            let seq: Vec<_> = events
                .iter()
                .filter(|e| e.1 == "chat.delta")
                .map(|e| e.2["seq"].as_u64().unwrap())
                .collect();
            assert_eq!(seq, (1..=seq.len() as u64).collect::<Vec<_>>());
        }
        let export = api
            .raw(
                Method::GET,
                &api.path(&format!("/chats/{chat}/export.md")),
                "application/json",
                Body::empty(),
            )
            .await;
        assert_eq!(
            export.headers()[header::CONTENT_TYPE],
            "text/markdown; charset=utf-8"
        );
        let text = String::from_utf8(
            export
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(text.contains("**Correct.** QUIZ_SECRET"));
    }

    #[tokio::test]
    async fn practice_problem_gate_and_conclude() {
        let api = Api::new().await;
        api.script(PRACTICE);
        let chat = api.create("practice").await;
        let p = api.state(&chat, "idle").await;
        assert_eq!(p["pending"]["problem"]["mode"], "composition");
        assert!(!p.to_string().contains("PRACTICE_SECRET"));
        api.answer(
            &chat,
            &p["pending"]["callId"],
            "attempt",
            json!({"text":"[3,2,1]","hint":true}),
        )
        .await;
        let done = api.state(&chat, "idle").await;
        assert_eq!(done["phase"], "done");
        assert!(done["pending"].is_null());
        assert!(done.to_string().contains("PRACTICE_SECRET"));
        let rec = api.stores().chats.load_chat(&chat).unwrap();
        assert!(rec.body.to_string().contains("a problem is already open"));
        assert_eq!(done["problemResult"]["outcome"], "solvedWithHints");
    }

    #[tokio::test]
    async fn double_post_idempotency_steer_stale_and_cancel_keep_partial() {
        let api = Api::new().await;
        let _hold = api.st.faux().faux_gated_turn(vec![
            StreamDelta::Text("Partial lesson".into()),
            StreamDelta::Text(" remainder".into()),
        ]);
        let mut events = api.events().await;
        let chat = api.create("teach").await;
        through_event(&mut events, "chat.delta").await;
        let running = api.state(&chat, "running").await;
        let (status, duplicate) = api
            .post(
                &format!("/chats/{chat}/turn"),
                json!({
                    "type":"user", "text":"retry", "clientId":format!("create:{chat}")
                }),
            )
            .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(duplicate["turnId"], running["turn"]["turnId"]);
        assert_eq!(duplicate["status"], "duplicate");
        let (status, v) = api
            .post(
                &format!("/chats/{chat}/turn"),
                json!({"type":"user","text":"one","clientId":"double"}),
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(v["error"]["code"], "turn_in_progress");
        let (status, v) = api
            .post(
                &format!("/chats/{chat}/turn"),
                json!({"type":"answer","callId":"call_gone","answer":{"choice":[],"text":"x"},"clientId":"late"}),
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(v["error"]["code"], "stale_question");
        let (status, _) = api
            .post(
                &format!("/chats/{chat}/turn"),
                json!({"type":"steer","text":"Use concrete examples","clientId":"steer-1"}),
            )
            .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let (status, v) = api.post(&format!("/chats/{chat}/cancel"), json!({})).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(v["cancelled"], true);
        through_rest(&mut events).await;
        let cancelled = api.state(&chat, "idle").await;
        assert!(cancelled["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["text"] == "Partial lesson" && m["interrupted"] == true));
        let (status, v) = api
            .post(
                &format!("/chats/{chat}/turn"),
                json!({"type":"steer","text":"Use concrete examples","clientId":"steer-1"}),
            )
            .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(v["status"], "duplicate");
        api.st.faux().faux_tool_call(
            "recall",
            json!({"question":"State ownership","modelAnswer":"ONE_SECRET"}),
        );
        let payload = json!({"type":"user","text":"Try again","clientId":"once"});
        let (status, started) = api
            .post(&format!("/chats/{chat}/turn"), payload.clone())
            .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let pending = api.state(&chat, "idle").await;
        let (status, dup) = api.post(&format!("/chats/{chat}/turn"), payload).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert_eq!(dup["turnId"], started["turnId"]);
        let (status, stale) = api
            .post(
                &format!("/chats/{chat}/turn"),
                json!({"type":"answer","callId":"old","clientId":"stale","answer":{"text":"one"}}),
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(stale["error"]["code"], "stale_question");
        assert_eq!(
            stale["error"]["details"]["view"]["pending"],
            pending["pending"]
        );
        assert!(!stale.to_string().contains("ONE_SECRET"));
    }

    #[tokio::test]
    async fn rate_limit_queues_recovers_and_crash_retries_same_client() {
        let api = Api::new().await;
        api.script(RATE);
        let mut events = api.events().await;
        let chat = api.create("teach").await;
        let queued = api.state(&chat, "queued").await;
        assert!(queued["turn"]["retryAt"].as_i64().is_some());
        through_rest(&mut events).await;
        let recovered = api.state(&chat, "idle").await;
        assert!(recovered.to_string().contains("Recovered after"));
        api.script(CRASH);
        let payload = json!({"type":"user","text":"Continue","clientId":"retry-me"});
        assert_eq!(
            api.post(&format!("/chats/{chat}/turn"), payload.clone())
                .await
                .0,
            StatusCode::ACCEPTED
        );
        let error = api.state(&chat, "error").await;
        assert_eq!(error["turn"]["error"]["retryable"], true);
        assert_eq!(error["turn"]["error"]["code"], "engine_unavailable");
        assert!(error["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["interrupted"] == true && m["text"] == "Text before the engine crashed"));
        let failed = through_rest(&mut events).await;
        let order: Vec<_> = failed.iter().map(|e| e.1.as_str()).collect();
        assert!(
            order.iter().position(|s| *s == "chat.error")
                < order.iter().position(|s| *s == "chat.rest")
        );
        assert_eq!(
            api.post(&format!("/chats/{chat}/turn"), payload).await.0,
            StatusCode::ACCEPTED
        );
        through_rest(&mut events).await;
        let resumed = api.state(&chat, "idle").await;
        assert_eq!(
            resumed["messages"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|m| m["text"] == "Continue")
                .count(),
            1
        );
    }

    #[tokio::test]
    async fn restart_mid_turn_exposes_interrupted_and_resume() {
        let mut api = Api::new().await;
        let _hold = api.st.faux().faux_gated_turn(vec![
            StreamDelta::Text("Before restart".into()),
            StreamDelta::Text("never".into()),
        ]);
        let chat = api.create("teach").await;
        api.state(&chat, "running").await;
        api.st.shutdown.cancel();
        api.state(&chat, "error").await;
        let mut restarted = api.st.clone().with_chat_config(RuntimeConfig::default());
        restarted.shutdown = tokio_util::sync::CancellationToken::new();
        api.st = restarted;
        let interrupted = api.state(&chat, "error").await;
        assert_eq!(interrupted["turn"]["error"]["interrupted"], true);
        api.st.faux().faux_text("Back after restart");
        let (status, _) = api
            .post(
                &format!("/chats/{chat}/turn"),
                json!({"type":"resume","clientId":"resume-1"}),
            )
            .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let resumed = api.state(&chat, "idle").await;
        assert!(resumed.to_string().contains("Back after restart"));
    }

    fn upload_body(name: &str, data: &[u8]) -> Vec<u8> {
        let mut body=format!("--pdfboundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"{name}\"\r\nContent-Type: application/pdf\r\n\r\n").into_bytes();
        body.extend_from_slice(data);
        body.extend_from_slice(b"\r\n--pdfboundary--\r\n");
        body
    }

    #[tokio::test]
    async fn note_upload_validates_persists_and_rejects_busy_chat() {
        let api = Api::new().await;
        api.st.faux().faux_text("Ready for notes");
        let chat = api.create("teach").await;
        api.state(&chat, "idle").await;
        let path = api.path(&format!("/sources/note?chat={chat}"));
        for (filename, data, expected) in [
            ("bad.pdf", b"hello".as_slice(), StatusCode::BAD_REQUEST),
            ("bad.md", b"\xff".as_slice(), StatusCode::BAD_REQUEST),
            ("empty.txt", b"  ".as_slice(), StatusCode::BAD_REQUEST),
            ("binary.txt", b"a\0b".as_slice(), StatusCode::BAD_REQUEST),
        ] {
            let res = api
                .raw(
                    Method::POST,
                    &path,
                    "multipart/form-data; boundary=pdfboundary",
                    Body::from(upload_body(filename, data)),
                )
                .await;
            assert_eq!(res.status(), expected);
        }
        let oversized = vec![b'x'; 1024 * 1024 + 1];
        let res = api
            .raw(
                Method::POST,
                &path,
                "multipart/form-data; boundary=pdfboundary",
                Body::from(upload_body("large.md", &oversized)),
            )
            .await;
        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
        for name in ["notes.md", "NOTES.TXT"] {
            let text = "# My notes\nOwnership means one owner. 🐶";
            let res = api
                .raw(
                    Method::POST,
                    &path,
                    "multipart/form-data; boundary=pdfboundary",
                    Body::from(upload_body(name, text.as_bytes())),
                )
                .await;
            assert_eq!(res.status(), StatusCode::CREATED);
            let uploaded: Value =
                serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            assert_eq!(uploaded["bytes"], text.len());
            let artifact = api
                .raw(
                    Method::GET,
                    &api.path(&format!(
                        "/artifacts/{}",
                        uploaded["artifactId"].as_str().unwrap()
                    )),
                    "application/json",
                    Body::empty(),
                )
                .await;
            assert_eq!(artifact.status(), StatusCode::OK);
            assert_eq!(
                artifact.into_body().collect().await.unwrap().to_bytes(),
                text.as_bytes()
            );
            let view = api.state(&chat, "idle").await;
            assert!(view["messages"].as_array().unwrap().iter().any(
                |m| m["role"] == "user" && m["text"].as_str().is_some_and(|t| t.contains(text))
            ));
        }
        let missing = api
            .raw(
                Method::POST,
                &api.path("/sources/note?chat=missing"),
                "multipart/form-data; boundary=pdfboundary",
                Body::from(upload_body("note.md", b"notes")),
            )
            .await;
        assert_eq!(missing.status(), StatusCode::NOT_FOUND);
        let _hold = api.st.faux().faux_gated_turn(vec![
            StreamDelta::Text("Thinking".into()),
            StreamDelta::Text("still".into()),
        ]);
        let busy = api.create("teach").await;
        api.state(&busy, "running").await;
        let res = api
            .raw(
                Method::POST,
                &api.path(&format!("/sources/note?chat={busy}")),
                "multipart/form-data; boundary=pdfboundary",
                Body::from(upload_body("note.md", b"notes")),
            )
            .await;
        assert_eq!(res.status(), StatusCode::CONFLICT);
        api.st.shutdown.cancel();
        api.st.chats.wait_idle().await;
    }

    #[tokio::test]
    async fn pdf_upload_job_lifecycle_abort_and_artifact_sandbox() {
        let api = Api::new().await;
        api.st.faux().faux_text("Ready for a document");
        let chat = api.create("teach").await;
        api.state(&chat, "idle").await;
        let path = api.path(&format!("/sources/pdf?chat={chat}"));
        for (filename, data) in [
            ("bad.pdf", b"no pdf".as_slice()),
            ("bad.txt", b"%PDF-1.7".as_slice()),
        ] {
            let res = api
                .raw(
                    Method::POST,
                    &path,
                    "multipart/form-data; boundary=pdfboundary",
                    Body::from(upload_body(filename, data)),
                )
                .await;
            assert_eq!(res.status(), StatusCode::BAD_REQUEST);
        }
        let oversize = vec![b'x'; super::super::sources::MAX_PDF_BYTES + 1];
        let res = api
            .raw(
                Method::POST,
                &path,
                "multipart/form-data; boundary=pdfboundary",
                Body::from(upload_body("large.pdf", &oversize)),
            )
            .await;
        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
        let _hold = api.st.faux().faux_gated_turn(vec![
            StreamDelta::Text("Reading".into()),
            StreamDelta::Text("stopped".into()),
        ]);
        let res = api
            .raw(
                Method::POST,
                &path,
                "multipart/form-data; boundary=pdfboundary",
                Body::from(upload_body("paper.pdf", b"%PDF-1.7\nfixture")),
            )
            .await;
        assert_eq!(res.status(), StatusCode::ACCEPTED);
        let uploaded: Value =
            serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
        let job = uploaded["jobId"].as_str().unwrap();
        let pdf = api
            .raw(
                Method::GET,
                &api.path(&format!(
                    "/artifacts/{}",
                    uploaded["artifactId"].as_str().unwrap()
                )),
                "application/json",
                Body::empty(),
            )
            .await;
        assert_eq!(pdf.headers()[header::CONTENT_TYPE], "application/pdf");
        tokio::time::timeout(Duration::from_secs(5), async {
            while api.st.chats.permits().available() == api.st.chats.permits().size() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .unwrap();
        assert_eq!(
            api.post(&format!("/jobs/{job}/abort"), json!({})).await.0,
            StatusCode::OK
        );
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let jobs = api.get("/jobs").await;
                if jobs["items"][0]["status"] == "failed" {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        // A second PDF completes, produces Markdown, and wakes the chat exactly once.
        api.st
            .faux()
            .faux_turn(vec![StreamDelta::ToolCall(ToolCall {
                id: "read".into(),
                name: "read".into(),
                input: json!({"page":1}),
            })]);
        api.st
            .faux()
            .faux_tool_call("submit", json!({"brief":"One owner per value"}));
        api.st.faux().faux_text("The document explains ownership");
        let res = api
            .raw(
                Method::POST,
                &path,
                "multipart/form-data; boundary=pdfboundary",
                Body::from(upload_body("paper.pdf", b"%PDF-1.7\nfixture")),
            )
            .await;
        assert_eq!(res.status(), StatusCode::ACCEPTED);
        let uploaded: Value =
            serde_json::from_slice(&res.into_body().collect().await.unwrap().to_bytes()).unwrap();
        let completed = tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let jobs = api.get("/jobs").await;
                if let Some(job) = jobs["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .find(|j| j["id"] == uploaded["jobId"] && j["status"] == "done")
                {
                    break job.clone();
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let artifact = completed["artifactId"].as_str().unwrap();
        let md = api
            .raw(
                Method::GET,
                &api.path(&format!("/artifacts/{artifact}")),
                "application/json",
                Body::empty(),
            )
            .await;
        assert_eq!(
            md.headers()[header::CONTENT_TYPE],
            "text/markdown; charset=utf-8"
        );
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let v = api.get(&format!("/chats/{chat}")).await;
                if v["turn"]["state"] == "idle"
                    && v.to_string().contains("The document explains ownership")
                {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
        let dir = lp_service::paths::workspace_dir(api.dir.path(), &api.ws).join("artifacts");
        std::fs::write(
            dir.join("demo.html"),
            "<script>window.parent.alert(1)</script>",
        )
        .unwrap();
        let html = api
            .raw(
                Method::GET,
                &api.path("/artifacts/demo"),
                "application/json",
                Body::empty(),
            )
            .await;
        assert_eq!(html.headers()[header::CONTENT_SECURITY_POLICY], "sandbox");
        assert_eq!(html.headers()[header::X_CONTENT_TYPE_OPTIONS], "nosniff");
        assert_eq!(
            html.headers()[header::CONTENT_TYPE],
            "text/html; charset=utf-8"
        );
        #[cfg(unix)]
        {
            std::fs::write(api.dir.path().join("outside.html"), "outside").unwrap();
            std::os::unix::fs::symlink(
                api.dir.path().join("outside.html"),
                dir.join("escape.html"),
            )
            .unwrap();
            assert_eq!(
                api.raw(
                    Method::GET,
                    &api.path("/artifacts/escape"),
                    "application/json",
                    Body::empty()
                )
                .await
                .status(),
                StatusCode::NOT_FOUND
            );
        }
    }

    #[tokio::test]
    async fn boot_recovers_interrupted_jobs_without_opening_a_chat() {
        let mut api = Api::new().await;
        api.st.faux().faux_text("initial");
        let chat = api.create("teach").await;
        api.state(&chat, "idle").await;
        api.st.shutdown.cancel();
        api.st = api.st.clone().with_chat_config(RuntimeConfig::default());
        api.st.shutdown = tokio_util::sync::CancellationToken::new();
        api.stores()
            .jobs
            .create_job(&JobRecord {
                id: "job_interrupted".into(),
                chat_id: chat.as_str().into(),
                kind: JobKind::Pdf,
                status: JobStatus::Running,
                delivery: DeliveryStatus::Pending,
                created_at: 1,
                result: None,
                usage: vec![],
            })
            .unwrap();
        api.st.faux().faux_text("The reading was interrupted");
        let _router = crate::app::router(api.st.clone());
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                if api.stores().jobs.all_report().unwrap().0.iter().any(|j| {
                    j.status == JobStatus::Done
                        && j.result
                            .as_ref()
                            .is_some_and(|r| r["error"] == "interrupted by a restart")
                }) {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn delta_budget_combines_thinking_and_text_and_seq_survives_replay_gaps() {
        let api = Api::new().await;
        let mut events = api.events().await;
        let deltas = (0..80)
            .flat_map(|_| {
                [
                    StreamDelta::Thinking("h".into()),
                    StreamDelta::Text("t".into()),
                ]
            })
            .collect();
        api.st.faux().faux_turn(deltas);
        let chat = api.create("teach").await;
        let started = tokio::time::Instant::now();
        let all = through_rest(&mut events).await;
        api.state(&chat, "idle").await;
        assert!(
            started.elapsed() >= Duration::from_millis(5_250),
            "combined delta budget exceeded"
        );
        let deltas: Vec<_> = all.iter().filter(|e| e.1 == "chat.delta").collect();
        assert_eq!(deltas.len(), 160);
        assert_eq!(
            deltas
                .iter()
                .filter(|e| e.2["kind"] == "text")
                .map(|e| e.2["text"].as_str().unwrap().len())
                .sum::<usize>(),
            80
        );
        assert!(deltas
            .windows(2)
            .all(|w| w[0].2["seq"].as_u64() < w[1].2["seq"].as_u64()));
        // Dropping events leaves sequence gaps, never a reversed sequence; the persisted view is complete.
        let dropped: Vec<_> = deltas
            .iter()
            .step_by(3)
            .map(|e| e.2["seq"].as_u64().unwrap())
            .collect();
        assert!(dropped.windows(2).all(|w| w[1] - w[0] == 3));
    }

    #[tokio::test]
    async fn global_cli_cap_queues_another_chat_and_cancel_unblocks_it() {
        let api = Api::new().await;
        let settings = lp_service::settings::Settings {
            concurrency_cap: 1,
            ..Default::default()
        };
        settings.save(api.dir.path()).unwrap();
        let _hold = api.st.faux().faux_gated_turn(vec![
            StreamDelta::Text("holding".into()),
            StreamDelta::Text("forever".into()),
        ]);
        let mut events = api.events().await;
        let first = api.create("teach").await;
        through_event(&mut events, "chat.delta").await;
        api.state(&first, "running").await;
        api.st.faux().faux_text("second chat");
        let second = api.create("teach").await;
        api.state(&second, "queued").await;
        api.post(&format!("/chats/{first}/cancel"), json!({})).await;
        api.state(&second, "idle").await;
    }
    #[tokio::test]
    async fn long_cooldown_surfaces_retry_after_and_unknown_tools_are_generic_chips() {
        let mut api = Api::new().await;
        api.st = api.st.clone().with_chat_config(RuntimeConfig {
            queue_limit: Duration::from_millis(40),
            ..RuntimeConfig::default()
        });
        api.st.chats.faults().schedule(
            0,
            chat::Fault {
                kind: chat::FaultKind::RateLimit,
                message: Some("rate limit; try again in 5 minutes".into()),
                partial_text: None,
            },
        );
        let chat = api.create("teach").await;
        let failed = api.state(&chat, "error").await;
        assert_eq!(failed["turn"]["error"]["code"], "rate_limited");
        assert!((299_900..=300_000)
            .contains(&failed["turn"]["error"]["retryAfterMs"].as_u64().unwrap()));
        api.st
            .faux()
            .faux_tool_call("future_tool", json!({"arbitrary":"untrusted"}));
        api.st
            .faux()
            .faux_text("I can continue after an unknown tool");
        api.post(
            &format!("/chats/{chat}/turn"),
            json!({"type":"resume","clientId":"unknown-retry"}),
        )
        .await;
        let view = api.state(&chat, "idle").await;
        assert!(view["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["calls"]
                .as_array()
                .is_some_and(|c| c.iter().any(|c| c["name"] == "future_tool"))));
        assert!(!view.to_string().contains("untrusted"));
    }

    #[tokio::test]
    async fn probe_cap_forces_teacher_handoff_and_compaction_keeps_pending_question() {
        let api = Api::new().await;
        let probe = json!({"question":"Owners?","options":[{"label":"One","value":"a"},{"label":"Two","value":"b"}],"correctAnswer":"a","explanation":"one"});
        for _ in 0..8 {
            api.st.faux().faux_tool_call("probe", probe.clone());
        }
        api.st.faux().faux_tool_call(
            "plan_init",
            json!({"mermaid":"graph TD\nown[\"Ownership\"]"}),
        );
        api.st.faux().faux_tool_call(
            "explain",
            json!({"type":"check","nodeId":"own","content":"Explain ownership"}),
        );
        let chat = api.create("teach").await;
        for i in 0..8 {
            let view = api.state(&chat, "idle").await;
            assert_eq!(view["probeProgress"]["asked"], i + 1);
            api.answer(
                &chat,
                &view["pending"]["callId"],
                &format!("probe-{i}"),
                json!({"choice":["a"]}),
            )
            .await;
        }
        let taught = api.state(&chat, "idle").await;
        assert_eq!(taught["phase"], "teach");
        assert_eq!(taught["probeProgress"]["asked"], 8);
        // Grow the old transcript through the storage port; the turn compacts it at rest.
        let stores = api.stores();
        let mut rec = stores.chats.load_chat(&chat).unwrap();
        let mut session = session_of(&rec);
        session.messages.splice(
            0..0,
            (0..40).map(|_| lp_agent::loops::Msg::User("old context ".repeat(800))),
        );
        let meta = chat::ChatMeta::of(&rec.body);
        rec.body = meta.body(&session).unwrap();
        stores.chats.save_chat(&rec).unwrap();
        api.st.faux().faux_tool_call(
            "explain",
            json!({"type":"check","nodeId":"own","content":"Explain again"}),
        );
        api.st
            .faux()
            .faux_text("Earlier: the learner practised ownership.");
        api.answer(
            &chat,
            &taught["pending"]["callId"],
            "compact",
            json!({"text":"One owner"}),
        )
        .await;
        let compacted = api.state(&chat, "idle").await;
        assert_eq!(compacted["pending"]["tool"], "explain");
        assert!(compacted["messages"]
            .as_array()
            .unwrap()
            .iter()
            .any(|m| m["role"] == "compaction"));
        assert!(compacted["messages"].as_array().unwrap().len() < 40);
    }

    #[tokio::test]
    async fn burst_deltas_coalesce_and_sse_replays_in_original_order() {
        let api = Api::new().await;
        let mut stream = api.events().await;
        api.st
            .faux()
            .faux_turn((0..2000).map(|_| StreamDelta::Text("x".into())).collect());
        let chat = api.create("teach").await;
        let events = through_rest(&mut stream).await;
        let deltas: Vec<_> = events.iter().filter(|e| e.1 == "chat.delta").collect();
        assert!(
            deltas.len() < 10,
            "{} delta frames for a burst",
            deltas.len()
        );
        assert_eq!(
            deltas
                .iter()
                .map(|e| e.2["text"].as_str().unwrap().len())
                .sum::<usize>(),
            2000
        );
        assert!(api
            .state(&chat, "idle")
            .await
            .to_string()
            .contains(&"x".repeat(2000)));
        let last = deltas[0].0.unwrap();
        let mut request = api.request(
            Method::GET,
            &format!("/events?workspace={}", api.ws),
            "application/json",
            Body::empty(),
        );
        request
            .headers_mut()
            .insert("last-event-id", last.to_string().parse().unwrap());
        let res = crate::app::router(api.st.clone())
            .oneshot(request)
            .await
            .unwrap();
        let replay = through_rest(&mut res.into_body()).await;
        let ids: Vec<_> = replay.iter().filter_map(|e| e.0).collect();
        assert_eq!(
            ids,
            events
                .iter()
                .filter_map(|e| e.0)
                .filter(|id| *id > last)
                .collect::<Vec<_>>()
        );
    }
}
