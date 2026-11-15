//! `--engine faux|real`: the engine a role's turns run on. `LlmEngine` is generic (not object
//! safe), so the choice is an enum that delegates.

use std::sync::Arc;

use async_trait::async_trait;
use lp_cli_driver::engine::{
    DriverError, FauxEngine, LlmEngine, StreamDelta, StructuredOutput, StructuredRequest,
    TurnRequest, TurnResponse,
};
use lp_service::engine::CliPoolEngine;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub enum RoleEngine {
    /// The process-wide scripted engine (`--engine faux`).
    Faux(Arc<FauxEngine>),
    /// The CLI account pool routed by the role's profile and pin.
    Real(CliPoolEngine),
}

#[async_trait]
impl LlmEngine for RoleEngine {
    async fn stream_turn(
        &self,
        req: TurnRequest,
        event_tx: mpsc::Sender<StreamDelta>,
        cancel: CancellationToken,
    ) -> Result<TurnResponse, DriverError> {
        match self {
            RoleEngine::Faux(e) => e.stream_turn(req, event_tx, cancel).await,
            RoleEngine::Real(e) => e.stream_turn(req, event_tx, cancel).await,
        }
    }

    async fn complete_structured<T: DeserializeOwned + JsonSchema + Send>(
        &self,
        req: StructuredRequest,
        cancel: CancellationToken,
    ) -> Result<StructuredOutput<T>, DriverError> {
        match self {
            RoleEngine::Faux(e) => e.complete_structured(req, cancel).await,
            RoleEngine::Real(e) => e.complete_structured(req, cancel).await,
        }
    }

    fn usage(&self) -> (u64, u64) {
        match self {
            RoleEngine::Faux(e) => e.usage(),
            RoleEngine::Real(e) => e.usage(),
        }
    }
}
