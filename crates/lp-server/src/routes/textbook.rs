//! Textbook mode (U7, `brain/learning-method/textbook-spec.md` §7): a thin layer over
//! `lp_service::textbook::TextbookService`, one shared service per workspace
//! (`AppState::textbook`). Upload is `POST …/sources/textbook` in `sources.rs`; the import runs
//! as a job on the existing jobs route and `job.updated` events. Everything here except
//! `import` is a stored read or a code-graded answer: no route makes a model call.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use lp_core::textbook::valid_book_id;
use lp_service::textbook::{conflict, AnswerMeta, TextbookError, TextbookService};

use super::common::{open_writable, open_ws};
use crate::app::AppState;
use crate::dto::course::SelfGradeRequest;
use crate::dto::textbook::{
    ChapterDetailDto, CheckAnswerRequest, CheckFeedbackDto, ImportRequest, ImportStartedDto,
    ReadingCheckDto, TextbookDto, TextbookListDto, TextbookUsageDto,
};
use crate::engine::RoleEngine;
use crate::error::{ApiError, ApiJson, ErrorCode, ErrorEnvelope};

type Svc = TextbookService<RoleEngine>;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces/{ws}/textbooks", get(list))
        .route("/workspaces/{ws}/textbooks/{book}", get(book))
        .route(
            "/workspaces/{ws}/textbooks/{book}/chapters/{n}",
            get(chapter),
        )
        .route("/workspaces/{ws}/textbooks/{book}/import", post(import))
        .route(
            "/workspaces/{ws}/textbooks/{book}/chapters/{n}/check",
            post(check),
        )
        .route(
            "/workspaces/{ws}/textbooks/{book}/checks/{check}/answer",
            post(answer),
        )
        .route(
            "/workspaces/{ws}/textbooks/{book}/checks/{check}/self-grade",
            post(self_grade),
        )
        .route("/workspaces/{ws}/textbooks/{book}/usage", get(usage))
}

const MAX_ITEM_ID: usize = 64;

impl From<TextbookError> for ApiError {
    fn from(e: TextbookError) -> Self {
        let m = e.to_string();
        match e {
            TextbookError::NotFound(_) => ApiError::not_found(m),
            TextbookError::Invalid(_) => ApiError::invalid("", m),
            TextbookError::Conflict(code) => {
                let message = match code.as_str() {
                    conflict::IMPORT_RUNNING => "an import is already running for this book",
                    conflict::NOTHING_TO_IMPORT => "nothing is left to import",
                    conflict::RATE_LIMITED => {
                        "this book allows 8 chapter imports per hour; try again later"
                    }
                    conflict::RETRY_USED => "this chapter already used its one retry",
                    conflict::CHAPTER_NOT_READY => "this chapter is not imported yet",
                    conflict::ALREADY_ANSWERED => "this question was already answered",
                    _ => "conflict",
                };
                ApiError::new(ErrorCode::Conflict, message)
                    .with_details(serde_json::json!({ "reason": code }))
            }
            TextbookError::Unavailable(_) => ApiError::new(ErrorCode::EngineUnavailable, m),
            TextbookError::Store(_) => ApiError::from(anyhow::anyhow!(m)),
        }
    }
}

/// A book id is `tb_` plus 16 lowercase hex characters; anything else cannot name a book.
fn checked_book(book: &str) -> Result<(), ApiError> {
    if valid_book_id(book) {
        Ok(())
    } else {
        Err(ApiError::not_found(format!("no textbook {book}")))
    }
}

/// `rc_` plus 32 lowercase hex characters.
fn checked_check(check: &str) -> Result<(), ApiError> {
    let ok = check.strip_prefix("rc_").is_some_and(|h| {
        h.len() == 32
            && h.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    });
    if ok {
        Ok(())
    } else {
        Err(ApiError::not_found(format!("no check {check}")))
    }
}

fn chapter_no(n: &str) -> Result<u32, ApiError> {
    n.parse()
        .map_err(|_| ApiError::not_found(format!("no chapter {n}")))
}

