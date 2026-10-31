//! `/api/v1` route files. Each file exposes `routes()`; add new files to [`api`] and their
//! handlers to `openapi.rs`.

pub mod accounts;
pub mod artifacts;
pub mod chats;
pub mod chats_write;
pub mod common;
pub mod digest;
pub mod graph;
pub mod jobs;
pub mod review;
pub mod search;
pub mod sources;
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
        .merge(chats::routes())
        .merge(chats_write::routes())
        .merge(jobs::routes())
        .merge(artifacts::routes())
        .merge(sources::routes())
        .merge(review::routes())
        .merge(digest::routes())
        .merge(accounts::routes())
        .merge(search::routes())
        .merge(crate::sse::routes());
    #[cfg(feature = "test-hooks")]
    let router = router.merge(testhooks::routes());
    router
}
