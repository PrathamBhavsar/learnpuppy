//! Decision digest (docs/web/03 §Digest): decisions by day and session, one decision with its
//! git commits, and card regeneration.

use axum::extract::{Path, State};
use axum::routing::{get, post};
use axum::{Json, Router};
use lp_core::ports::TrackerStore;
use lp_service::digest::{self as svc, CardLinks, Filter, RegenError};
use lp_service::settings::Settings;
use tokio_util::sync::CancellationToken;

use super::common::{blocking, core, open_writable, open_ws};
use crate::app::AppState;
use crate::dto::digest::{
    CommitDto, DecisionDetailDto, DecisionDto, DigestDto, DigestQuery, RegenerateDto,
    SessionBriefDto,
};
use crate::dto::review::CardBriefDto;
use crate::error::{ApiError, ApiQuery, ErrorCode, ErrorEnvelope};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces/{ws}/digest", get(digest))
        .route("/workspaces/{ws}/digest/decisions/{id}", get(decision))
        .route(
            "/workspaces/{ws}/digest/decisions/{id}/cards",
            post(regenerate),
        )
}

const SOURCES: [&str; 3] = ["claude", "codex", "agy"];

/// Epoch ms, or `YYYY-MM-DD` (UTC midnight).
fn parse_instant(field: &str, v: &str) -> Result<i64, ApiError> {
    let v = v.trim();
    if let Ok(ms) = v.parse::<i64>() {
        return Ok(ms);
    }
    chrono::NaiveDate::parse_from_str(v, "%Y-%m-%d")
        .ok()
        .and_then(|d| d.and_hms_opt(0, 0, 0))
        .map(|dt| dt.and_utc().timestamp_millis())
        .ok_or_else(|| ApiError::invalid(field, "must be epoch milliseconds or YYYY-MM-DD"))
}

fn filter(q: &DigestQuery) -> Result<Filter, ApiError> {
    let mut sources = Vec::new();
    for s in q.source.as_deref().unwrap_or_default().split(',') {
        let s = s.trim();
        if s.is_empty() {
            continue;
        }
        if !SOURCES.contains(&s) {
            return Err(ApiError::invalid("source", format!("unknown source `{s}`")));
        }
        sources.push(s.to_owned());
    }
    let from = q
        .from
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(|v| parse_instant("from", v))
        .transpose()?;
    let to =
        q.to.as_deref()
            .filter(|s| !s.is_empty())
            .map(|v| parse_instant("to", v))
            .transpose()?;
    Ok(Filter { from, to, sources })
}

/// Decisions grouped by day, then coding session, newest first.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/digest",
    tag = "digest",
    params(("ws" = String, Path, description = "Workspace id"), DigestQuery),
    responses(
        (status = 200, body = DigestDto),
        (status = 404, body = ErrorEnvelope),
        (status = 422, description = "Bad date or source", body = ErrorEnvelope),
    )
)]
pub async fn digest(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiQuery(q): ApiQuery<DigestQuery>,
) -> Result<Json<DigestDto>, ApiError> {
    let f = filter(&q)?;
    let h = open_ws(&st, &ws).await?;
    blocking(move || {
        let s = h.stores();
        let days = svc::grouped(&*s.graph, &*s.tracker, &f)?;
        let links = CardLinks::read(&h.root().join("tracker"));
        Ok(Json(DigestDto::new(days, &links)))
    })
    .await
}

/// One decision with its session, linked cards and the git commits made while it ran.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/digest/decisions/{id}",
    tag = "digest",
    params(
        ("ws" = String, Path, description = "Workspace id"),
        ("id" = String, Path, description = "Decision id"),
    ),
    responses((status = 200, body = DecisionDetailDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn decision(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<DecisionDetailDto>, ApiError> {
    let core = core(&st);
    let h = open_ws(&st, &ws).await?;
    let (h2, id2) = (h.clone(), id.clone());
    let (row, session, cards, links) = blocking(move || {
        let s = h2.stores();
        let (row, session) = svc::decision(&*s.graph, &*s.tracker, &id2)?;
        let links = CardLinks::read(&h2.root().join("tracker"));
        let now = core.clock.now();
        let mut cards = Vec::new();
        for cid in links.cards_of(&row.record.id) {
            if let Some(c) = s.tracker.card(cid)? {
                cards.push(CardBriefDto::new(&c, Some(&row.record.id), now));
            }
        }
        Ok((row, session, cards, links))
    })
    .await?;
    let (commits, commits_error) = match &session {
        Some(sess) => {
            let allow = h.meta().sources.allow;
            match lp_ingest::commits_for_session(sess, sess.started_at, &allow).await {
                Ok(c) => (c.into_iter().map(CommitDto::from).collect(), None),
                Err(e) => (Vec::new(), Some(e)),
            }
        }
        None => (Vec::new(), None),
    };
    Ok(Json(DecisionDetailDto {
        decision: DecisionDto::new(row, &links),
        session: session.as_ref().map(SessionBriefDto::from),
        commits,
        commits_error,
        cards,
    }))
}

/// Generates 4-choice cards for a decision. Drafts that fail the card-shape check come back in
/// `rejected`.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/digest/decisions/{id}/cards",
    tag = "digest",
    params(
        ("ws" = String, Path, description = "Workspace id"),
        ("id" = String, Path, description = "Decision id"),
    ),
    responses(
        (status = 200, body = RegenerateDto),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
        (status = 503, description = "Engine unavailable", body = ErrorEnvelope),
    )
)]
pub async fn regenerate(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<RegenerateDto>, ApiError> {
    let core = core(&st);
    let h = open_writable(&st, &ws).await?;
    let root = core.root().to_owned();
    let settings = blocking(move || Ok(Settings::load(&root)?)).await?;
    let engine = st.engine_for(&settings, "distiller");
    let now = core.clock.now();
    // The generator calls blocking store methods; keep them off the async workers.
    let done = {
        let (h, id) = (h.clone(), id.clone());
        let cancel: CancellationToken = st.shutdown.child_token();
        let rt = tokio::runtime::Handle::current();
        tokio::task::spawn_blocking(move || {
            rt.block_on(svc::regenerate_cards(
                &engine,
                &*h.stores().tracker,
                &h.root().join("tracker"),
                &id,
                now,
                cancel,
            ))
        })
        .await?
    };
    match done {
        Ok(r) => {
            core.registry.invalidate_stats(h.id());
            h.emit("review.changed", serde_json::json!({}));
            Ok(Json(r.into()))
        }
        Err(RegenError::NotFound(id)) => Err(ApiError::not_found(format!("no decision {id}"))),
        Err(RegenError::Engine(m)) => Err(ApiError::new(ErrorCode::EngineUnavailable, m)),
        Err(RegenError::Store(m)) => Err(ApiError::from(anyhow::anyhow!(m))),
    }
}
