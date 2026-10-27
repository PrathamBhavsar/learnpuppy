//! `run_submit_agent`: a bounded tool-using sub-agent that must finish by calling `submit`.

use async_trait::async_trait;
use lp_cli_driver::engine::{DriverError, LlmEngine, TurnRequest};
use lp_core::plan::heal_encoded_arguments;
use serde_json::Value;
use tokio::sync::{mpsc, Semaphore};
use tokio_util::sync::CancellationToken;

const MAX_TURNS: usize = 4;
const EVENT_CAP: usize = 64;

/// Material-gathering tools of one sub-agent; `None` means the tool name is not theirs.
#[async_trait]
pub trait Tools: Send + Sync {
    async fn call(&self, name: &str, args: &Value) -> Option<Result<String, String>>;
}

pub struct NoTools;

#[async_trait]
impl Tools for NoTools {
    async fn call(&self, _: &str, _: &Value) -> Option<Result<String, String>> {
        None
    }
}

#[derive(Debug, thiserror::Error)]
pub enum AgentError {
    #[error(transparent)]
    Driver(#[from] DriverError),
    #[error("no valid submit after {MAX_TURNS} turns: {0}")]
    NoSubmit(String),
}

/// What a sub-agent needs from its host: the engine, the shared model-call cap and cancellation.
pub struct Run<'a, E> {
    pub engine: &'a E,
    pub bg_llm: &'a Semaphore,
    pub cancel: &'a CancellationToken,
}

pub fn str_arg<'a>(args: &'a Value, key: &str) -> Result<&'a str, String> {
    args.get(key)
        .and_then(Value::as_str)
        .filter(|s| !s.trim().is_empty())
        .ok_or_else(|| format!("`{key}` must be a non-empty string"))
}

/// Up to `MAX_TURNS` model turns; a rejected `submit` or a failed tool is reported back in the prompt.
pub async fn run_submit_agent<E: LlmEngine, T>(
    run: &Run<'_, E>,
    task: &str,
    tools: &dyn Tools,
    check: impl Fn(Value) -> Result<T, String>,
) -> Result<T, AgentError> {
    let mut prompt =
        format!("{task}\n\nGather material with the other tools, then finish by calling submit.\n");
    let mut last = "no tool call".to_string();
    for _ in 0..MAX_TURNS {
        let resp = {
            // The permit bounds concurrent background model calls, never the tool calls.
            let _permit = tokio::select! {
                _ = run.cancel.cancelled() => return Err(DriverError::Cancelled.into()),
                p = run.bg_llm.acquire() => p.map_err(|_| DriverError::Cancelled)?,
            };
            let (tx, mut rx) = mpsc::channel(EVENT_CAP);
            let drain = tokio::spawn(async move { while rx.recv().await.is_some() {} });
            let req = TurnRequest {
                prompt: prompt.clone(),
                model: None,
                effort: None,
                tools: vec![lp_cli_driver::engine::ToolDecl {
                    name: "submit".into(),
                    usage: "submit({...}) the final result".into(),
                }],
            };
            let resp = run.engine.stream_turn(req, tx, run.cancel.clone()).await;
            let _ = drain.await;
            resp?
        };
        if resp.tool_calls.is_empty() {
            prompt.push_str("\n[error] no tool call; finish by calling submit.\n");
        }
        for c in resp.tool_calls {
            let args = heal_encoded_arguments(c.input);
            if c.name == "submit" {
                match check(args) {
                    Ok(v) => return Ok(v),
                    Err(e) => {
                        prompt.push_str(&format!(
                            "\n[submit rejected] {e}. Fix it and submit again.\n"
                        ));
                        last = e;
                    }
                }
                continue;
            }
            let out = tools
                .call(&c.name, &args)
                .await
                .unwrap_or_else(|| Err(format!("unknown tool `{}`", c.name)));
            match out {
                Ok(t) => prompt.push_str(&format!("\n[{} result]\n{t}\n", c.name)),
                Err(e) => prompt.push_str(&format!("\n[{} error] {e}\n", c.name)),
            }
        }
    }
    Err(AgentError::NoSubmit(last))
}
