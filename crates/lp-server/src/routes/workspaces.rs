//! Workspace lifecycle and trash (docs/web/02, 03 §Workspaces, §Bootstrap/global).

use std::path::Path as FsPath;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{delete, get, post};
use axum::{Json, Router};

use super::common::{blocking, core};
use crate::app::AppState;
use crate::dto::workspace::{
    CreateWorkspace, DeleteWorkspace, ListWorkspacesQuery, PatchWorkspace, TrashEntryDto,
    TrashListDto, WorkspaceDto, WorkspaceListDto,
};
use crate::error::{ApiError, ApiJson, ApiQuery, ErrorEnvelope};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces", get(list).post(create))
        .route("/workspaces/{ws}", get(get_one).patch(patch).delete(remove))
        .route("/workspaces/{ws}/archive", post(archive))
        .route("/workspaces/{ws}/restore", post(restore))
        .route("/workspaces/{ws}/repair", post(repair))
        .route("/trash", get(trash_list))
        .route("/trash/{id}/restore", post(trash_restore))
        .route("/trash/{id}", delete(trash_purge))
}

/// Workspaces with stats, most recently opened first; damaged directories are listed apart.
#[utoipa::path(
    get,
    path = "/workspaces",
    tag = "workspaces",
    params(ListWorkspacesQuery),
    responses((status = 200, body = WorkspaceListDto), (status = 500, body = ErrorEnvelope))
)]
pub async fn list(
    State(st): State<AppState>,
    ApiQuery(q): ApiQuery<ListWorkspacesQuery>,
) -> Result<Json<WorkspaceListDto>, ApiError> {
    let core = core(&st);
    blocking(move || {
        let l = core.registry.list(q.archived)?;
        Ok(Json(WorkspaceListDto {
            items: l
                .workspaces
                .into_iter()
                .map(|w| WorkspaceDto::new(w.meta, w.stats))
                .collect(),
            damaged: l.damaged.into_iter().map(Into::into).collect(),
        }))
    })
    .await
}

/// Creates a workspace.
#[utoipa::path(
    post,
    path = "/workspaces",
    tag = "workspaces",
    request_body = CreateWorkspace,
    responses(
        (status = 201, body = WorkspaceDto),
        (status = 409, description = "Name already in use", body = ErrorEnvelope),
        (status = 422, description = "Invalid field", body = ErrorEnvelope),
    )
)]
pub async fn create(
    State(st): State<AppState>,
    ApiJson(body): ApiJson<CreateWorkspace>,
) -> Result<(StatusCode, Json<WorkspaceDto>), ApiError> {
    let core = core(&st);
    blocking(move || {
        let h = core.registry.create(body.into())?;
        Ok((StatusCode::CREATED, Json(h.meta().into())))
    })
    .await
}

