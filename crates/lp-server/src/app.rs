//! Shared server state and the router: `/api/v1` (route files under `routes/`), the SPA
//! fallback, and the middleware stack (request id → compression → Host/token/CSRF guard).

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use lp_cli_driver::accounts::AccountPool;
use lp_cli_driver::engine::FauxEngine;
use lp_service::engine::CliPoolEngine;
use lp_service::events::{DeltaPump, EventHub};
use lp_service::settings::Settings;
use tokio_util::sync::CancellationToken;
use tower_http::compression::CompressionLayer;

use crate::assets::Assets;
use crate::directory::WorkspaceDirectory;
pub use crate::dto::system::EngineMode;
use crate::engine::RoleEngine;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// SSE comment heartbeat interval (docs/web/03 §SSE).
pub const HEARTBEAT: Duration = Duration::from_secs(15);

#[derive(Clone, Debug)]
pub struct ServerConfig {
    /// The bound port; `Host` and `Origin` must name it.
    pub port: u16,
    /// Launch token; `None` in `--dev` mode (no token check).
    pub token: Option<String>,
    pub engine: EngineMode,
    pub data_root: PathBuf,
    pub heartbeat: Duration,
}

impl ServerConfig {
    pub fn dev(&self) -> bool {
        self.token.is_none()
    }
}

/// Everything a handler can reach. Cheap to clone.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<ServerConfig>,
    pub hub: Arc<EventHub>,
    /// All `chat.*` events of a streaming chat go through the pump (delta coalescing).
    pub pump: Arc<DeltaPump>,
    pub workspaces: Arc<dyn WorkspaceDirectory>,
    /// Process-wide CLI account pool (shared by every workspace).
    pub pool: Arc<AccountPool>,
    pub assets: Arc<Assets>,
    pub device_id: Arc<str>,
    /// Cancelled on SIGINT/SIGTERM: streams end, turns stop.
    pub shutdown: CancellationToken,
    faux: Arc<FauxEngine>,
}

impl AppState {
    pub fn new(
        config: ServerConfig,
        hub: Arc<EventHub>,
        workspaces: Arc<dyn WorkspaceDirectory>,
        pool: Arc<AccountPool>,
        assets: Assets,
        device_id: String,
    ) -> Self {
        Self {
            config: Arc::new(config),
            pump: Arc::new(DeltaPump::new(hub.clone())),
            hub,
            workspaces,
            pool,
            assets: Arc::new(assets),
            device_id: device_id.into(),
            shutdown: CancellationToken::new(),
            faux: Arc::new(FauxEngine::new()),
        }
    }

    /// The engine serving `role`: the shared scripted engine in faux mode, else the CLI pool
    /// routed by the role's profile and pin.
    pub fn engine_for(&self, settings: &Settings, role: &str) -> RoleEngine {
        match self.config.engine {
            EngineMode::Faux => RoleEngine::Faux(self.faux.clone()),
            EngineMode::Real => RoleEngine::Real(CliPoolEngine::for_role(
                self.pool.clone(),
                settings,
                role,
                settings.pin(role),
            )),
        }
    }

    /// The scripted engine used in faux mode (tests queue responses on it).
    pub fn faux(&self) -> &Arc<FauxEngine> {
        &self.faux
    }
}

/// The whole HTTP surface.
pub fn router(state: AppState) -> Router {
    let api = crate::routes::api()
        .fallback(crate::error::api_not_found)
        .method_not_allowed_fallback(crate::error::method_not_allowed);
    Router::new()
        .nest("/api/v1", api)
        .fallback(crate::assets::serve)
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            crate::auth::guard,
        ))
        .layer(CompressionLayer::new())
        .layer(axum::middleware::from_fn(crate::error::request_id))
        .with_state(state)
}
