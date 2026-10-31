//! `/api/v1` route files. Each file exposes `routes()`; add new files to [`api`] and their
//! handlers to `openapi.rs`.

pub mod system;

use axum::Router;

use crate::app::AppState;

/// Everything nested under `/api/v1`.
pub fn api() -> Router<AppState> {
    Router::new()
        .merge(system::routes())
        .merge(crate::sse::routes())
}