fn checked_item(item: &str) -> Result<(), ApiError> {
    if item.is_empty() || item.len() > MAX_ITEM_ID {
        return Err(ApiError::invalid("item", "item id is 1 to 64 characters"));
    }
    Ok(())
}

async fn svc(
    st: &AppState,
    ws: &str,
    write: bool,
) -> Result<(Svc, std::sync::Arc<lp_service::workspace::WorkspaceHandle>), ApiError> {
    let h = if write {
        open_writable(st, ws).await?
    } else {
        open_ws(st, ws).await?
    };
    Ok((st.textbook(&h).await?, h))
}

/// Imported books, newest first.
#[utoipa::path(
    get, path = "/workspaces/{ws}/textbooks", tag = "textbooks",
    params(("ws" = String, Path, description = "Workspace id")),
    responses((status = 200, body = TextbookListDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn list(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<TextbookListDto>, ApiError> {
    let (svc, _) = svc(&st, &ws, false).await?;
    Ok(Json(TextbookListDto {
        items: svc.list().await?.into_iter().map(Into::into).collect(),
    }))
}

/// One book with its chapter map and per-chapter import state.
#[utoipa::path(
    get, path = "/workspaces/{ws}/textbooks/{book}", tag = "textbooks",
    params(("ws" = String, Path), ("book" = String, Path, description = "Book id, `tb_` + 16 hex")),
    responses((status = 200, body = TextbookDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn book(
    State(st): State<AppState>,
    Path((ws, book)): Path<(String, String)>,
) -> Result<Json<TextbookDto>, ApiError> {
    checked_book(&book)?;
    let (svc, _) = svc(&st, &ws, false).await?;
    Ok(Json(svc.book(&book).await?.into()))
}

/// One chapter with its learning objectives (never their quiz items).
#[utoipa::path(
    get, path = "/workspaces/{ws}/textbooks/{book}/chapters/{n}", tag = "textbooks",
    params(("ws" = String, Path), ("book" = String, Path), ("n" = u32, Path, description = "Chapter index")),
    responses((status = 200, body = ChapterDetailDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn chapter(
    State(st): State<AppState>,
    Path((ws, book, n)): Path<(String, String, String)>,
) -> Result<Json<ChapterDetailDto>, ApiError> {
    checked_book(&book)?;
    let n = chapter_no(&n)?;
    let (svc, _) = svc(&st, &ws, false).await?;
    Ok(Json(svc.chapter(&book, n).await?.into()))
}

/// Starts an import of the listed chapters (or every chapter still pending) as a background job.
/// This is how a chapter gets imported: the learner opens it. Each chapter gets one try plus one
/// retry, and a book allows 8 chapter imports in a rolling hour; what does not fit stays pending.
#[utoipa::path(
    post, path = "/workspaces/{ws}/textbooks/{book}/import", tag = "textbooks",
    params(("ws" = String, Path), ("book" = String, Path)),
    request_body = ImportRequest,
    responses((status = 202, body = ImportStartedDto), (status = 400, body = ErrorEnvelope),
              (status = 404, body = ErrorEnvelope),
              (status = 409, description = "`details.reason` is `import_running`, `nothing_to_import`, `import_rate_limited` or `chapter_retry_used`", body = ErrorEnvelope))
)]
pub async fn import(
    State(st): State<AppState>,
    Path((ws, book)): Path<(String, String)>,
    ApiJson(req): ApiJson<ImportRequest>,
) -> Result<(StatusCode, Json<ImportStartedDto>), ApiError> {
    checked_book(&book)?;
    if req.chapters.as_ref().is_some_and(|c| c.len() > 200) {
        return Err(ApiError::invalid("chapters", "at most 200 chapters"));
    }
    let (svc, h) = svc(&st, &ws, true).await?;
    let job_id = svc.import(&book, req.chapters, h.busy()).await?;
    Ok((StatusCode::ACCEPTED, Json(ImportStartedDto { job_id })))
}

/// "I read chapter N": a quiz of stored items, one per objective. No model call.
#[utoipa::path(
    post, path = "/workspaces/{ws}/textbooks/{book}/chapters/{n}/check", tag = "textbooks",
    params(("ws" = String, Path), ("book" = String, Path), ("n" = u32, Path, description = "Chapter index")),
    responses((status = 200, body = ReadingCheckDto), (status = 404, body = ErrorEnvelope),
              (status = 409, description = "`details.reason` is `chapter_not_ready`", body = ErrorEnvelope))
)]
pub async fn check(
    State(st): State<AppState>,
    Path((ws, book, n)): Path<(String, String, String)>,
) -> Result<Json<ReadingCheckDto>, ApiError> {
    checked_book(&book)?;
    let n = chapter_no(&n)?;
    let (svc, _) = svc(&st, &ws, true).await?;
    Ok(Json(svc.start_check(&book, n).await?.into()))
}

/// Grades one answer by code and feeds the item's FSRS card. A retry with the same `clientId`
/// returns the stored feedback.
#[utoipa::path(
    post, path = "/workspaces/{ws}/textbooks/{book}/checks/{check}/answer", tag = "textbooks",
    params(("ws" = String, Path), ("book" = String, Path), ("check" = String, Path, description = "`rc_` + 32 hex")),
    request_body = CheckAnswerRequest,
    responses((status = 200, body = CheckFeedbackDto), (status = 400, body = ErrorEnvelope),
              (status = 404, body = ErrorEnvelope),
              (status = 409, description = "`details.reason` is `already_answered`", body = ErrorEnvelope))
)]
pub async fn answer(
    State(st): State<AppState>,
    Path((ws, book, check)): Path<(String, String, String)>,
    ApiJson(req): ApiJson<CheckAnswerRequest>,
) -> Result<Json<CheckFeedbackDto>, ApiError> {
    checked_book(&book)?;
    checked_check(&check)?;
    checked_item(&req.item)?;
    if req
        .client_id
        .as_ref()
        .is_some_and(|c| c.is_empty() || c.len() > 128)
    {
        return Err(ApiError::invalid(
            "clientId",
            "clientId is 1 to 128 characters",
        ));
    }
    let (svc, _) = svc(&st, &ws, true).await?;
    let meta = AnswerMeta {
        confidence: req.confidence,
        ms: req.ms,
        client_id: req.client_id,
    };
    let fb = svc
        .answer(&book, &check, &req.item, &req.text, meta)
        .await?;
    Ok(Json(fb.into()))
}

/// The learner's own grade for a typed answer that was close or wrong.
#[utoipa::path(
    post, path = "/workspaces/{ws}/textbooks/{book}/checks/{check}/self-grade", tag = "textbooks",
    params(("ws" = String, Path), ("book" = String, Path), ("check" = String, Path)),
    request_body = SelfGradeRequest,
    responses((status = 204, description = "Graded"), (status = 400, body = ErrorEnvelope),
              (status = 404, body = ErrorEnvelope), (status = 409, body = ErrorEnvelope))
)]
pub async fn self_grade(
    State(st): State<AppState>,
    Path((ws, book, check)): Path<(String, String, String)>,
    ApiJson(req): ApiJson<SelfGradeRequest>,
) -> Result<StatusCode, ApiError> {
    checked_book(&book)?;
    checked_check(&check)?;
    checked_item(&req.item)?;
    let (svc, _) = svc(&st, &ws, true).await?;
    svc.self_grade(&book, &check, &req.item, &req.grade).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Cheap-model tokens and calls spent on this book, against the lifetime cap.
#[utoipa::path(
    get, path = "/workspaces/{ws}/textbooks/{book}/usage", tag = "textbooks",
    params(("ws" = String, Path), ("book" = String, Path)),
    responses((status = 200, body = TextbookUsageDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn usage(
    State(st): State<AppState>,
    Path((ws, book)): Path<(String, String)>,
) -> Result<Json<TextbookUsageDto>, ApiError> {
    checked_book(&book)?;
    let (svc, _) = svc(&st, &ws, false).await?;
    Ok(Json(svc.usage(&book).await?.into()))
}
