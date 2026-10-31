//! Helpers shared by the route files: blocking work, workspace access, error mapping,
//! conditional GET and cursor paging.

use std::sync::Arc;

use axum::http::header::{ETAG, IF_NONE_MATCH};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use lp_core::ports::StoreError;
use lp_service::workspace::{WorkspaceError, WorkspaceHandle};
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::app::AppState;
use crate::directory::Core;
use crate::error::{ApiError, ErrorCode};

impl From<WorkspaceError> for ApiError {
    fn from(e: WorkspaceError) -> Self {
        let message = e.to_string();
        match e {
            WorkspaceError::NotFound(_) => ApiError::not_found(message),
            WorkspaceError::Validation { field, message } => ApiError::invalid(field, message),
            WorkspaceError::ConfirmMismatch => ApiError::invalid("confirmName", message),
            WorkspaceError::InvalidArchive(_) => ApiError::invalid("file", message),
            WorkspaceError::NameTaken(_) => ApiError::new(ErrorCode::Conflict, message)
                .with_details(serde_json::json!({
                    "fields": [{ "path": "name", "message": "already in use" }]
                })),
            WorkspaceError::Conflict(_) => ApiError::new(ErrorCode::Conflict, message),
            WorkspaceError::Archived(_) => ApiError::new(ErrorCode::WorkspaceArchived, message),
            WorkspaceError::StorageFull(_) => ApiError::new(ErrorCode::StorageFull, message),
            WorkspaceError::Damaged { .. } => ApiError::new(ErrorCode::Conflict, message)
                .with_details(serde_json::json!({ "damaged": true })),
            WorkspaceError::Internal(_) => ApiError::from(anyhow::anyhow!(message)),
        }
    }
}

impl From<StoreError> for ApiError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::NotFound(id) => ApiError::not_found(format!("not found: {id}")),
            StoreError::Backend(m) => ApiError::from(WorkspaceError::from(StoreError::Backend(m))),
        }
    }
}

