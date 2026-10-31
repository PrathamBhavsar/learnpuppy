//! FSRS review (docs/web/03 §Review). The server clock decides what is due; grading is
//! idempotent by `clientId` and commits card and log atomically.

use axum::extract::{Path, State};
use axum::http::{header::ETAG, HeaderMap, HeaderValue, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use lp_core::ports::{FlashCard, TrackerStore};
use lp_service::digest::CardLinks;
use lp_service::review as svc;

use super::common::{blocking, body_etag, core, not_modified, open_writable, open_ws, page};
use crate::app::AppState;
use crate::dto::review::{
    AnswerDto, AnswerRequest, CardBriefDto, CardDetailDto, CardPageDto, CardsQuery, GradeDto,
    GradeRequest, PreviewListDto, QueueCardDto, QueueDto, QueueQuery, StatsDto, SyncDto,
};
use crate::error::{ApiError, ApiJson, ApiQuery, ErrorEnvelope};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces/{ws}/review/queue", get(queue))
        .route("/workspaces/{ws}/review/stats", get(stats))
        .route("/workspaces/{ws}/review/cards", get(cards))
        .route("/workspaces/{ws}/review/cards/{card}", get(card_detail))
        .route("/workspaces/{ws}/review/preview/{card}", get(preview))
        .route("/workspaces/{ws}/review/sync", post(sync))
        .route("/workspaces/{ws}/review/{card}/answer", post(answer))
        .route("/workspaces/{ws}/review/{card}/grade", post(grade))
}

const DEFAULT_QUEUE: usize = 20;
const MAX_QUEUE: usize = 200;
const MAX_CLIENT_ID: usize = 128;

fn client_id(id: &str) -> Result<(), ApiError> {
    if id.is_empty() || id.len() > MAX_CLIENT_ID {
        return Err(ApiError::invalid(
            "clientId",
            format!("must be 1 to {MAX_CLIENT_ID} characters"),
        ));
    }
    Ok(())
}

fn find_card(t: &dyn TrackerStore, id: &str) -> Result<FlashCard, ApiError> {
    t.card(id)?
        .ok_or_else(|| ApiError::not_found(format!("no card {id}")))
}

/// Due cards at the server's `now`, oldest due first. Answers are withheld.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/review/queue",
    tag = "review",
    params(("ws" = String, Path, description = "Workspace id"), QueueQuery),
    responses((status = 200, body = QueueDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn queue(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiQuery(q): ApiQuery<QueueQuery>,
) -> Result<Json<QueueDto>, ApiError> {
    let core = core(&st);
    let h = open_ws(&st, &ws).await?;
    let limit = q.limit.unwrap_or(DEFAULT_QUEUE).clamp(1, MAX_QUEUE);
    blocking(move || {
        let now = core.clock.now();
        let (due, total) = svc::queue(&*h.stores().tracker, now, limit)?;
        let links = CardLinks::read(&h.root().join("tracker"));
        Ok(Json(QueueDto {
            items: due
                .iter()
                .map(|c| QueueCardDto::new(c, links.decision_of(&c.id)))
                .collect(),
            due_now: total,
            now,
        }))
    })
    .await
}

/// Checks a choice without changing the schedule: correctness, the rationale, the suggested
/// grade and what each grade would schedule.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/review/{card}/answer",
    tag = "review",
    params(
        ("ws" = String, Path, description = "Workspace id"),
        ("card" = String, Path, description = "Card id"),
    ),
    request_body = AnswerRequest,
    responses(
        (status = 200, body = AnswerDto),
        (status = 404, body = ErrorEnvelope),
        (status = 422, description = "Unknown choice or confidence", body = ErrorEnvelope),
    )
)]
pub async fn answer(
    State(st): State<AppState>,
    Path((ws, card)): Path<(String, String)>,
    ApiJson(req): ApiJson<AnswerRequest>,
) -> Result<Json<AnswerDto>, ApiError> {
    client_id(&req.client_id)?;
    let picked = svc::option_index(&req.choice_key)
        .ok_or_else(|| ApiError::invalid("choiceKey", "must be one of a, b, c, d"))?;
    if req.confidence.is_some_and(|c| !(1..=5).contains(&c)) {
        return Err(ApiError::invalid("confidence", "must be between 1 and 5"));
    }
    let core = core(&st);
    let h = open_ws(&st, &ws).await?;
    blocking(move || {
        let c = find_card(&*h.stores().tracker, &card)?;
        Ok(Json(AnswerDto::new(&c, picked, &req, core.clock.now())))
    })
    .await
}

/// Grades a card (1 again .. 4 easy). A retry with the same `clientId` returns the first result.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/review/{card}/grade",
    tag = "review",
    params(
        ("ws" = String, Path, description = "Workspace id"),
        ("card" = String, Path, description = "Card id"),
    ),
    request_body = GradeRequest,
    responses(
        (status = 200, body = GradeDto),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
        (status = 422, description = "Rating out of range", body = ErrorEnvelope),
    )
)]
pub async fn grade(
    State(st): State<AppState>,
    Path((ws, card)): Path<(String, String)>,
    ApiJson(req): ApiJson<GradeRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    client_id(&req.client_id)?;
    let rating = svc::rating_from_int(req.rating)
        .ok_or_else(|| ApiError::invalid("rating", "must be between 1 and 4"))?;
    let core = core(&st);
    let h = open_writable(&st, &ws).await?;
    let device = st.device_id.clone();
    let key = format!("grade:{ws}:{card}:{}", req.client_id);
    blocking(move || {
        let _serial = core.review_lock.lock();
        if let Some(first) = core.replies.get(&key) {
            return Ok(Json(first));
        }
        let t = &*h.stores().tracker;
        find_card(t, &card)?;
        let now = core.clock.now();
        svc::grade(t, &device, &card, rating, now)?;
        let updated = find_card(t, &card)?;
        let links = CardLinks::read(&h.root().join("tracker"));
        let dto = GradeDto {
            card: CardBriefDto::new(&updated, links.decision_of(&updated.id), now),
            preview: crate::dto::review::preview(&updated, now),
        };
        let value = serde_json::to_value(&dto).map_err(anyhow::Error::from)?;
        core.replies.put(key, value.clone());
        core.registry.invalidate_stats(h.id());
        h.emit("review.changed", serde_json::json!({}));
        Ok(Json(value))
    })
    .await
}

