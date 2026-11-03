//! Courses (docs/web/03 §Courses): a thin layer over `lp_service::course::CourseService`, one
//! shared service per workspace (`AppState::course_service`). Stored steps need no model, so
//! `next`, `seen`, `answer` and the grades work offline.
//!
//! Calls that run a model (`create`, `placement`, `ask`, `check`) are plain awaited POSTs, run in a
//! detached task so a browser disconnect cannot cancel them half way (state is stored either way).
//! Writing and refilling are polled through `status`; there is no SSE for them.

use std::future::Future;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use lp_service::course::{CourseError, CourseService, PlacementAnswer};

use super::common::{blocking, open_writable, open_ws};
use crate::app::AppState;
use crate::dto::course::{
    AskQuestionRequest, AskReplyDto, CheckDto, CheckRequest, CourseAnswerRequest, CourseListDto,
    CourseStatusDto, CreateCourseRequest, CreatedCourseDto, FeedbackDto, FlagRequest, GradedDto,
    GradedListDto, MissDto, MissListDto, PlacementListDto, SeenRequest, SelfGradeRequest, StepDto,
    SubmitPlacementRequest, UsageDto,
};
use crate::engine::RoleEngine;
use crate::error::{ApiError, ApiJson, ErrorCode, ErrorEnvelope};

type Svc = CourseService<RoleEngine>;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces/{ws}/courses", get(list).post(create))
        .route(
            "/workspaces/{ws}/courses/{id}/placement",
            get(pending_placement).post(placement),
        )
        .route("/workspaces/{ws}/courses/{id}/misses", get(misses))
        .route("/workspaces/{ws}/courses/{id}/status", get(status))
        .route("/workspaces/{ws}/courses/{id}/retry", post(retry))
        .route("/workspaces/{ws}/courses/{id}/next", post(next))
        .route("/workspaces/{ws}/courses/{id}/seen", post(seen))
        .route("/workspaces/{ws}/courses/{id}/answer", post(answer))
        .route("/workspaces/{ws}/courses/{id}/self-grade", post(self_grade))
        .route("/workspaces/{ws}/courses/{id}/flag", post(flag))
        .route("/workspaces/{ws}/courses/{id}/ask", post(ask))
        .route("/workspaces/{ws}/courses/{id}/check", post(check))
        .route("/workspaces/{ws}/courses/{id}/usage", get(usage))
}

const MAX_GOAL: usize = 500;
const MAX_QUESTION: usize = 2_000;
const MAX_ANSWER: usize = 5_000;
const MAX_REASON: usize = 2_000;
const MAX_ID: usize = 128;
const MAX_ANSWERS: usize = 50;

impl From<CourseError> for ApiError {
    fn from(e: CourseError) -> Self {
        let m = e.to_string();
        match e {
            CourseError::NotFound(_) => ApiError::not_found(m),
            CourseError::Invalid(_) => ApiError::invalid("", m),
            // The same code the UI already handles for "no AI could answer".
            CourseError::Unavailable(_) => ApiError::new(ErrorCode::EngineUnavailable, m),
            CourseError::Store(_) => ApiError::from(anyhow::anyhow!(m)),
        }
    }
}

/// A course id is `[a-z0-9_-]` up to 128 characters; anything else cannot name a course, so it is a
/// plain 404 (this is also what stops `..` from reaching the filesystem).
fn course_id(id: &str) -> Result<(), ApiError> {
    let ok = !id.is_empty()
        && id.len() <= MAX_ID
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(ApiError::not_found(format!("no course {id}")))
    }
}

fn text(field: &str, v: &str, min: usize, max: usize) -> Result<(), ApiError> {
    let n = v.trim().chars().count();
    if n < min || v.chars().count() > max {
        return Err(ApiError::invalid(
            field,
            format!("must be {min} to {max} characters"),
        ));
    }
    Ok(())
}

fn item_id(v: &str) -> Result<(), ApiError> {
    text("item", v, 1, MAX_ID)
}

async fn svc(st: &AppState, ws: &str, write: bool) -> Result<Svc, ApiError> {
    let h = if write {
        open_writable(st, ws).await?
    } else {
        open_ws(st, ws).await?
    };
    let (st, id) = (st.clone(), ws.to_owned());
    blocking(move || Ok(st.course_service(&id, h.stores(), h.root()))).await
}

