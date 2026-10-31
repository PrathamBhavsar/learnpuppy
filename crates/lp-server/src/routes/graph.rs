//! Knowledge graph reads (docs/web/03 §Graph): snapshot with cached layout, node detail,
//! full-text search and the chain table.

use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use lp_core::ports::GraphStore;
use lp_service::digest::CardLinks;
use lp_service::graph as svc;
use smol_str::SmolStr;

use super::common::{blocking, core, not_modified, open_ws};
use crate::app::AppState;
use crate::dto::graph::{
    ChainTableDto, ChainsQuery, GraphDto, GraphSearchDto, GraphSearchQuery, NodeBriefDto,
    NodeDetailDto,
};
use crate::error::{ApiError, ApiQuery, ErrorEnvelope};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces/{ws}/graph", get(snapshot))
        .route("/workspaces/{ws}/graph/nodes/{id}", get(node))
        .route("/workspaces/{ws}/graph/search", get(search))
        .route("/workspaces/{ws}/graph/chains", get(chains))
}

const SEARCH_LIMIT: usize = 50;
const MAX_SEARCH_LIMIT: usize = 200;

/// The whole graph, clusters and cached layout. `positions` is `null` and `layoutPending` true
/// while the layout is computed; `graph.changed` follows when it is ready.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/graph",
    tag = "graph",
    params(("ws" = String, Path, description = "Workspace id")),
    responses(
        (status = 200, body = GraphDto, headers(("ETag" = String, description = "Changes with the graph or its layout"))),
        (status = 304, description = "Unchanged since `If-None-Match`"),
        (status = 404, body = ErrorEnvelope),
    )
)]
pub async fn snapshot(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let core = core(&st);
    let handle = open_ws(&st, &ws).await?;
    let (c, h) = (core.clone(), handle.clone());
    let (loaded, positions) = blocking(move || {
        let dir = h.root().join("graph");
        let loaded = c.graphs.get(&dir, &*h.stores().graph)?;
        let positions = svc::read_positions(&dir, loaded.version);
        Ok((loaded, positions))
    })
    .await?;
    let pending = positions.is_none();
    if pending {
        core.start_layout(&handle, loaded.clone());
    }
    let etag = format!(
        "\"g{}-{}\"",
        loaded.version,
        if pending { "pending" } else { "ready" }
    );
    let mut res = if not_modified(&headers, &etag) {
        axum::http::StatusCode::NOT_MODIFIED.into_response()
    } else {
        let h = handle.clone();
        let l = loaded.clone();
        let body = blocking(move || {
            let sessions: Vec<SmolStr> = l
                .snap
                .birth_session_partition()
                .into_iter()
                .map(|c| c.chat_id)
                .collect();
            let labels = svc::cluster_labels(&*h.stores().chats, &sessions);
            Ok(GraphDto::new(
                &l.snap, l.version, positions, &labels, pending,
            ))
        })
        .await?;
        Json(body).into_response()
    };
    if let Ok(v) = axum::http::HeaderValue::from_str(&etag) {
        res.headers_mut().insert(axum::http::header::ETAG, v);
    }
    Ok(res)
}

/// One node with its provenance, chains, related nodes and linked cards.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/graph/nodes/{id}",
    tag = "graph",
    params(
        ("ws" = String, Path, description = "Workspace id"),
        ("id" = String, Path, description = "Node id"),
    ),
    responses((status = 200, body = NodeDetailDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn node(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<NodeDetailDto>, ApiError> {
    let core = core(&st);
    let handle = open_ws(&st, &ws).await?;
    blocking(move || {
        let s = handle.stores();
        let loaded = core.graphs.get(&handle.root().join("graph"), &*s.graph)?;
        let links = CardLinks::read(&handle.root().join("tracker"));
        let detail = svc::node_detail(&loaded.snap, &id, &*s.chats, &*s.tracker, &links)?
            .ok_or_else(|| ApiError::not_found(format!("no node {id}")))?;
        Ok(Json(NodeDetailDto::new(
            detail,
            &loaded.snap,
            core.clock.now(),
        )))
    })
    .await
}

/// Full-text search over propositions, best match first.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/graph/search",
    tag = "graph",
    params(("ws" = String, Path, description = "Workspace id"), GraphSearchQuery),
    responses(
        (status = 200, body = GraphSearchDto),
        (status = 404, body = ErrorEnvelope),
        (status = 422, description = "Empty query", body = ErrorEnvelope),
    )
)]
pub async fn search(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiQuery(q): ApiQuery<GraphSearchQuery>,
) -> Result<Json<GraphSearchDto>, ApiError> {
    let text = q.q.trim().to_owned();
    if text.is_empty() {
        return Err(ApiError::invalid("q", "must not be empty"));
    }
    let limit = q.limit.unwrap_or(SEARCH_LIMIT).clamp(1, MAX_SEARCH_LIMIT);
    let handle = open_ws(&st, &ws).await?;
    blocking(move || {
        let hits = handle.stores().graph.search_propositions(&text, limit)?;
        Ok(Json(GraphSearchDto {
            items: hits.iter().map(NodeBriefDto::from).collect(),
        }))
    })
    .await
}

/// Chains touching the selected nodes, grouped by the conclusion's birth session.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/graph/chains",
    tag = "graph",
    params(("ws" = String, Path, description = "Workspace id"), ChainsQuery),
    responses((status = 200, body = ChainTableDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn chains(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiQuery(q): ApiQuery<ChainsQuery>,
) -> Result<Json<ChainTableDto>, ApiError> {
    let selection: Vec<String> = q
        .selection
        .as_deref()
        .unwrap_or_default()
        .split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_owned)
        .collect();
    let core = core(&st);
    let handle = open_ws(&st, &ws).await?;
    blocking(move || {
        let s = handle.stores();
        let loaded = core.graphs.get(&handle.root().join("graph"), &*s.graph)?;
        let sessions: Vec<SmolStr> = loaded
            .snap
            .birth_session_partition()
            .into_iter()
            .map(|c| c.chat_id)
            .collect();
        let labels: HashMap<SmolStr, String> = svc::cluster_labels(&*s.chats, &sessions);
        Ok(Json(ChainTableDto::new(&loaded.snap, selection, &labels)))
    })
    .await
}
