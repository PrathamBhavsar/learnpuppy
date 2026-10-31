//! `GET …/artifacts/{id}`: a job's output or an uploaded source from the workspace's
//! `artifacts/` directory, with its real content type. HTML and SVG can carry script, so they are
//! served with `Content-Security-Policy: sandbox` (docs/web/09 #4) and never sniffed.

use axum::extract::{Path, State};
use axum::http::{header, HeaderValue};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::Router;

use super::chats_write::checked_id;
use super::common::{blocking, open_ws};
use crate::app::AppState;
use crate::error::{ApiError, ErrorEnvelope};

pub fn routes() -> Router<AppState> {
    Router::new().route("/workspaces/{ws}/artifacts/{id}", get(artifact))
}

/// Served extensions, their content type, and whether the response is sandboxed.
const TYPES: [(&str, &str, bool); 8] = [
    ("md", "text/markdown; charset=utf-8", false),
    ("txt", "text/plain; charset=utf-8", false),
    ("json", "application/json", false),
    ("html", "text/html; charset=utf-8", true),
    ("svg", "image/svg+xml", true),
    ("png", "image/png", false),
    ("jpg", "image/jpeg", false),
    ("pdf", "application/pdf", false),
];

/// The artifact file `id.<ext>` and its type entry.
fn find(
    dir: &std::path::Path,
    id: &str,
) -> Option<(
    std::path::PathBuf,
    &'static (&'static str, &'static str, bool),
)> {
    TYPES.iter().find_map(|t| {
        let p = dir.join(format!("{id}.{}", t.0));
        p.is_file().then_some((p, t))
    })
}

/// One artifact.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/artifacts/{id}",
    tag = "chats",
    params(
        ("ws" = String, Path, description = "Workspace id"),
        ("id" = String, Path, description = "Artifact id (`JobDto.artifactId`)"),
    ),
    responses(
        (status = 200, description = "The file; `text/html` and `image/svg+xml` carry `Content-Security-Policy: sandbox`", content_type = "application/octet-stream", body = Vec<u8>),
        (status = 404, body = ErrorEnvelope),
    )
)]
pub async fn artifact(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Response, ApiError> {
    checked_id("artifact", &id)?;
    let h = open_ws(&st, &ws).await?;
    let (bytes, (ext, mime, sandbox)) = blocking(move || {
        let dir = h.root().join("artifacts");
        let (path, t) =
            find(&dir, &id).ok_or_else(|| ApiError::not_found(format!("no artifact {id}")))?;
        let path = path.canonicalize().map_err(anyhow::Error::from)?;
        let dir = dir.canonicalize().map_err(anyhow::Error::from)?;
        if !path.starts_with(&dir) {
            return Err(ApiError::not_found("artifact outside workspace"));
        }
        let bytes = std::fs::read(&path).map_err(anyhow::Error::from)?;
        Ok((bytes, *t))
    })
    .await?;
    let mut res = bytes.into_response();
    let hd = res.headers_mut();
    hd.insert(header::CONTENT_TYPE, HeaderValue::from_static(mime));
    hd.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    hd.insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-cache"),
    );
    if sandbox {
        hd.insert(
            header::CONTENT_SECURITY_POLICY,
            HeaderValue::from_static("sandbox"),
        );
    }
    if ext == "pdf" {
        hd.insert(
            header::CONTENT_DISPOSITION,
            HeaderValue::from_static("inline"),
        );
    }
    Ok(res)
}
