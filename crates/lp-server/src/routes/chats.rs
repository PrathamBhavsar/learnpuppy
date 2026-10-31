//! Chat reads and housekeeping (docs/web/03 §Learn): summaries, the `ClassroomView`, rename,
//! delete to the workspace-local trash. Turns, cancel, jobs and artifacts live elsewhere.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::get;
use axum::{Json, Router};
use lp_core::ports::{ChatStore, JobRecord};
use lp_service::workspace::WorkspaceHandle;

use super::common::{blocking, open_writable, open_ws, page};
use crate::app::AppState;
use crate::dto::chat::{
    ChatPageDto, ChatSummaryDto, ChatsQuery, ClassroomViewDto, RenameChat, TurnDto, TurnState,
};
use crate::error::{ApiError, ApiJson, ApiQuery, ErrorEnvelope};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces/{ws}/chats", get(list))
        .route(
            "/workspaces/{ws}/chats/{id}",
            get(view).patch(rename).delete(remove),
        )
}

const MAX_TITLE_CHARS: usize = 120;

/// Ids are file names; anything that could name another path is simply not found.
fn checked(id: &str) -> Result<(), ApiError> {
    let ok = !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(ApiError::not_found(format!("no chat {id}")))
    }
}

fn load(h: &WorkspaceHandle, id: &str) -> Result<lp_core::ports::ChatRecord, ApiError> {
    checked(id)?;
    h.stores().chats.load_chat(id).map_err(|e| match e {
        lp_core::ports::StoreError::NotFound(_) => ApiError::not_found(format!("no chat {id}")),
        other => other.into(),
    })
}

fn jobs_of(h: &WorkspaceHandle) -> Vec<JobRecord> {
    h.stores()
        .jobs
        .all_report()
        .map(|(jobs, _)| jobs)
        .unwrap_or_default()
}

/// Chats, most recently updated first.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/chats",
    tag = "chats",
    params(("ws" = String, Path, description = "Workspace id"), ChatsQuery),
    responses((status = 200, body = ChatPageDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn list(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiQuery(q): ApiQuery<ChatsQuery>,
) -> Result<Json<ChatPageDto>, ApiError> {
    let h = open_ws(&st, &ws).await?;
    blocking(move || {
        let all = h.stores().chats.list_chats()?;
        let total = all.len();
        let (rows, next_cursor) = page(all, q.limit, q.cursor.as_deref())?;
        let jobs = jobs_of(&h);
        let mut items = Vec::with_capacity(rows.len());
        for row in rows {
            // A chat that vanished or broke between the listing and the load is left out.
            if let Ok(rec) = h.stores().chats.load_chat(&row.id) {
                items.push(ChatSummaryDto::new(&rec, &jobs));
            }
        }
        Ok(Json(ChatPageDto {
            items,
            next_cursor,
            total,
        }))
    })
    .await
}

/// Everything the Classroom renders for one chat. A pending question never carries its answer.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/chats/{id}",
    tag = "chats",
    params(
        ("ws" = String, Path, description = "Workspace id"),
        ("id" = String, Path, description = "Chat id"),
    ),
    responses((status = 200, body = ClassroomViewDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn view(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<ClassroomViewDto>, ApiError> {
    let h = open_ws(&st, &ws).await?;
    blocking(move || {
        let rec = load(&h, &id)?;
        let turn = TurnDto {
            state: TurnState::Idle,
            turn_id: None,
            started_at: None,
        };
        Ok(Json(ClassroomViewDto::new(&rec, &jobs_of(&h), turn)))
    })
    .await
}

/// Renames a chat.
#[utoipa::path(
    patch,
    path = "/workspaces/{ws}/chats/{id}",
    tag = "chats",
    params(
        ("ws" = String, Path, description = "Workspace id"),
        ("id" = String, Path, description = "Chat id"),
    ),
    request_body = RenameChat,
    responses(
        (status = 200, body = ChatSummaryDto),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
        (status = 422, description = "Invalid title", body = ErrorEnvelope),
    )
)]
pub async fn rename(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(body): ApiJson<RenameChat>,
) -> Result<Json<ChatSummaryDto>, ApiError> {
    let title = body.title.trim().to_owned();
    if title.is_empty() {
        return Err(ApiError::invalid("title", "must not be empty"));
    }
    if title.chars().count() > MAX_TITLE_CHARS {
        return Err(ApiError::invalid(
            "title",
            format!("must be at most {MAX_TITLE_CHARS} characters"),
        ));
    }
    let h = open_writable(&st, &ws).await?;
    let now = st.workspaces.core().clock.now();
    blocking(move || {
        let mut rec = load(&h, &id)?;
        rec.title = title;
        rec.updated_at = now.max(rec.updated_at.saturating_add(1));
        h.stores().chats.save_chat(&rec)?;
        h.emit("chat.updated", serde_json::json!({ "chatId": id }));
        Ok(Json(ChatSummaryDto::new(&rec, &jobs_of(&h))))
    })
    .await
}

/// Moves a chat to the workspace's `chats/.trash`.
#[utoipa::path(
    delete,
    path = "/workspaces/{ws}/chats/{id}",
    tag = "chats",
    params(
        ("ws" = String, Path, description = "Workspace id"),
        ("id" = String, Path, description = "Chat id"),
    ),
    responses(
        (status = 204),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
    )
)]
pub async fn remove(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    let h: Arc<WorkspaceHandle> = open_writable(&st, &ws).await?;
    let core = super::common::core(&st);
    blocking(move || {
        load(&h, &id)?;
        let chats = h.root().join("chats");
        let trash = chats.join(".trash");
        std::fs::create_dir_all(&trash).map_err(anyhow::Error::from)?;
        std::fs::rename(
            chats.join(format!("{id}.json")),
            trash.join(format!("{id}.json")),
        )
        .map_err(anyhow::Error::from)?;
        core.registry.invalidate_stats(h.id());
        h.emit("chat.updated", serde_json::json!({ "chatId": id }));
        Ok(StatusCode::NO_CONTENT)
    })
    .await
}