/// `[again, hard, good, easy]` intervals for a card.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/review/preview/{card}",
    tag = "review",
    params(
        ("ws" = String, Path, description = "Workspace id"),
        ("card" = String, Path, description = "Card id"),
    ),
    responses((status = 200, body = PreviewListDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn preview(
    State(st): State<AppState>,
    Path((ws, card)): Path<(String, String)>,
) -> Result<Json<PreviewListDto>, ApiError> {
    let core = core(&st);
    let h = open_ws(&st, &ws).await?;
    blocking(move || {
        let c = find_card(&*h.stores().tracker, &card)?;
        Ok(Json(PreviewListDto {
            items: crate::dto::review::preview(&c, core.clock.now()),
        }))
    })
    .await
}

/// Review header numbers: due counts, streak, retention and a 14-day forecast.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/review/stats",
    tag = "review",
    params(("ws" = String, Path, description = "Workspace id")),
    responses(
        (status = 200, body = StatsDto, headers(("ETag" = String, description = "Changes with the stats, not with the clock"))),
        (status = 304, description = "Unchanged since `If-None-Match`"),
        (status = 404, body = ErrorEnvelope),
    )
)]
pub async fn stats(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let core = core(&st);
    let h = open_ws(&st, &ws).await?;
    let dto = blocking(move || {
        let now = core.clock.now();
        Ok(StatsDto::new(svc::stats(&*h.stores().tracker, now)?, now))
    })
    .await?;
    let etag = body_etag(&StatsDto {
        now: 0,
        ..dto.clone()
    })?;
    let mut res = if not_modified(&headers, &etag) {
        StatusCode::NOT_MODIFIED.into_response()
    } else {
        Json(&dto).into_response()
    };
    if let Ok(v) = HeaderValue::from_str(&etag) {
        res.headers_mut().insert(ETAG, v);
    }
    Ok(res)
}

/// Browse every card, soonest due first.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/review/cards",
    tag = "review",
    params(("ws" = String, Path, description = "Workspace id"), CardsQuery),
    responses(
        (status = 200, body = CardPageDto),
        (status = 404, body = ErrorEnvelope),
        (status = 422, description = "Unknown state", body = ErrorEnvelope),
    )
)]
pub async fn cards(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiQuery(q): ApiQuery<CardsQuery>,
) -> Result<Json<CardPageDto>, ApiError> {
    let state = match q.state.as_deref().filter(|s| !s.is_empty()) {
        None => None,
        Some(s) => Some(svc::parse_state(s).ok_or_else(|| {
            ApiError::invalid("state", "must be new, learning, review or relearning")
        })?),
    };
    let core = core(&st);
    let h = open_ws(&st, &ws).await?;
    blocking(move || {
        let now = core.clock.now();
        let all = svc::browse(&*h.stores().tracker, state, q.q.as_deref())?;
        let total = all.len();
        let (rows, next_cursor) = page(all, q.limit, q.cursor.as_deref())?;
        let links = CardLinks::read(&h.root().join("tracker"));
        Ok(Json(CardPageDto {
            items: rows
                .iter()
                .map(|c| CardBriefDto::new(c, links.decision_of(&c.id), now))
                .collect(),
            next_cursor,
            total,
        }))
    })
    .await
}

/// One card with its answer, rationale and review history (the browser's drawer).
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/review/cards/{card}",
    tag = "review",
    params(
        ("ws" = String, Path, description = "Workspace id"),
        ("card" = String, Path, description = "Card id"),
    ),
    responses((status = 200, body = CardDetailDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn card_detail(
    State(st): State<AppState>,
    Path((ws, card)): Path<(String, String)>,
) -> Result<Json<CardDetailDto>, ApiError> {
    let core = core(&st);
    let h = open_ws(&st, &ws).await?;
    blocking(move || {
        let t = &*h.stores().tracker;
        let c = find_card(t, &card)?;
        let links = CardLinks::read(&h.root().join("tracker"));
        Ok(Json(CardDetailDto::new(
            &c,
            links.decision_of(&c.id),
            svc::history(t, &card)?,
            core.clock.now(),
        )))
    })
    .await
}

/// Merges other devices' review logs, then ships this device's cards.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/review/sync",
    tag = "review",
    params(("ws" = String, Path, description = "Workspace id")),
    responses(
        (status = 200, body = SyncDto),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
    )
)]
pub async fn sync(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<SyncDto>, ApiError> {
    let core = core(&st);
    let h = open_writable(&st, &ws).await?;
    let device = st.device_id.clone();
    blocking(move || {
        let _serial = core.review_lock.lock();
        let t = &h.stores().tracker;
        svc::sync(t, &device)?;
        svc::export_cards(&**t, &device)?;
        core.registry.invalidate_stats(h.id());
        h.emit("review.changed", serde_json::json!({}));
        Ok(Json(SyncDto {
            cards: t.cards()?.len(),
            reviews: t.reviews()?.len(),
        }))
    })
    .await
}
