//! Background archive exports and bounded, disk-streamed multipart imports.
use super::common::{blocking, core, open_ws};
use crate::{
    app::AppState,
    dto::mutations::*,
    error::{ApiError, ErrorEnvelope},
};
use axum::{
    body::Body,
    extract::{DefaultBodyLimit, Multipart, Path, State},
    http::{header, HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use lp_service::{
    events::{Event, JobInfo, Scope},
    export_jobs::{self, Download, MAX_IMPORT_BYTES},
};
use tokio::io::AsyncWriteExt;
use tokio_util::io::ReaderStream;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces/{ws}/export", post(export))
        .route("/downloads/{token}", get(download))
        .route(
            "/workspaces/import",
            post(import).layer(DefaultBodyLimit::disable()),
        )
}
#[utoipa::path(post, path="/workspaces/{ws}/export", tag="workspaces", params(("ws"=String, Path)), responses((status=202, body=ExportStarted),(status=404, body=ErrorEnvelope)))]
pub async fn export(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<(StatusCode, Json<ExportStarted>), ApiError> {
    let h = open_ws(&st, &ws).await?;
    let c = core(&st);
    let job_id = format!("export_{:032x}", rand::random::<u128>());
    let job = job_id.clone();
    let busy = h.busy();
    std::mem::drop(tokio::task::spawn_blocking(move || {
        let _busy = busy;
        let scope = Scope::Workspace(h.id().to_owned());
        let graph_lock = c.graph_mutations.workspace_lock(h.root());
        let _graph_lock = graph_lock.lock();
        let result = export_jobs::job_path(c.root(), &job)
            .and_then(|path| c.registry.export(h.id(), &path).map_err(Into::into));
        match result {
            Ok(report) => {
                let token = c.export_jobs.ready(report.path);
                c.hub
                    .publish(scope, Event::ExportReady { job_id: job, token });
            }
            Err(e) => {
                c.hub.publish(
                    scope,
                    Event::JobUpdated {
                        job: JobInfo {
                            id: job,
                            kind: "export".into(),
                            status: "error".into(),
                            title: format!("Export failed: {e}"),
                            artifact_id: None,
                        },
                    },
                );
            }
        }
    }));
    Ok((StatusCode::ACCEPTED, Json(ExportStarted { job_id })))
}
#[utoipa::path(get, path="/downloads/{token}", tag="workspaces", params(("token"=String, Path)), responses((status=200, description="One-shot streamed workspace zip", content_type="application/zip", body=String),(status=404, body=ErrorEnvelope)))]
pub async fn download(
    State(st): State<AppState>,
    Path(token): Path<String>,
) -> Result<Response, ApiError> {
    let jobs = core(&st).export_jobs.clone();
    let download = blocking(move || {
        jobs.take(&token)
            .ok_or_else(|| ApiError::not_found("download token expired or already used"))
    })
    .await?;
    let file = tokio::fs::File::open(&download.path)
        .await
        .map_err(|e| ApiError::from(anyhow::Error::from(e)))?;
    // The guard lives in the stream, including if a client disconnects early.
    let stream = futures::stream::unfold(
        (ReaderStream::new(file), download),
        |(mut reader, guard)| async move {
            use futures::StreamExt;
            reader.next().await.map(|chunk| (chunk, (reader, guard)))
        },
    );
    Ok((
        [
            (header::CONTENT_TYPE, "application/zip"),
            (
                header::CONTENT_DISPOSITION,
                "attachment; filename=workspace.zip",
            ),
            (header::CACHE_CONTROL, "no-store"),
        ],
        Body::from_stream(stream),
    )
        .into_response())
}
#[utoipa::path(post, path="/workspaces/import", tag="workspaces", request_body(content=ImportArchive, content_type="multipart/form-data", description="A single `file` zip field, maximum 2 GiB; streamed to disk"), responses((status=201, body=ImportedWorkspace),(status=400, body=ErrorEnvelope),(status=413, body=ErrorEnvelope)))]
pub async fn import(
    State(st): State<AppState>,
    headers: HeaderMap,
    multipart: Result<Multipart, axum::extract::multipart::MultipartRejection>,
) -> Result<Response, ApiError> {
    import_with_limit(st, headers, multipart, MAX_IMPORT_BYTES).await
}

async fn import_with_limit(
    st: AppState,
    headers: HeaderMap,
    multipart: Result<Multipart, axum::extract::multipart::MultipartRejection>,
    limit: u64,
) -> Result<Response, ApiError> {
    if headers
        .get(header::CONTENT_LENGTH)
        .and_then(|h| h.to_str().ok())
        .and_then(|s| s.parse::<u64>().ok())
        .is_some_and(|n| n > limit + (1 << 20))
    {
        return Ok(too_large());
    }
    let mut multipart = multipart.map_err(|e| ApiError::invalid("file", e.to_string()))?;
    let c = core(&st);
    let dir_core = c.clone();
    let path = blocking(move || {
        Ok(export_jobs::job_path(
            dir_core.root(),
            &format!("upload_{:032x}", rand::random::<u128>()),
        )?)
    })
    .await?;
    let guard = Download { path };
    let mut file = tokio::fs::File::create(&guard.path)
        .await
        .map_err(|e| ApiError::from(anyhow::Error::from(e)))?;
    let (mut found, mut bytes) = (false, 0u64);
    while let Some(mut field) = multipart
        .next_field()
        .await
        .map_err(|e| ApiError::invalid("file", e.to_string()))?
    {
        if found || field.name() != Some("file") {
            return Err(ApiError::invalid(
                "file",
                "expected one zip field named file",
            ));
        }
        found = true;
        while let Some(chunk) = field
            .chunk()
            .await
            .map_err(|e| ApiError::invalid("file", e.to_string()))?
        {
            bytes = bytes.saturating_add(chunk.len() as u64);
            if bytes > limit {
                return Ok(too_large());
            }
            file.write_all(&chunk)
                .await
                .map_err(|e| ApiError::from(anyhow::Error::from(e)))?;
        }
    }
    if !found || bytes == 0 {
        return Err(ApiError::invalid("file", "upload a workspace zip"));
    }
    file.flush()
        .await
        .map_err(|e| ApiError::from(anyhow::Error::from(e)))?;
    drop(file);
    let imported = blocking(move || {
        let result = c.registry.import(&guard.path)?;
        Ok(ImportedWorkspace {
            workspace_id: result.meta.id,
        })
    })
    .await?;
    Ok((StatusCode::CREATED, Json(imported)).into_response())
}
fn too_large() -> Response {
    let mut res =
        ApiError::invalid("file", "workspace zip exceeds the 2 GiB upload limit").into_response();
    *res.status_mut() = StatusCode::PAYLOAD_TOO_LARGE;
    res
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        app::{router, EngineMode, ServerConfig},
        assets::Assets,
        directory::{RegistryDirectory, WorkspaceDirectory},
    };
    use axum::{
        body::Body,
        http::{Method, Request},
        response::Response,
    };
    use http_body_util::BodyExt;
    use lp_cli_driver::accounts::DiscoverRoots;
    use lp_core::{
        graph::{GraphChain, GraphNode},
        ports::{ChatStore, GraphStore, TrackerStore},
    };
    use lp_service::{
        events::{Event, EventHub, Subscription},
        workspace::{NewWorkspace, WorkspaceHandle},
    };
    use serde_json::{json, Value};
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::time::Duration;
    use tower::ServiceExt;

    struct Api {
        _dir: tempfile::TempDir,
        state: AppState,
        ws: Arc<WorkspaceHandle>,
        home: PathBuf,
    }
    impl Api {
        fn new() -> Self {
            Self::setup(false)
        }
        fn seeded() -> Self {
            Self::setup(true)
        }
        fn setup(seed: bool) -> Self {
            let dir = tempfile::tempdir().expect("tempdir");
            let root = dir.path().join("data");
            let home = dir.path().join("home");
            if seed {
                crate::seed::seed(crate::seed::Fixture::Small, &root).expect("seed stores");
            }
            let hub = Arc::new(EventHub::new());
            let directory = RegistryDirectory::open(root.clone(), hub.clone(), home.clone(), None)
                .expect("registry");
            let ws = if seed {
                directory
                    .core()
                    .registry
                    .open(crate::seed::MAIN_WS)
                    .expect("seed workspace")
            } else {
                directory
                    .core()
                    .registry
                    .create(NewWorkspace {
                        name: "Test".into(),
                        ..Default::default()
                    })
                    .expect("workspace")
            };
            let pool = lp_service::accounts::discover_pool(&DiscoverRoots {
                home: home.clone(),
                ..Default::default()
            });
            let state = AppState::new(
                ServerConfig {
                    port: 7878,
                    token: None,
                    engine: EngineMode::Faux,
                    data_root: root,
                    heartbeat: Duration::from_secs(15),
                },
                hub,
                Arc::new(directory),
                pool,
                Assets::Dir(dir.path().join("dist")),
                "test-device".into(),
            );
            Self {
                _dir: dir,
                state,
                ws,
                home,
            }
        }
        fn path(&self, suffix: &str) -> String {
            format!("/workspaces/{}{suffix}", self.ws.id())
        }
        fn request(
            &self,
            method: Method,
            path: &str,
            body: Body,
            content_type: Option<&str>,
        ) -> axum::http::request::Builder {
            let mut req = Request::builder()
                .method(method)
                .uri(format!("/api/v1{path}"))
                .header(header::HOST, "127.0.0.1:7878")
                .header(header::ORIGIN, "http://127.0.0.1:7878")
                .header("x-lp-csrf", "1");
            if let Some(ct) = content_type {
                req = req.header(header::CONTENT_TYPE, ct);
            }
            drop(body);
            req
        }
        async fn raw(&self, method: Method, path: &str, body: Body, ct: Option<&str>) -> Response {
            let req = self
                .request(method, path, Body::empty(), ct)
                .body(body)
                .expect("request");
            router(self.state.clone())
                .oneshot(req)
                .await
                .expect("response")
        }
        async fn call(&self, method: Method, suffix: &str, body: Value) -> (StatusCode, Value) {
            let response = self
                .raw(
                    method,
                    &self.path(suffix),
                    Body::from(body.to_string()),
                    Some("application/json"),
                )
                .await;
            let status = response.status();
            let bytes = response
                .into_body()
                .collect()
                .await
                .expect("body")
                .to_bytes();
            (
                status,
                serde_json::from_slice(&bytes).unwrap_or(Value::Null),
            )
        }
        fn node(&self, id: &str) {
            self.ws
                .stores()
                .graph
                .append_node(&GraphNode {
                    id: id.into(),
                    name: id.into(),
                    statement: format!("{id} proposition"),
                    aliases: vec![],
                    created_at: chrono::DateTime::UNIX_EPOCH,
                    provenance: vec![],
                })
                .expect("node");
        }
        fn chain(&self, id: &str, from: &str, to: &str) {
            self.ws
                .stores()
                .graph
                .append_chain(&GraphChain {
                    id: id.into(),
                    premises: vec![from.into()],
                    conclusion: to.into(),
                    provenance: vec![],
                    created_at: chrono::DateTime::UNIX_EPOCH,
                })
                .expect("chain");
        }
        async fn upload(&self, zip: &[u8]) -> (StatusCode, Value) {
            let mut body = b"--b22\r\nContent-Disposition: form-data; name=\"file\"; filename=\"workspace.zip\"\r\nContent-Type: application/zip\r\n\r\n".to_vec();
            body.extend_from_slice(zip);
            body.extend_from_slice(b"\r\n--b22--\r\n");
            let res = self
                .raw(
                    Method::POST,
                    "/workspaces/import",
                    Body::from(body),
                    Some("multipart/form-data; boundary=b22"),
                )
                .await;
            let status = res.status();
            let bytes = res
                .into_body()
                .collect()
                .await
                .expect("upload response")
                .to_bytes();
            (
                status,
                serde_json::from_slice(&bytes).expect("json response"),
            )
        }
        async fn configure(&self, allow: &[&Path], claude: bool, codex: bool) -> Value {
            let (status, value) = self.call(Method::PUT, "/ingest/sources", json!({"claude":claude,"codex":codex,"agy":false,"git":false,"allow":allow,"sinceDays":30})).await;
            assert_eq!(status, StatusCode::OK, "{value}");
            value
        }
        fn transcripts(&self, project: &Path) {
            let claude = self.home.join(".claude/projects/p");
            let codex = self.home.join(".codex/sessions/2026");
            std::fs::create_dir_all(&claude).expect("claude dir");
            std::fs::create_dir_all(&codex).expect("codex dir");
            let ts = chrono::Utc::now().to_rfc3339();
            let one = json!({"type":"user","cwd":project,"timestamp":ts,"message":{"content":"Explain Rust ownership"}});
            std::fs::write(claude.join("one.jsonl"), format!("{one}\n")).expect("claude fixture");
            let meta =
                json!({"type":"session_meta","timestamp":ts,"payload":{"id":"two","cwd":project}});
            let msg = json!({"type":"response_item","timestamp":ts,"payload":{"type":"message","role":"user","content":[{"type":"input_text","text":"Explain borrowing"}]}});
            std::fs::write(codex.join("two.jsonl"), format!("{meta}\n{msg}\n"))
                .expect("codex fixture");
        }
    }
    async fn cleanup_settled(a: &Api) {
        tokio::time::timeout(Duration::from_secs(2), async {
            while std::fs::read_dir(core(&a.state).root().join("exports"))
                .expect("exports")
                .next()
                .is_some()
            {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("archive cleanup");
    }

    async fn event(sub: &mut Subscription, predicate: impl Fn(&Event) -> bool) -> Event {
        tokio::time::timeout(Duration::from_secs(15), async {
            loop {
                let stored = sub.live.recv().await.expect("event");
                if predicate(&stored.event) {
                    return stored.event.clone();
                }
            }
        })
        .await
        .expect("event timeout")
    }

    #[tokio::test]
    async fn preview_delete_stale_and_orphan_chains() {
        let a = Api::new();
        for id in ["a", "b", "c"] {
            a.node(id);
        }
        a.chain("ab", "a", "b");
        a.chain("bc", "b", "c");
        a.chain("dangling", "b", "missing");
        let (status, preview) = a
            .call(
                Method::POST,
                "/graph/delete-preview",
                json!({"nodeIds":["a","a"]}),
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(preview["deleted"], 1);
        assert_eq!(preview["blastRadius"], json!(["ab", "dangling"]));
        a.node("new");
        let (status, _) = a
            .call(
                Method::DELETE,
                "/graph/nodes",
                json!({"nodeIds":["a"],"previewToken":preview["previewToken"]}),
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT);
        assert_eq!(a.ws.stores().graph.nodes().expect("nodes").len(), 4);
        let (_, preview) = a
            .call(
                Method::POST,
                "/graph/delete-preview",
                json!({"nodeIds":["a"]}),
            )
            .await;
        let (status, _) = a
            .call(
                Method::DELETE,
                "/graph/nodes",
                json!({"nodeIds":["b"],"previewToken":preview["previewToken"]}),
            )
            .await;
        assert_eq!(status, StatusCode::CONFLICT);
        let mut sub = a.state.hub.subscribe(Some(a.ws.id()), None);
        let (status, deleted) = a
            .call(
                Method::DELETE,
                "/graph/nodes",
                json!({"nodeIds":["a"],"previewToken":preview["previewToken"]}),
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{deleted}");
        assert_eq!(
            a.ws.stores()
                .graph
                .chains()
                .expect("chains")
                .iter()
                .map(|c| c.id.as_str())
                .collect::<Vec<_>>(),
            ["bc"]
        );
        assert!(!a
            .ws
            .stores()
            .graph
            .nodes()
            .expect("nodes")
            .iter()
            .any(|n| n.id == "a"));
        let ev = event(&mut sub, |e| matches!(e, Event::GraphChanged { .. })).await;
        assert!(
            matches!(ev, Event::GraphChanged{version} if version == deleted["version"].as_u64().expect("version"))
        );
        for ids in [json!([]), json!(["absent"])] {
            assert_eq!(
                a.call(
                    Method::POST,
                    "/graph/delete-preview",
                    json!({"nodeIds":ids})
                )
                .await
                .0,
                StatusCode::BAD_REQUEST
            );
        }
    }

    #[tokio::test]
    async fn relayout_versions_cache_etag_and_practice_stub() {
        let a = Api::new();
        a.node("a");
        a.node("b");
        a.chain("ab", "a", "b");
        let (status, first) = a.call(Method::POST, "/graph/relayout", json!({})).await;
        assert_eq!(status, StatusCode::OK);
        let first_read = a
            .raw(Method::GET, &a.path("/graph"), Body::empty(), None)
            .await;
        let etag = first_read.headers()[header::ETAG].clone();
        let (_, second) = a.call(Method::POST, "/graph/relayout", json!({})).await;
        assert_eq!(first["version"], second["version"]);
        assert!(second["layoutVersion"].as_u64() > first["layoutVersion"].as_u64());
        let req = a
            .request(Method::GET, &a.path("/graph"), Body::empty(), None)
            .header(header::IF_NONE_MATCH, etag)
            .body(Body::empty())
            .expect("get");
        let res = router(a.state.clone()).oneshot(req).await.expect("graph");
        assert_eq!(res.status(), StatusCode::OK);
        let graph: Value =
            serde_json::from_slice(&res.into_body().collect().await.expect("body").to_bytes())
                .expect("json");
        assert_eq!(graph["layoutVersion"], second["layoutVersion"]);
        assert_eq!(graph["positions"].as_object().expect("positions").len(), 2);
        assert_eq!(graph["layoutPending"], false);
        let cache = lp_service::graph::read_positions(
            &a.ws.root().join("graph"),
            second["version"].as_u64().expect("version"),
        )
        .expect("cache");
        assert_eq!(cache.positions.len(), 2);
        assert_eq!(
            a.call(Method::POST, "/graph/practice", json!({"nodeIds":["a"]}))
                .await
                .0,
            StatusCode::NOT_IMPLEMENTED
        );
        assert_eq!(
            a.call(
                Method::POST,
                "/graph/practice",
                json!({"nodeIds":["missing"]})
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
        core(&a.state).registry.archive(a.ws.id()).expect("archive");
        assert_eq!(
            a.call(Method::POST, "/graph/relayout", json!({})).await.0,
            StatusCode::CONFLICT
        );
    }

    #[tokio::test]
    async fn ingest_overlap_detection_empty_allow_and_concurrent_run() {
        let a = Api::new();
        let project = a.home.join("project");
        std::fs::create_dir_all(&project).expect("project");
        a.transcripts(&project);
        let other = core(&a.state)
            .registry
            .create(NewWorkspace {
                name: "Other".into(),
                sources: Some(lp_service::workspace::Sources {
                    allow: vec![project.join("nested")],
                    ..Default::default()
                }),
                ..Default::default()
            })
            .expect("other");
        let saved = a.configure(&[&project], true, true).await;
        assert_eq!(saved["warnings"][0]["workspaceId"], other.id());
        let (_, sources) = a.call(Method::GET, "/ingest/sources", json!({})).await;
        assert_eq!(
            sources["candidates"].as_array().expect("candidates").len(),
            2
        );
        assert!(sources["candidates"]
            .as_array()
            .expect("candidates")
            .iter()
            .all(|c| c["sessions"] == 1));
        let held = core(&a.state).ingest_runs.reserve(&a.ws).expect("reserve");
        assert_eq!(
            a.call(Method::POST, "/ingest/run", json!({})).await.0,
            StatusCode::CONFLICT
        );
        let cancel_path = format!("/ingest/runs/{}/cancel", held.id);
        assert_eq!(
            a.call(Method::POST, &cancel_path, json!({})).await.0,
            StatusCode::NO_CONTENT
        );
        assert!(held.cancel.is_cancelled());
        assert!(
            core(&a.state).ingest_runs.reserve(&a.ws).is_none(),
            "cancellation keeps reservation until scan exits"
        );
        drop(held);
        let empty = a.configure(&[], true, true).await;
        assert_eq!(empty["enabled"], false);
        assert_eq!(
            a.call(Method::POST, "/ingest/run", json!({})).await.0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            a.call(Method::POST, "/ingest/runs/missing/cancel", json!({}))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
    }

    #[tokio::test]
    async fn ingest_fixture_progress_persisted_history_and_source_toggles() {
        let a = Api::new();
        let project = a.home.join("project");
        std::fs::create_dir_all(&project).expect("project");
        a.transcripts(&project);
        a.configure(&[&project], true, false).await;
        // No decisions is a valid distilled session; all downstream steps have empty inputs.
        a.state.faux().faux_structured(json!({"decisions":[]}));
        let mut sub = a.state.hub.subscribe(Some(a.ws.id()), None);
        let (status, run) = a
            .call(Method::POST, "/ingest/run", json!({"sinceDays":30}))
            .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        event(
            &mut sub,
            |e| matches!(e, Event::IngestProgress{phase,..} if phase == "scan"),
        )
        .await;
        let done = event(&mut sub, |e| matches!(e, Event::IngestDone { .. })).await;
        assert!(
            matches!(done, Event::IngestDone{ref run_id, ref report} if run_id == run["runId"].as_str().expect("run id") && report.sessions == 1 && report.errors.is_empty()),
            "{done:?}"
        );
        assert!(a
            .ws
            .stores()
            .tracker
            .session("claude:one")
            .expect("claude session")
            .is_some());
        assert!(a
            .ws
            .stores()
            .tracker
            .session("codex:two")
            .expect("codex session")
            .is_none());
        let (_, history) = a.call(Method::GET, "/ingest/runs", json!({})).await;
        assert_eq!(history["items"][0]["id"], run["runId"]);
        assert!(history["items"][0]["finishedAt"].is_i64());
        assert!(a.ws.root().join("ingest/runs.jsonl").is_file());
        a.configure(&[&project], false, true).await;
        a.state.faux().faux_structured(json!({"decisions":[]}));
        let (status, _) = a.call(Method::POST, "/ingest/run", json!({})).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        event(&mut sub, |e| matches!(e, Event::IngestDone { .. })).await;
        assert!(a
            .ws
            .stores()
            .tracker
            .session("codex:two")
            .expect("codex session")
            .is_some());
        let (_, history) = a.call(Method::GET, "/ingest/runs", json!({})).await;
        assert_eq!(history["items"].as_array().expect("history").len(), 2);
        assert_eq!(
            a.call(Method::POST, "/ingest/run", json!({"sinceDays":0}))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn export_download_one_shot_and_import_equal_stores() {
        let a = Api::seeded();
        let mut sub = a.state.hub.subscribe(Some(a.ws.id()), None);
        let (status, job) = a.call(Method::POST, "/export", json!({})).await;
        assert_eq!(status, StatusCode::ACCEPTED);
        let ready = event(&mut sub, |e| matches!(e, Event::ExportReady { .. })).await;
        let Event::ExportReady { job_id, token } = ready else {
            panic!("export event")
        };
        assert_eq!(job_id, job["jobId"]);
        let res = a
            .raw(
                Method::GET,
                &format!("/downloads/{token}"),
                Body::empty(),
                None,
            )
            .await;
        assert_eq!(res.status(), StatusCode::OK);
        assert_eq!(res.headers()[header::CONTENT_TYPE], "application/zip");
        let zip = res
            .into_body()
            .collect()
            .await
            .expect("download")
            .to_bytes();
        assert_eq!(
            a.raw(
                Method::GET,
                &format!("/downloads/{token}"),
                Body::empty(),
                None
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        let (status, imported) = a.upload(&zip).await;
        assert_eq!(status, StatusCode::CREATED, "{imported}");
        let imported = core(&a.state)
            .registry
            .open(imported["workspaceId"].as_str().expect("workspace id"))
            .expect("imported");
        assert_ne!(a.ws.id(), imported.id());
        assert_eq!(
            a.ws.stores().graph.nodes().expect("original nodes"),
            imported.stores().graph.nodes().expect("imported nodes")
        );
        assert_eq!(
            a.ws.stores().graph.chains().expect("original chains"),
            imported.stores().graph.chains().expect("imported chains")
        );
        assert_eq!(imported.meta().name, format!("{} (2)", a.ws.meta().name));
        let before = a.ws.stores();
        let after = imported.stores();
        assert_eq!(
            before.graph.coverage().expect("coverage"),
            after.graph.coverage().expect("coverage")
        );
        assert_eq!(
            before.chats.list_chats().expect("chats"),
            after.chats.list_chats().expect("chats")
        );
        assert_eq!(
            before.tracker.cards().expect("cards"),
            after.tracker.cards().expect("cards")
        );
        assert_eq!(
            before.tracker.decisions().expect("decisions"),
            after.tracker.decisions().expect("decisions")
        );
        assert_eq!(
            before.tracker.reviews().expect("reviews"),
            after.tracker.reviews().expect("reviews")
        );
        cleanup_settled(&a).await;
        assert!(
            std::fs::read_dir(core(&a.state).root().join("exports"))
                .expect("exports")
                .next()
                .is_none(),
            "no upload or download residue"
        );
    }

    #[tokio::test]
    async fn bad_zip_and_oversize_imports_are_rejected_without_residue() {
        let a = Api::new();
        let (status, value) = a.upload(b"not a zip").await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{value}");
        let request = a
            .request(
                Method::POST,
                "/workspaces/import",
                Body::empty(),
                Some("multipart/form-data; boundary=b22"),
            )
            .header(
                header::CONTENT_LENGTH,
                (MAX_IMPORT_BYTES + (2 << 20)).to_string(),
            )
            .body(Body::empty())
            .expect("oversize request");
        let res = router(a.state.clone())
            .oneshot(request)
            .await
            .expect("oversize");
        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(
            core(&a.state)
                .registry
                .list(false)
                .expect("list")
                .workspaces
                .len(),
            1
        );
        cleanup_settled(&a).await;
        assert!(std::fs::read_dir(core(&a.state).root().join("exports"))
            .expect("exports")
            .next()
            .is_none());
        assert_eq!(
            a.raw(Method::GET, "/downloads/missing", Body::empty(), None)
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            a.raw(Method::POST, "/workspaces/import", Body::empty(), None)
                .await
                .status(),
            StatusCode::BAD_REQUEST
        );
    }

    #[tokio::test]
    async fn chunked_import_enforces_limit_without_content_length() {
        use axum::extract::FromRequest;
        let a = Api::new();
        let chunks = [
            b"--b22\r\nContent-Disposition: form-data; name=\"file\"; filename=\"w.zip\"\r\n\r\n"
                .to_vec(),
            vec![b'x'; 20],
            b"\r\n--b22--\r\n".to_vec(),
        ];
        let stream = futures::stream::iter(chunks.into_iter().map(Ok::<_, std::io::Error>));
        let request = Request::post("/workspaces/import")
            .header(header::CONTENT_TYPE, "multipart/form-data; boundary=b22")
            .body(Body::from_stream(stream))
            .expect("chunked request");
        let multipart = Multipart::from_request(request, &a.state).await;
        let response = import_with_limit(a.state.clone(), HeaderMap::new(), multipart, 16)
            .await
            .expect("bounded upload");
        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
        cleanup_settled(&a).await;
        assert!(std::fs::read_dir(core(&a.state).root().join("exports"))
            .expect("exports")
            .next()
            .is_none());
    }
}
