//! Graph mutation endpoints; all ledger and layout work runs on the blocking pool.
use super::common::{blocking, core, open_writable};
use crate::{
    app::AppState,
    dto::mutations::*,
    error::{ApiError, ApiJson, ErrorCode, ErrorEnvelope},
};
use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    routing::{delete, post},
    Json, Router,
};
use lp_service::{graph, graph_mut::MutationError};
use smol_str::SmolStr;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces/{ws}/graph/delete-preview", post(preview))
        .route("/workspaces/{ws}/graph/nodes", delete(remove))
        .route("/workspaces/{ws}/graph/relayout", post(relayout))
        .route("/workspaces/{ws}/graph/practice", post(practice))
}
fn mutation_error(e: MutationError) -> ApiError {
    match e {
        MutationError::Selection => ApiError::invalid("nodeIds", e.to_string()),
        MutationError::Stale => ApiError::new(ErrorCode::Conflict, e.to_string()),
        MutationError::Other(e) => ApiError::from(e),
    }
}
fn selection(ids: Vec<String>) -> Result<Vec<SmolStr>, ApiError> {
    if ids.is_empty() || ids.len() > 10_000 || ids.iter().any(|id| id.is_empty() || id.len() > 256)
    {
        return Err(ApiError::invalid(
            "nodeIds",
            "select 1–10000 valid node ids",
        ));
    }
    Ok(ids.into_iter().map(Into::into).collect())
}
fn result(
    h: &lp_service::workspace::WorkspaceHandle,
    version: u64,
) -> Result<GraphMutation, ApiError> {
    let positions = graph::read_positions(&h.root().join("graph"), version)
        .ok_or_else(|| ApiError::from(anyhow::anyhow!("layout cache was not written")))?;
    Ok(GraphMutation {
        version,
        layout_version: positions.layout_version.max(positions.version),
    })
}
#[utoipa::path(post, path="/workspaces/{ws}/graph/delete-preview", tag="graph", params(("ws"=String, Path)), request_body=NodeSelection,
    responses((status=200, body=DeletePreview),(status=400, body=ErrorEnvelope),(status=404, body=ErrorEnvelope)))]
pub async fn preview(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(body): ApiJson<NodeSelection>,
) -> Result<Json<DeletePreview>, ApiError> {
    let ids = selection(body.node_ids)?;
    let h = open_writable(&st, &ws).await?;
    let c = core(&st);
    blocking(move || {
        Ok(Json(
            c.graph_mutations
                .preview(&h, &ids)
                .map_err(mutation_error)?
                .into(),
        ))
    })
    .await
}
#[utoipa::path(delete, path="/workspaces/{ws}/graph/nodes", tag="graph", params(("ws"=String, Path)), request_body=DeleteNodes,
    responses((status=200, body=GraphMutation),(status=409, body=ErrorEnvelope),(status=400, body=ErrorEnvelope)))]
pub async fn remove(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(body): ApiJson<DeleteNodes>,
) -> Result<Json<GraphMutation>, ApiError> {
    let ids = selection(body.node_ids)?;
    let h = open_writable(&st, &ws).await?;
    let c = core(&st);
    blocking(move || {
        let loaded = c
            .graph_mutations
            .delete(&h, &ids, &body.preview_token)
            .map_err(mutation_error)?;
        c.graphs.forget(&h.root().join("graph"));
        c.registry.invalidate_stats(h.id());
        Ok(Json(result(&h, loaded.version)?))
    })
    .await
}
#[utoipa::path(post, path="/workspaces/{ws}/graph/relayout", tag="graph", params(("ws"=String, Path)),
    responses((status=200, body=GraphMutation),(status=409, body=ErrorEnvelope)))]
pub async fn relayout(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<GraphMutation>, ApiError> {
    let h = open_writable(&st, &ws).await?;
    let c = core(&st);
    blocking(move || {
        let l = c.graph_mutations.relayout(&h)?;
        Ok(Json(result(&h, l.version)?))
    })
    .await
}
#[utoipa::path(post, path="/workspaces/{ws}/graph/practice", tag="graph", params(("ws"=String, Path)), request_body=NodeSelection,
    responses((status=201, body=PracticeChat),(status=501, body=ErrorEnvelope),(status=400, body=ErrorEnvelope)))]
pub async fn practice(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(body): ApiJson<NodeSelection>,
) -> Result<Response, ApiError> {
    let ids = selection(body.node_ids.clone())?;
    let h = open_writable(&st, &ws).await?;
    let c = core(&st);
    let check = h.clone();
    let mutations = c.graph_mutations.clone();
    blocking(move || {
        let lock = mutations.workspace_lock(check.root());
        let _guard = lock.lock();
        let snap = graph::snapshot(&*check.stores().graph)?;
        if ids.iter().any(|id| !snap.nodes.contains_key(id)) {
            return Err(ApiError::invalid("nodeIds", "select existing nodes"));
        }
        Ok(())
    })
    .await?;
    match c.practice_seeder.seed(h, body.node_ids).await? {
        Some(chat_id) => Ok((StatusCode::CREATED, Json(PracticeChat { chat_id })).into_response()),
        None => {
            let mut res = ApiError::new(
                ErrorCode::EngineUnavailable,
                "practice chat creation is not installed",
            )
            .into_response();
            *res.status_mut() = StatusCode::NOT_IMPLEMENTED;
            Ok(res)
        }
    }
}