/// Runs a model-backed call to completion even if the client goes away.
async fn detached<T, F>(f: F) -> Result<T, ApiError>
where
    T: Send + 'static,
    F: Future<Output = Result<T, CourseError>> + Send + 'static,
{
    Ok(tokio::spawn(f).await??)
}

/// Courses, newest first.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/courses",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    responses((status = 200, body = CourseListDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn list(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<CourseListDto>, ApiError> {
    let s = svc(&st, &ws, false).await?;
    let courses = s.list().await?.into_iter().map(Into::into).collect();
    Ok(Json(CourseListDto { courses }))
}

/// Starts a course: runs placement on a cheap model (can take many seconds). Nothing is stored when
/// no AI can answer (503 `engine_unavailable`).
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    request_body = CreateCourseRequest,
    responses(
        (status = 201, body = CreatedCourseDto),
        (status = 400, description = "Bad goal", body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
        (status = 503, description = "No AI available", body = ErrorEnvelope),
    )
)]
pub async fn create(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(req): ApiJson<CreateCourseRequest>,
) -> Result<(StatusCode, Json<CreatedCourseDto>), ApiError> {
    text("goal", &req.goal, 1, MAX_GOAL)?;
    let s = svc(&st, &ws, true).await?;
    let c = detached(async move { s.create(&req.goal).await }).await?;
    Ok((StatusCode::CREATED, Json(c.into())))
}

/// Grades placement with code and starts writing the course in the background; poll `status`.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/placement",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = SubmitPlacementRequest,
    responses(
        (status = 200, body = GradedListDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
    )
)]
pub async fn placement(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<SubmitPlacementRequest>,
) -> Result<Json<GradedListDto>, ApiError> {
    course_id(&id)?;
    if req.answers.len() > MAX_ANSWERS {
        return Err(ApiError::invalid("answers", "too many answers"));
    }
    for a in &req.answers {
        item_id(&a.item)?;
        text("text", &a.text, 0, MAX_ANSWER)?;
    }
    let s = svc(&st, &ws, true).await?;
    let answers = req
        .answers
        .into_iter()
        .map(|a| PlacementAnswer {
            item: a.item,
            text: a.text,
        })
        .collect();
    let graded = detached(async move { s.submit_placement(&id, answers).await }).await?;
    let graded = graded
        .into_iter()
        .map(|g| GradedDto {
            item: g.item,
            correct: g.correct,
            answer: g.answer,
            why: g.why,
        })
        .collect();
    Ok(Json(GradedListDto { graded }))
}

/// The placement questions still to answer (empty once submitted), so a reloaded page can carry on.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/courses/{id}/placement",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    responses((status = 200, body = PlacementListDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn pending_placement(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<PlacementListDto>, ApiError> {
    course_id(&id)?;
    let s = svc(&st, &ws, false).await?;
    let items = s.pending_placement(&id).await?;
    Ok(Json(PlacementListDto {
        items: items.into_iter().map(Into::into).collect(),
    }))
}

/// The questions the learner missed in placement, with answers: a mini lesson while the course is
/// written.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/courses/{id}/misses",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    responses((status = 200, body = MissListDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn misses(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<MissListDto>, ApiError> {
    course_id(&id)?;
    let s = svc(&st, &ws, false).await?;
    let misses = s
        .placement_misses(&id)
        .await?
        .into_iter()
        .map(|m| MissDto {
            q: m.q,
            answer: m.answer,
            why: m.why,
        })
        .collect();
    Ok(Json(MissListDto { misses }))
}

/// `writing`, `ready`, `failed` (with a reason) or `refilling`. Poll this while waiting.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/courses/{id}/status",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    responses((status = 200, body = CourseStatusDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn status(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<CourseStatusDto>, ApiError> {
    course_id(&id)?;
    let s = svc(&st, &ws, false).await?;
    Ok(Json(s.status(&id).await?.into()))
}

