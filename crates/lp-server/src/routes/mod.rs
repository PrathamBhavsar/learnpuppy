//! `/api/v1` route files. Each file exposes `routes()`; add new files to [`api`] and their
//! handlers to `openapi.rs`.

pub mod accounts;
pub mod chats;
pub mod common;
pub mod digest;
pub mod exports;
pub mod graph;
pub mod graph_write;
pub mod ingest;
pub mod review;
pub mod search;
pub mod system;
#[cfg(feature = "test-hooks")]
pub mod testhooks;
pub mod workspaces;

use axum::Router;

use crate::app::AppState;

/// Everything nested under `/api/v1`.
pub fn api() -> Router<AppState> {
    let router = Router::new()
        .merge(system::routes())
        .merge(workspaces::routes())
        .merge(graph::routes())
        .merge(graph_write::routes())
        .merge(ingest::routes())
        .merge(exports::routes())
        .merge(chats::routes())
        .merge(review::routes())
        .merge(digest::routes())
        .merge(accounts::routes())
        .merge(search::routes())
        .merge(crate::sse::routes());
    #[cfg(feature = "test-hooks")]
    let router = router.merge(testhooks::routes());
    router
}
