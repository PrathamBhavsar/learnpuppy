//! `LlmEngine` port, its request/response types, and the engines behind it.

mod faux;
mod http;
mod subprocess;

use std::time::Duration;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub use faux::{FauxEngine, FauxStreamControl};
pub use http::HttpApiEngine;
pub use subprocess::{CliKind, SubprocessEngine};

#[derive(Debug, Clone, PartialEq)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
}

#[derive(Debug, Clone, PartialEq)]
pub enum StreamDelta {
    Text(String),
    Thinking(String),
    ToolCall(ToolCall),
}

#[derive(Debug, Clone, Default)]
pub struct TurnRequest {
    pub prompt: String,
    pub model: Option<String>,
    /// Reasoning effort (`low`..`ultra`); each CLI gets it in its own flag, or not at all.
    pub effort: Option<String>,
    /// Tools the model may call; rendered into the prompt as a text/JSON protocol.
    pub tools: Vec<ToolDecl>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct ToolDecl {
    pub name: String,
    pub usage: String,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct TurnResponse {
    pub text: String,
    pub tool_calls: Vec<ToolCall>,
}

#[derive(Debug, Clone, Default)]
pub struct StructuredRequest {
    pub prompt: String,
    pub model: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructuredOutput<T> {
    pub value: T,
    pub raw: Value,
}

#[derive(Debug, thiserror::Error)]
pub enum DriverError {
    #[error("cancelled")]
    Cancelled,
    #[error("timed out after {0:?}")]
    Timeout(Duration),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("cli exited with {code:?}: {stderr}")]
    Exit { code: Option<i32>, stderr: String },
    #[error("bad cli output: {0}")]
    Parse(String),
    #[error("cli returned an empty turn (no text, no tool calls)")]
    EmptyResponse,
    #[error("cli reported failure: {0}")]
    Failed(String),
    #[error("faux engine has no scripted response left")]
    FauxExhausted,
}

#[async_trait]
pub trait LlmEngine: Send + Sync {
    /// Stream a conversational or tool-calling turn.
    async fn stream_turn(
        &self,
        req: TurnRequest,
        event_tx: mpsc::Sender<StreamDelta>,
        cancel: CancellationToken,
    ) -> Result<TurnResponse, DriverError>;

    /// Run a JSON-schema constrained completion.
    async fn complete_structured<T: DeserializeOwned + JsonSchema + Send>(
        &self,
        req: StructuredRequest,
        cancel: CancellationToken,
    ) -> Result<StructuredOutput<T>, DriverError>;
}
