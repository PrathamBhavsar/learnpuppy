//! Inert PDF uploads, stored in the workspace and attached to a chat's reading job.

use axum::body::{to_bytes, Bytes};
use axum::extract::{Path, Request, State};
use axum::http::{header, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use axum::{Json, Router};

use super::chats_write::{checked_id, rejected, turn_ctx};
use super::common::{blocking, open_writable};
use crate::app::AppState;
use crate::dto::chat::{NoteUploadDto, NoteUploadForm, PdfQuery, PdfUploadDto, PdfUploadForm};
use crate::error::{ApiError, ApiQuery, ErrorCode, ErrorEnvelope};

pub const MAX_PDF_BYTES: usize = 50 * 1024 * 1024;
const MAX_FORM_BYTES: usize = MAX_PDF_BYTES + 64 * 1024;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces/{ws}/sources/pdf", post(pdf))
        .route("/workspaces/{ws}/sources/note", post(note))
}

fn bad(message: &str) -> ApiError {
    ApiError::invalid("file", message)
}

fn too_large() -> ApiError {
    ApiError::new(ErrorCode::Validation, "PDF exceeds the 50 MB limit")
        .with_details(serde_json::json!({"maxBytes": MAX_PDF_BYTES}))
}

fn boundary(content_type: &str) -> Result<String, ApiError> {
    let mut parts = content_type.split(';');
    if parts.next().map(str::trim) != Some("multipart/form-data") {
        return Err(bad("expected multipart/form-data"));
    }
    let b = parts
        .find_map(|p| {
            p.trim()
                .strip_prefix("boundary=")
                .map(|s| s.trim_matches('"'))
        })
        .filter(|b| !b.is_empty() && b.len() <= 70 && b.bytes().all(|c| c.is_ascii_graphic()))
        .ok_or_else(|| bad("missing or invalid multipart boundary"))?;
    Ok(b.to_owned())
}

fn position(data: &[u8], needle: &[u8]) -> Option<usize> {
    data.windows(needle.len()).position(|w| w == needle)
}

/// Parse only the two fields this endpoint accepts; delimiters must be complete MIME lines.
fn form(bytes: Bytes, boundary: &str, note: bool) -> Result<(Bytes, String), ApiError> {
    let delimiter = format!("\r\n--{boundary}");
    let first = format!("--{boundary}\r\n");
    if !bytes.starts_with(first.as_bytes()) {
        return Err(bad("malformed multipart body"));
    }
    let mut at = first.len();
    let mut file = None;
    let mut question = None;
    loop {
        let start = at;
        let end = position(&bytes[at..], b"\r\n\r\n")
            .map(|i| at + i)
            .filter(|end| end - start <= 8192)
            .ok_or_else(|| bad("invalid multipart headers"))?;
        let headers =
            std::str::from_utf8(&bytes[at..end]).map_err(|_| bad("invalid multipart headers"))?;
        let disposition = headers
            .split("\r\n")
            .find_map(|line| {
                let (key, value) = line.split_once(':')?;
                key.eq_ignore_ascii_case("content-disposition")
                    .then_some(value.trim())
            })
            .ok_or_else(|| bad("missing Content-Disposition"))?;
        let parameter = |name: &str| {
            disposition.split(';').skip(1).find_map(|p| {
                let (k, v) = p.trim().split_once('=')?;
                (k == name).then(|| v.trim().trim_matches('"'))
            })
        };
        at = end + 4;
        let mut search = at;
        let body_end = loop {
            let i = position(&bytes[search..], delimiter.as_bytes())
                .map(|i| search + i)
                .ok_or_else(|| bad("unterminated multipart field"))?;
            let tail = &bytes[i + delimiter.len()..];
            if tail.starts_with(b"\r\n") || tail.starts_with(b"--") {
                break i;
            }
            search = i + delimiter.len();
        };
        let data = bytes.slice(at..body_end);
        match parameter("name") {
            Some("file") if file.is_none() => {
                let name = parameter("filename").ok_or_else(|| bad("file needs a filename"))?;
                if note {
                    let name = name.to_ascii_lowercase();
                    if !name.ends_with(".md") && !name.ends_with(".txt") {
                        return Err(bad("notes must have a .md or .txt extension"));
                    }
                    if data.len() > MAX_NOTE_BYTES {
                        return Err(note_too_large());
                    }
                    let text =
                        std::str::from_utf8(&data).map_err(|_| bad("note must be UTF-8 text"))?;
                    if text.trim().is_empty() || text.contains('\0') {
                        return Err(bad("note must contain non-empty text without NUL bytes"));
                    }
                } else {
                    if !name.to_ascii_lowercase().ends_with(".pdf") {
                        return Err(bad("file must have a .pdf extension"));
                    }
                    if data.len() > MAX_PDF_BYTES {
                        return Err(too_large());
                    }
                    if !data.starts_with(b"%PDF-") {
                        return Err(bad("file does not have PDF magic"));
                    }
                }
                file = Some(data);
            }
            Some("question") if !note && question.is_none() && data.len() <= 20_000 => {
                question = Some(
                    std::str::from_utf8(&data)
                        .map_err(|_| bad("question must be UTF-8"))?
                        .to_owned(),
                );
            }
            _ => return Err(bad("unexpected, duplicate or oversized multipart field")),
        }
        at = body_end + delimiter.len();
        if bytes[at..].starts_with(b"--") {
            break;
        }
        at += 2;
    }
    Ok((
        file.ok_or_else(|| bad("missing file field"))?,
        question.unwrap_or_default(),
    ))
}