/// One workspace's metadata and stats.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}",
    tag = "workspaces",
    params(("ws" = String, Path, description = "Workspace id")),
    responses((status = 200, body = WorkspaceDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn get_one(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<WorkspaceDto>, ApiError> {
    let core = core(&st);
    blocking(move || {
        let meta = core.registry.get(&ws)?;
        let stats = core.registry.stats(&ws).ok();
        Ok(Json(WorkspaceDto::new(meta, stats)))
    })
    .await
}

/// Edits name, icon, hue, description, sources or overrides.
#[utoipa::path(
    patch,
    path = "/workspaces/{ws}",
    tag = "workspaces",
    params(("ws" = String, Path, description = "Workspace id")),
    request_body = PatchWorkspace,
    responses(
        (status = 200, body = WorkspaceDto),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Archived, or name in use", body = ErrorEnvelope),
        (status = 422, description = "Invalid field", body = ErrorEnvelope),
    )
)]
pub async fn patch(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(body): ApiJson<PatchWorkspace>,
) -> Result<Json<WorkspaceDto>, ApiError> {
    let patch = body
        .into_patch()
        .map_err(|(path, message)| ApiError::invalid(path, message))?;
    let core = core(&st);
    blocking(move || Ok(Json(core.registry.update(&ws, patch)?.into()))).await
}

/// Makes a workspace read-only and hides it from the default list.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/archive",
    tag = "workspaces",
    params(("ws" = String, Path, description = "Workspace id")),
    responses((status = 200, body = WorkspaceDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn archive(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<WorkspaceDto>, ApiError> {
    let core = core(&st);
    blocking(move || Ok(Json(core.registry.archive(&ws)?.into()))).await
}

/// Brings an archived workspace back.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/restore",
    tag = "workspaces",
    params(("ws" = String, Path, description = "Workspace id")),
    responses((status = 200, body = WorkspaceDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn restore(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<WorkspaceDto>, ApiError> {
    let core = core(&st);
    blocking(move || Ok(Json(core.registry.restore(&ws)?.into()))).await
}

/// Rewrites minimal metadata for a damaged workspace from its directory name.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/repair",
    tag = "workspaces",
    params(("ws" = String, Path, description = "Workspace id")),
    responses(
        (status = 200, body = WorkspaceDto),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Not damaged", body = ErrorEnvelope),
    )
)]
pub async fn repair(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<WorkspaceDto>, ApiError> {
    let core = core(&st);
    blocking(move || Ok(Json(core.registry.repair(&ws)?.into()))).await
}

/// Moves a workspace to the trash. The body must repeat its exact name.
#[utoipa::path(
    delete,
    path = "/workspaces/{ws}",
    tag = "workspaces",
    params(("ws" = String, Path, description = "Workspace id")),
    request_body = DeleteWorkspace,
    responses(
        (status = 200, body = TrashEntryDto),
        (status = 404, body = ErrorEnvelope),
        (status = 422, description = "Name does not match", body = ErrorEnvelope),
    )
)]
pub async fn remove(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(body): ApiJson<DeleteWorkspace>,
) -> Result<Json<TrashEntryDto>, ApiError> {
    let core = core(&st);
    blocking(move || {
        let entry = core.registry.delete(&ws, &body.confirm_name)?;
        let bytes = dir_bytes(&entry.path);
        Ok(Json(TrashEntryDto::new(entry, bytes)))
    })
    .await
}

/// Deleted workspaces waiting in the trash, newest first.
#[utoipa::path(
    get,
    path = "/trash",
    tag = "workspaces",
    responses((status = 200, body = TrashListDto))
)]
pub async fn trash_list(State(st): State<AppState>) -> Result<Json<TrashListDto>, ApiError> {
    let core = core(&st);
    blocking(move || {
        let mut items: Vec<TrashEntryDto> = core
            .registry
            .trash_list()?
            .into_iter()
            .map(|e| {
                let bytes = dir_bytes(&e.path);
                TrashEntryDto::new(e, bytes)
            })
            .collect();
        items.sort_by(|a, b| {
            b.deleted_at
                .cmp(&a.deleted_at)
                .then_with(|| a.id.cmp(&b.id))
        });
        Ok(Json(TrashListDto { items }))
    })
    .await
}

/// Restores a trashed workspace (renamed with a suffix if its name was taken).
#[utoipa::path(
    post,
    path = "/trash/{id}/restore",
    tag = "workspaces",
    params(("id" = String, Path, description = "Trash entry id")),
    responses((status = 200, body = WorkspaceDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn trash_restore(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<WorkspaceDto>, ApiError> {
    let core = core(&st);
    blocking(move || Ok(Json(core.registry.trash_restore(&id)?.into()))).await
}

/// Deletes a trashed workspace for good.
#[utoipa::path(
    delete,
    path = "/trash/{id}",
    tag = "workspaces",
    params(("id" = String, Path, description = "Trash entry id")),
    responses((status = 204), (status = 404, body = ErrorEnvelope))
)]
pub async fn trash_purge(
    State(st): State<AppState>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    let core = core(&st);
    blocking(move || {
        core.registry.trash_purge(&id)?;
        Ok(StatusCode::NO_CONTENT)
    })
    .await
}

/// Bytes under `dir`; unreadable entries count as zero.
fn dir_bytes(dir: &FsPath) -> u64 {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return 0;
    };
    rd.flatten()
        .map(|e| match e.file_type() {
            Ok(t) if t.is_dir() => dir_bytes(&e.path()),
            Ok(t) if t.is_file() => e.metadata().map_or(0, |m| m.len()),
            _ => 0,
        })
        .sum()
}