/// Starts writing a failed course again.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/retry",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    responses(
        (status = 204),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
    )
)]
pub async fn retry(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    course_id(&id)?;
    svc(&st, &ws, true).await?.retry(&id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// The next step of the session from stored content (no model call). A POST because it advances
/// the session.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/next",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    responses(
        (status = 200, body = StepDto),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
    )
)]
pub async fn next(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<StepDto>, ApiError> {
    course_id(&id)?;
    Ok(Json(svc(&st, &ws, true).await?.next(&id).await?.into()))
}

/// Marks a lesson explanation as read.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/seen",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = SeenRequest,
    responses(
        (status = 204),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
    )
)]
pub async fn seen(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<SeenRequest>,
) -> Result<StatusCode, ApiError> {
    course_id(&id)?;
    text("lesson", &req.lesson, 1, MAX_ID)?;
    svc(&st, &ws, true).await?.seen(&id, &req.lesson).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Grades an answer with code. Mcq: send the option text; recall: the typed answer.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/answer",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = CourseAnswerRequest,
    responses(
        (status = 200, body = FeedbackDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
    )
)]
pub async fn answer(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<CourseAnswerRequest>,
) -> Result<Json<FeedbackDto>, ApiError> {
    course_id(&id)?;
    item_id(&req.item)?;
    text("text", &req.text, 0, MAX_ANSWER)?;
    let s = svc(&st, &ws, true).await?;
    let f = s
        .answer(&id, &req.item, &req.text, req.confidence, req.ms)
        .await?;
    Ok(Json(f.into()))
}

/// The learner's own grade for a recall item (`again`, `hard`, `good`, `easy`).
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/self-grade",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = SelfGradeRequest,
    responses(
        (status = 204),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
    )
)]
pub async fn self_grade(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<SelfGradeRequest>,
) -> Result<StatusCode, ApiError> {
    course_id(&id)?;
    item_id(&req.item)?;
    text("grade", &req.grade, 1, 16)?;
    svc(&st, &ws, true)
        .await?
        .self_grade(&id, &req.item, &req.grade)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// "This looks wrong": flags an item.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/flag",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = FlagRequest,
    responses(
        (status = 204),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
    )
)]
pub async fn flag(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<FlagRequest>,
) -> Result<StatusCode, ApiError> {
    course_id(&id)?;
    item_id(&req.item)?;
    text("reason", &req.reason, 0, MAX_REASON)?;
    svc(&st, &ws, true)
        .await?
        .flag(&id, &req.item, &req.reason)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// A short answer to a learner question (cheap model). The question is kept even when no AI can
/// answer now (503 `engine_unavailable`).
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/ask",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = AskQuestionRequest,
    responses(
        (status = 200, body = AskReplyDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
        (status = 503, description = "No AI available", body = ErrorEnvelope),
    )
)]
pub async fn ask(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<AskQuestionRequest>,
) -> Result<Json<AskReplyDto>, ApiError> {
    course_id(&id)?;
    text("question", &req.question, 1, MAX_QUESTION)?;
    let s = svc(&st, &ws, true).await?;
    let reply = detached(async move { s.ask(&id, &req.question).await }).await?;
    Ok(Json(AskReplyDto { reply }))
}

/// "Check my answer": a cheap model judges a typed answer.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/check",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = CheckRequest,
    responses(
        (status = 200, body = CheckDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
        (status = 503, description = "No AI available", body = ErrorEnvelope),
    )
)]
pub async fn check(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<CheckRequest>,
) -> Result<Json<CheckDto>, ApiError> {
    course_id(&id)?;
    item_id(&req.item)?;
    text("text", &req.text, 0, MAX_ANSWER)?;
    let s = svc(&st, &ws, true).await?;
    let c = detached(async move { s.check(&id, &req.item, &req.text).await }).await?;
    Ok(Json(c.into()))
}

/// Tokens spent by tier and the strong-call budget.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/courses/{id}/usage",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    responses((status = 200, body = UsageDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn usage(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<UsageDto>, ApiError> {
    course_id(&id)?;
    let s = svc(&st, &ws, false).await?;
    // `usage` of an unknown course would read as all zeros; `status` answers 404 for it.
    s.status(&id).await?;
    Ok(Json(s.usage(&id).await?.into()))
}
