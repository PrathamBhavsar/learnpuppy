//! Workbench jobs (docs/web/03 §Learn): the workspace's research, PDF, visual and demo jobs, and
//! abort. Live progress arrives as `job.updated` events.

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};

use super::chats_write::{checked_id, rejected, turn_ctx};
use super::common::{open_writable, open_ws};
use crate::app::AppState;
use crate::dto::chat::{JobItemDto, JobListDto};
use crate::error::{ApiError, ErrorEnvelope};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces/{ws}/jobs", get(list))
        .route("/workspaces/{ws}/jobs/{id}/abort", post(abort))
}

/// Every job of the workspace, newest first. Jobs a previous server left running read as
/// finished with an "interrupted by a restart" error.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/jobs",
    tag = "chats",
    params(("ws" = String, Path, description = "Workspace id")),
    responses((status = 200, body = JobListDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn list(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<JobListDto>, ApiError> {
    let h = open_ws(&st, &ws).await?;
    let ctx = turn_ctx(&st, h).await?;
    let jobs = st.chats.jobs(&ctx).await.map_err(|e| rejected(e, None))?;
    Ok(Json(JobListDto {
        items: jobs.into_iter().map(JobItemDto::from).collect(),
    }))
}

/// Stops a running job; its result is never delivered to the chat.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/jobs/{id}/abort",
    tag = "chats",
    params(
        ("ws" = String, Path, description = "Workspace id"),
        ("id" = String, Path, description = "Job id"),
    ),
    responses(
        (status = 200, description = "Abort requested; `job.updated` follows", body = JobItemDto),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "The job is not running", body = ErrorEnvelope),
    )
)]
pub async fn abort(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<JobItemDto>, ApiError> {
    checked_id("job", &id)?;
    let h = open_writable(&st, &ws).await?;
    let ctx = turn_ctx(&st, h).await?;
    let job = st
        .chats
        .abort_job(&ctx, &id)
        .await
        .map_err(|e| rejected(e, None))?;
    Ok(Json(job.into()))
}
