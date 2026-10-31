//! `GET /search`: full-text search fanned out across live workspaces.

use std::time::Duration;

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};
use lp_core::ports::GraphStore;

use super::common::{blocking, core};
use crate::app::AppState;
use crate::dto::graph::NodeBriefDto;
use crate::dto::search::{SearchDto, SearchHitDto, SearchQuery};
use crate::error::{ApiError, ApiQuery, ErrorEnvelope};

pub fn routes() -> Router<AppState> {
    Router::new().route("/search", get(search))
}

/// How long one workspace may take before its hits are left out.
pub const WORKSPACE_DEADLINE: Duration = Duration::from_millis(300);
const DEFAULT_LIMIT: usize = 30;
const MAX_LIMIT: usize = 100;

/// Searches every live workspace's propositions. A workspace slower than 300 ms is skipped and
/// named in `timedOut`.
#[utoipa::path(
    get,
    path = "/search",
    tag = "system",
    params(SearchQuery),
    responses(
        (status = 200, body = SearchDto),
        (status = 422, description = "Empty query", body = ErrorEnvelope),
    )
)]
pub async fn search(
    State(st): State<AppState>,
    ApiQuery(q): ApiQuery<SearchQuery>,
) -> Result<Json<SearchDto>, ApiError> {
    let text = q.q.trim().to_owned();
    if text.is_empty() {
        return Err(ApiError::invalid("q", "must not be empty"));
    }
    let limit = q.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT);
    let core = core(&st);
    let c = core.clone();
    let live = blocking(move || Ok(c.registry.list(false)?)).await?;

    let mut tasks = Vec::new();
    for w in live.workspaces {
        let (core, text) = (core.clone(), text.clone());
        let (id, name) = (w.meta.id.clone(), w.meta.name.clone());
        let job = tokio::task::spawn_blocking(move || {
            let h = core.registry.open(&id)?;
            let hits = h.stores().graph.search_propositions(&text, limit)?;
            Ok::<_, ApiError>(hits)
        });
        tasks.push((w.meta.id, name, job));
    }

    let mut items = Vec::new();
    let mut timed_out = Vec::new();
    for (id, name, job) in tasks {
        match tokio::time::timeout(WORKSPACE_DEADLINE, job).await {
            Err(_) => timed_out.push(id),
            Ok(Err(e)) => tracing::warn!(workspace = id, "search task failed: {e}"),
            Ok(Ok(Err(e))) => tracing::warn!(workspace = id, "search failed: {}", e.message),
            Ok(Ok(Ok(hits))) => {
                for (rank, n) in hits.iter().enumerate() {
                    items.push(SearchHitDto {
                        workspace_id: id.clone(),
                        workspace_name: name.clone(),
                        node: NodeBriefDto::from(n),
                        score: (1.0 / (1.0 + rank as f64) * 1000.0).round() / 1000.0,
                    });
                }
            }
        }
    }
    items.sort_by(|a, b| {
        b.score
            .total_cmp(&a.score)
            .then_with(|| a.workspace_name.cmp(&b.workspace_name))
            .then_with(|| a.node.id.cmp(&b.node.id))
    });
    items.truncate(limit);
    Ok(Json(SearchDto { items, timed_out }))
}