#[utoipa::path(
    post, path = "/workspaces/{ws}/sources/pdf", tag = "chats",
    params(("ws" = String, Path), PdfQuery),
    request_body(content = PdfUploadForm, content_type = "multipart/form-data"),
    responses((status = 202, body = PdfUploadDto), (status = 400, body = ErrorEnvelope),
              (status = 413, description = "PDF exceeds 50 MB", body = ErrorEnvelope),
              (status = 404, body = ErrorEnvelope), (status = 409, body = ErrorEnvelope))
)]
pub async fn pdf(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiQuery(q): ApiQuery<PdfQuery>,
    request: Request,
) -> Response {
    match upload(st, ws, q, request).await {
        Ok(reply) => reply.into_response(),
        Err(error) => {
            let large = error
                .details
                .as_ref()
                .is_some_and(|d| d.get("maxBytes").is_some());
            let mut response = error.into_response();
            if large {
                *response.status_mut() = StatusCode::PAYLOAD_TOO_LARGE;
            }
            response
        }
    }
}

async fn upload(
    st: AppState,
    ws: String,
    q: PdfQuery,
    request: Request,
) -> Result<(StatusCode, Json<PdfUploadDto>), ApiError> {
    checked_id("chat", &q.chat)?;
    let h = open_writable(&st, &ws).await?;
    let b = boundary(
        request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default(),
    )?;
    let bytes = to_bytes(request.into_body(), MAX_FORM_BYTES)
        .await
        .map_err(|_| too_large())?;
    let (file, question) = blocking(move || form(bytes, &b, false)).await?;
    let bytes = file.len() as u64;
    let artifact_id = format!("source_{}", uuid::Uuid::new_v4().simple());
    let path = h
        .root()
        .join("artifacts")
        .join(format!("{artifact_id}.pdf"));
    let output = path.clone();
    blocking(move || {
        if let Some(dir) = output.parent() {
            std::fs::create_dir_all(dir).map_err(anyhow::Error::from)?;
        }
        std::fs::write(&output, file).map_err(anyhow::Error::from)?;
        Ok(())
    })
    .await?;
    let ctx = turn_ctx(&st, h).await?;
    let job = st.chats.attach_pdf(&ctx, &q.chat, &path, &question).await;
    let job_id = match job {
        Ok(id) => id,
        Err(e) => {
            blocking(move || {
                let _ = std::fs::remove_file(path);
                Ok(())
            })
            .await?;
            return Err(rejected(e, None));
        }
    };
    Ok((
        StatusCode::ACCEPTED,
        Json(PdfUploadDto {
            job_id,
            artifact_id,
            chat_id: q.chat,
            bytes,
        }),
    ))
}

const MAX_NOTE_BYTES: usize = 1024 * 1024;

fn note_too_large() -> ApiError {
    ApiError::new(ErrorCode::Validation, "Note exceeds the 1 MB limit")
        .with_details(serde_json::json!({"maxBytes": MAX_NOTE_BYTES}))
}

#[utoipa::path(
    post, path = "/workspaces/{ws}/sources/note", tag = "chats",
    params(("ws" = String, Path), PdfQuery),
    request_body(content = NoteUploadForm, content_type = "multipart/form-data"),
    responses((status = 201, body = NoteUploadDto), (status = 400, body = ErrorEnvelope),
              (status = 413, description = "Note exceeds 1 MB", body = ErrorEnvelope),
              (status = 404, body = ErrorEnvelope), (status = 409, body = ErrorEnvelope))
)]
pub async fn note(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiQuery(q): ApiQuery<PdfQuery>,
    request: Request,
) -> Response {
    match upload_note(st, ws, q, request).await {
        Ok(reply) => reply.into_response(),
        Err(error) => {
            let large = error
                .details
                .as_ref()
                .is_some_and(|d| d.get("maxBytes").is_some());
            let mut response = error.into_response();
            if large {
                *response.status_mut() = StatusCode::PAYLOAD_TOO_LARGE;
            }
            response
        }
    }
}

async fn upload_note(
    st: AppState,
    ws: String,
    q: PdfQuery,
    request: Request,
) -> Result<(StatusCode, Json<NoteUploadDto>), ApiError> {
    checked_id("chat", &q.chat)?;
    let h = open_writable(&st, &ws).await?;
    let b = boundary(
        request
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or_default(),
    )?;
    let bytes = to_bytes(request.into_body(), MAX_NOTE_BYTES + 64 * 1024)
        .await
        .map_err(|_| note_too_large())?;
    let (file, _) = blocking(move || form(bytes, &b, true)).await?;
    let bytes = file.len() as u64;
    let text = std::str::from_utf8(&file)
        .map_err(|_| bad("note must be UTF-8 text"))?
        .to_owned();
    let artifact_id = format!("source_{}", uuid::Uuid::new_v4().simple());
    let path = h
        .root()
        .join("artifacts")
        .join(format!("{artifact_id}.txt"));
    let output = path.clone();
    let ctx = turn_ctx(&st, h).await?;
    blocking(move || {
        if let Some(dir) = output.parent() {
            std::fs::create_dir_all(dir).map_err(anyhow::Error::from)?;
        }
        std::fs::write(output, file).map_err(anyhow::Error::from)?;
        Ok(())
    })
    .await?;
    if let Err(e) = st
        .chats
        .attach_note(&ctx, &q.chat, &artifact_id, text)
        .await
    {
        blocking(move || {
            let _ = std::fs::remove_file(path);
            Ok(())
        })
        .await?;
        return Err(rejected(e, None));
    }
    Ok((
        StatusCode::CREATED,
        Json(NoteUploadDto {
            artifact_id,
            chat_id: q.chat,
            bytes,
        }),
    ))
}
