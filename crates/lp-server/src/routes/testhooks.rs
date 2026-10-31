//! Test-only server hooks (docs/web/07 §Fixtures). Compiled only with the `test-hooks` feature,
//! never in release builds.

use axum::extract::State;
use axum::routing::post;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use super::common::core;
use crate::app::AppState;
use crate::error::{ApiError, ApiJson};

pub fn routes() -> Router<AppState> {
    Router::new().route("/__test/clock", post(set_clock))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockRequest {
    /// Pin the server clock to this epoch ms; `null` releases it.
    pub now: Option<i64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockReply {
    pub now: i64,
}

async fn set_clock(
    State(st): State<AppState>,
    ApiJson(body): ApiJson<ClockRequest>,
) -> Result<Json<ClockReply>, ApiError> {
    let core = core(&st);
    core.clock.pin(body.now);
    Ok(Json(ClockReply {
        now: core.clock.now(),
    }))
}
