//! `GET /health` and `GET /bootstrap`.

use axum::extract::State;
use axum::routing::get;
use axum::{Json, Router};

use crate::app::{AppState, VERSION};
use crate::dto::system::{AccountSummaryDto, BannerDto, Bootstrap, Health, WorkspaceSummaryDto};
use crate::error::{ApiError, ErrorEnvelope};

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/health", get(health))
        .route("/bootstrap", get(bootstrap))
}

/// Liveness; needs no session (still Host-checked).
#[utoipa::path(
    get,
    path = "/health",
    tag = "system",
    responses(
        (status = 200, body = Health),
        (status = 421, description = "Wrong Host", body = ErrorEnvelope),
    )
)]
pub async fn health(State(st): State<AppState>) -> Json<Health> {
    Json(Health {
        ok: true,
        version: VERSION.to_owned(),
        engine: st.config.engine,
        data_dir: st.config.data_root.display().to_string(),
    })
}

/// One call at app start: version, device, workspaces, accounts, banners.
#[utoipa::path(
    get,
    path = "/bootstrap",
    tag = "system",
    responses(
        (status = 200, body = Bootstrap),
        (status = 401, description = "No session", body = ErrorEnvelope),
        (status = 500, body = ErrorEnvelope),
    )
)]
pub async fn bootstrap(State(st): State<AppState>) -> Result<Json<Bootstrap>, ApiError> {
    let dir = st.workspaces.clone();
    let (workspaces, last, banners) = tokio::task::spawn_blocking(move || {
        anyhow::Ok((dir.list()?, dir.last_workspace()?, dir.banners()?))
    })
    .await??;
    let now = lp_service::now_ms();
    let accounts = lp_service::accounts::list(&st.pool)
        .into_iter()
        .map(|a| AccountSummaryDto::from_account(a, now))
        .collect();
    Ok(Json(Bootstrap {
        version: VERSION.to_owned(),
        server_time: now,
        device_id: st.device_id.to_string(),
        engine: st.config.engine,
        dev: st.config.dev(),
        workspaces: workspaces
            .into_iter()
            .map(WorkspaceSummaryDto::from)
            .collect(),
        last_workspace_id: last,
        accounts,
        banners: banners.into_iter().map(BannerDto::from).collect(),
    }))
}