/// Runs blocking store or filesystem work off the async runtime.
pub async fn blocking<T, F>(f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, ApiError> + Send + 'static,
{
    tokio::task::spawn_blocking(f).await?
}

pub fn core(st: &AppState) -> Arc<Core> {
    st.workspaces.core().clone()
}

/// The open workspace `ws`; unknown or damaged ids are `not_found` / `conflict`.
pub async fn open_ws(st: &AppState, ws: &str) -> Result<Arc<WorkspaceHandle>, ApiError> {
    let core = core(st);
    let ws = ws.to_owned();
    blocking(move || Ok(core.registry.open(&ws)?)).await
}

/// Like [`open_ws`] for mutations: an archived workspace is `409 workspace_archived`.
pub async fn open_writable(st: &AppState, ws: &str) -> Result<Arc<WorkspaceHandle>, ApiError> {
    let h = open_ws(st, ws).await?;
    h.ensure_writable()?;
    Ok(h)
}

/// A strong ETag from the bytes of `body`'s JSON.
pub fn body_etag<T: Serialize>(body: &T) -> Result<String, ApiError> {
    let bytes = serde_json::to_vec(body)
        .map_err(|e| ApiError::from(anyhow::Error::new(e).context("serializing response")))?;
    let digest = Sha256::digest(&bytes);
    Ok(format!("\"{}\"", hex::encode(&digest[..12])))
}

/// Whether the request's `If-None-Match` names `etag` (or `*`).
pub fn not_modified(headers: &HeaderMap, etag: &str) -> bool {
    let Some(v) = headers.get(IF_NONE_MATCH).and_then(|v| v.to_str().ok()) else {
        return false;
    };
    let bare = |t: &str| t.trim().trim_start_matches("W/").to_owned();
    let want = bare(etag);
    v.split(',').any(|t| {
        let t = bare(t);
        t == "*" || t == want
    })
}

/// `200` with the body and its `ETag`, or `304` when the client already has it.
pub fn conditional<T: Serialize>(
    headers: &HeaderMap,
    etag: String,
    body: &T,
) -> Result<Response, ApiError> {
    let value = HeaderValue::from_str(&etag)
        .map_err(|e| ApiError::from(anyhow::Error::new(e).context("etag")))?;
    let mut res = if not_modified(headers, &etag) {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        Json(body).into_response()
    };
    res.headers_mut().insert(ETAG, value);
    Ok(res)
}

/// `200` JSON with an `ETag` derived from the body.
pub fn conditional_body<T: Serialize>(headers: &HeaderMap, body: &T) -> Result<Response, ApiError> {
    conditional(headers, body_etag(body)?, body)
}

pub const DEFAULT_PAGE: usize = 50;
pub const MAX_PAGE: usize = 500;

/// A page of `items` from `cursor` (an offset the previous page returned) of at most `limit`.
pub fn page<T>(
    items: Vec<T>,
    limit: Option<usize>,
    cursor: Option<&str>,
) -> Result<(Vec<T>, Option<String>), ApiError> {
    let start = match cursor {
        None | Some("") => 0,
        Some(c) => c
            .parse::<usize>()
            .map_err(|_| ApiError::invalid("cursor", "not a cursor from a previous page"))?,
    };
    let limit = limit.unwrap_or(DEFAULT_PAGE).clamp(1, MAX_PAGE);
    let total = items.len();
    let out: Vec<T> = items.into_iter().skip(start).take(limit).collect();
    let next = (start + out.len() < total).then(|| (start + out.len()).to_string());
    Ok((out, next))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paging_walks_the_list_and_rejects_bad_cursors() {
        let all: Vec<u32> = (0..5).collect();
        let (a, next) = page(all.clone(), Some(2), None).unwrap();
        assert_eq!((a, next.as_deref()), (vec![0, 1], Some("2")));
        let (b, next) = page(all.clone(), Some(2), Some("2")).unwrap();
        assert_eq!((b, next.as_deref()), (vec![2, 3], Some("4")));
        let (c, next) = page(all.clone(), Some(2), Some("4")).unwrap();
        assert_eq!((c, next), (vec![4], None));
        let (d, next) = page(all.clone(), Some(100), Some("9")).unwrap();
        assert!(d.is_empty() && next.is_none());
        assert_eq!(page(all.clone(), Some(0), None).unwrap().0.len(), 1);
        assert!(page(all, None, Some("x")).is_err());
    }

    #[test]
    fn if_none_match_accepts_lists_weak_tags_and_star() {
        let mut h = HeaderMap::new();
        assert!(!not_modified(&h, "\"a\""));
        h.insert(IF_NONE_MATCH, HeaderValue::from_static("\"b\", W/\"a\""));
        assert!(not_modified(&h, "\"a\""));
        assert!(!not_modified(&h, "\"c\""));
        h.insert(IF_NONE_MATCH, HeaderValue::from_static("*"));
        assert!(not_modified(&h, "\"c\""));
    }

    #[test]
    fn workspace_errors_map_to_their_wire_codes() {
        let code = |e: WorkspaceError| ApiError::from(e).code;
        assert_eq!(
            code(WorkspaceError::NotFound("x".into())),
            ErrorCode::NotFound
        );
        assert_eq!(
            code(WorkspaceError::Archived("x".into())),
            ErrorCode::WorkspaceArchived
        );
        assert_eq!(
            code(WorkspaceError::NameTaken("x".into())),
            ErrorCode::Conflict
        );
        assert_eq!(code(WorkspaceError::ConfirmMismatch), ErrorCode::Validation);
        assert_eq!(
            code(WorkspaceError::StorageFull("x".into())),
            ErrorCode::StorageFull
        );
        assert_eq!(
            code(WorkspaceError::Internal("x".into())),
            ErrorCode::Internal
        );
        let e = ApiError::from(WorkspaceError::Validation {
            field: "name",
            message: "must not be empty".into(),
        });
        assert_eq!(e.details.unwrap()["fields"][0]["path"], "name");
    }
}
