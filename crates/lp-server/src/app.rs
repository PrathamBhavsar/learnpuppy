//! Shared server state and the router: `/api/v1` (route files under `routes/`), the SPA
//! fallback, and the middleware stack (request id → compression → Host/token/CSRF guard).

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use axum::Router;
use lp_agent::workbench::{Pdf, PdftotextPdf};
use lp_cli_driver::accounts::AccountPool;
use lp_cli_driver::engine::FauxEngine;
use lp_service::chat::runtime::Engines;
use lp_service::chat::{ChatRuntime, RuntimeConfig};
use lp_service::course::CourseService;
use lp_service::engine::CliPoolEngine;
use lp_service::events::{DeltaPump, Event, EventHub, Scope};
use lp_service::settings::Settings;
use lp_service::textbook::TextbookService;
use lp_service::workspace::WorkspaceHandle;
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
    /// Chat turns, the CLI semaphore and workbench jobs (docs/web/03 §Turn state machine).
    pub chats: Arc<ChatRuntime<RoleEngine>>,
    /// One course service per workspace, shared by every request: it holds the session state and
    /// locks. Built on first access (which also runs its restart recovery for stuck courses).
    courses: Arc<parking_lot::Mutex<HashMap<String, CourseService<RoleEngine>>>>,
    /// One textbook service per workspace (U7), built on first use; it recovers once per workspace.
    textbooks: Arc<parking_lot::Mutex<HashMap<String, TextbookService<RoleEngine>>>>,
    textbook_recovered: Arc<parking_lot::Mutex<HashSet<String>>>,
    textbook_pdf: Arc<dyn Pdf>,
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
        let pump = Arc::new(DeltaPump::new(hub.clone()));
        let core = workspaces.core().clone();
        let chats = ChatRuntime::new(
            pump.clone(),
            pool.clone(),
            RuntimeConfig {
                clock: Arc::new(move || core.clock.now()),
                ..RuntimeConfig::default()
            },
        );
        Self {
            config: Arc::new(config),
            pump,
            hub,
            workspaces,
            pool,
            assets: Arc::new(assets),
            device_id: device_id.into(),
            shutdown: CancellationToken::new(),
            chats,
            courses: Arc::default(),
            textbooks: Arc::default(),
            textbook_recovered: Arc::default(),
            textbook_pdf: Arc::new(PdftotextPdf),
            faux: Arc::new(FauxEngine::new()),
        }
    }

    /// Replaces the chat runtime (tests swap in offline web/PDF adapters or a short queue limit).
    pub fn with_chat_config(mut self, config: RuntimeConfig) -> Self {
        self.chats = ChatRuntime::new(self.pump.clone(), self.pool.clone(), config);
        self
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

    /// Role engines for the services: every call reads the current settings, so a profile change
    /// applies to the next call.
    fn role_engines(&self) -> Engines<RoleEngine> {
        let (mode, faux, pool, data) = (
            self.config.engine,
            self.faux.clone(),
            self.pool.clone(),
            self.config.data_root.clone(),
        );
        Arc::new(move |role: &str| {
            let settings = Settings::load(&data).unwrap_or_else(|_| {
                let mut s = Settings::default();
                s.fill_course_roles();
                s
            });
            Arc::new(match mode {
                EngineMode::Faux => RoleEngine::Faux(faux.clone()),
                EngineMode::Real => RoleEngine::Real(CliPoolEngine::for_role(
                    pool.clone(),
                    &settings,
                    role,
                    settings.pin(role),
                )),
            })
        })
    }

    /// Tests swap in a fake PDF extractor for textbook imports.
    pub fn with_textbook_pdf(mut self, pdf: Arc<dyn Pdf>) -> Self {
        self.textbook_pdf = pdf;
        self
    }

    /// The shared textbook service of workspace `ws`, built on first use. Blocking: opens files.
    /// Job updates go out as `job.updated` events of the workspace.
    pub fn textbook_service(
        &self,
        ws: &str,
        stores: &lp_service::stores::Stores,
        root: &std::path::Path,
    ) -> TextbookService<RoleEngine> {
        let mut map = self.textbooks.lock();
        if let Some(s) = map.get(ws) {
            return s.clone();
        }
        let (hub, scope_ws) = (self.hub.clone(), ws.to_owned());
        let notify = Arc::new(move |rec: lp_core::ports::JobRecord| {
            hub.publish(
                Scope::Workspace(scope_ws.clone()),
                Event::JobUpdated {
                    job: lp_service::chat::job_info(&rec),
                },
            );
        });
        let s = TextbookService::new(
            root.to_path_buf(),
            Arc::new(stores.clone()),
            self.role_engines(),
            self.textbook_pdf.clone(),
            self.device_id.to_string(),
            notify,
        );
        map.insert(ws.to_owned(), s.clone());
        s
    }

    /// [`Self::textbook_service`] for an open workspace; the first call for a workspace also runs
    /// the service's restart recovery, so a stale `Running` import never shows in the job list.
    pub async fn textbook(
        &self,
        h: &Arc<WorkspaceHandle>,
    ) -> Result<TextbookService<RoleEngine>, crate::error::ApiError> {
        let (st, h2) = (self.clone(), h.clone());
        let svc = crate::routes::common::blocking(move || {
            Ok(st.textbook_service(h2.id(), h2.stores(), h2.root()))
        })
        .await?;
        if self.textbook_recovered.lock().insert(h.id().to_owned()) {
            if let Err(e) = svc.recover().await {
                tracing::warn!(workspace = h.id(), error = %e, "textbook recovery failed");
            }
        }
        Ok(svc)
    }

    /// The shared course service of workspace `ws`, built on first use. Blocking: opens files.
    pub fn course_service(
        &self,
        ws: &str,
        stores: &lp_service::stores::Stores,
        root: &std::path::Path,
    ) -> CourseService<RoleEngine> {
        let mut map = self.courses.lock();
        if let Some(s) = map.get(ws) {
            return s.clone();
        }
        let engines = self.role_engines();
        let s = CourseService::new(
            root.to_path_buf(),
            Arc::new(stores.clone()),
            engines,
            self.device_id.to_string(),
        );
        map.insert(ws.to_owned(), s.clone());
        s
    }

    /// The scripted engine used in faux mode (tests queue responses on it).
    pub fn faux(&self) -> &Arc<FauxEngine> {
        &self.faux
    }
}

/// The whole HTTP surface.
pub fn router(state: AppState) -> Router {
    crate::routes::chats_write::recover_on_boot(&state);
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
