use std::collections::VecDeque;
use std::sync::Arc;

use async_trait::async_trait;
use parking_lot::Mutex;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::sync::{mpsc, Semaphore};
use tokio_util::sync::CancellationToken;

use super::{
    DriverError, LlmEngine, StreamDelta, StructuredOutput, StructuredRequest, ToolCall,
    TurnRequest, TurnResponse,
};

struct Turn {
    deltas: Vec<StreamDelta>,
    gate: Option<Arc<Semaphore>>,
}

/// Releases a gated faux turn: the turn emits its first delta, then waits here.
pub struct FauxStreamControl(Arc<Semaphore>);

impl FauxStreamControl {
    pub fn release(&self) {
        self.0.add_permits(1);
    }
}

/// Scripted in-memory engine: each call pops the next queued response.
#[derive(Default)]
pub struct FauxEngine {
    turns: Mutex<VecDeque<Turn>>,
    structured: Mutex<VecDeque<Value>>,
}

impl FauxEngine {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn faux_turn(&self, deltas: Vec<StreamDelta>) {
        self.turns.lock().push_back(Turn { deltas, gate: None });
    }

    /// Like `faux_turn`, but the turn stalls after its first delta until released.
    pub fn faux_gated_turn(&self, deltas: Vec<StreamDelta>) -> FauxStreamControl {
        let gate = Arc::new(Semaphore::new(0));
        self.turns.lock().push_back(Turn {
            deltas,
            gate: Some(gate.clone()),
        });
        FauxStreamControl(gate)
    }

    pub fn faux_text(&self, text: &str) {
        self.faux_turn(vec![StreamDelta::Text(text.into())]);
    }

    pub fn faux_tool_call(&self, name: &str, input: Value) {
        self.faux_turn(vec![StreamDelta::ToolCall(ToolCall {
            id: format!("faux_{name}"),
            name: name.into(),
            input,
        })]);
    }

    pub fn faux_structured(&self, value: Value) {
        self.structured.lock().push_back(value);
    }
}

#[async_trait]
impl LlmEngine for FauxEngine {
    async fn stream_turn(
        &self,
        _req: TurnRequest,
        event_tx: mpsc::Sender<StreamDelta>,
        cancel: CancellationToken,
    ) -> Result<TurnResponse, DriverError> {
        let turn = self
            .turns
            .lock()
            .pop_front()
            .ok_or(DriverError::FauxExhausted)?;
        let mut resp = TurnResponse::default();
        for (i, delta) in turn.deltas.into_iter().enumerate() {
            if i == 1 {
                if let Some(gate) = &turn.gate {
                    tokio::select! {
                        _ = cancel.cancelled() => return Err(DriverError::Cancelled),
                        permit = gate.acquire() => permit.map_err(|_| DriverError::Cancelled)?.forget(),
                    }
                }
            }
            match &delta {
                StreamDelta::Text(t) => resp.text.push_str(t),
                StreamDelta::ToolCall(c) => resp.tool_calls.push(c.clone()),
                StreamDelta::Thinking(_) => {}
            }
            tokio::select! {
                _ = cancel.cancelled() => return Err(DriverError::Cancelled),
                sent = event_tx.send(delta) => sent.map_err(|_| DriverError::Cancelled)?,
            }
        }
        Ok(resp)
    }

    async fn complete_structured<T: DeserializeOwned + JsonSchema + Send>(
        &self,
        _req: StructuredRequest,
        _cancel: CancellationToken,
    ) -> Result<StructuredOutput<T>, DriverError> {
        let raw = self
            .structured
            .lock()
            .pop_front()
            .ok_or(DriverError::FauxExhausted)?;
        let value =
            serde_json::from_value(raw.clone()).map_err(|e| DriverError::Parse(e.to_string()))?;
        Ok(StructuredOutput { value, raw })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn streams_scripted_deltas_in_order() {
        let engine = FauxEngine::new();
        engine.faux_turn(vec![
            StreamDelta::Thinking("hm".into()),
            StreamDelta::Text("hello".into()),
        ]);
        engine.faux_tool_call("probe", serde_json::json!({"q": 1}));
        let (tx, mut rx) = mpsc::channel(8);

        let r1 = engine
            .stream_turn(TurnRequest::default(), tx.clone(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(r1.text, "hello");
        assert_eq!(rx.recv().await, Some(StreamDelta::Thinking("hm".into())));
        assert_eq!(rx.recv().await, Some(StreamDelta::Text("hello".into())));

        let r2 = engine
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(r2.tool_calls[0].name, "probe");
        assert!(matches!(
            engine
                .stream_turn(
                    TurnRequest::default(),
                    mpsc::channel(1).0,
                    CancellationToken::new()
                )
                .await,
            Err(DriverError::FauxExhausted)
        ));
    }

    #[tokio::test]
    async fn barrier_holds_turn_until_released() {
        let engine = Arc::new(FauxEngine::new());
        let ctl = engine.faux_gated_turn(vec![
            StreamDelta::Text("a".into()),
            StreamDelta::Text("b".into()),
        ]);
        let (tx, mut rx) = mpsc::channel(8);
        let e = engine.clone();
        let task = tokio::spawn(async move {
            e.stream_turn(TurnRequest::default(), tx, CancellationToken::new())
                .await
        });

        assert_eq!(rx.recv().await, Some(StreamDelta::Text("a".into())));
        tokio::task::yield_now().await;
        assert!(rx.try_recv().is_err());
        ctl.release();
        assert_eq!(rx.recv().await, Some(StreamDelta::Text("b".into())));
        assert_eq!(task.await.unwrap().unwrap().text, "ab");
    }

    #[tokio::test]
    async fn cancel_unblocks_gated_turn() {
        let engine = FauxEngine::new();
        let _ctl = engine.faux_gated_turn(vec![
            StreamDelta::Text("a".into()),
            StreamDelta::Text("b".into()),
        ]);
        let cancel = CancellationToken::new();
        cancel.cancel();
        let (tx, _rx) = mpsc::channel(8);
        let r = engine.stream_turn(TurnRequest::default(), tx, cancel).await;
        assert!(matches!(r, Err(DriverError::Cancelled)));
    }

    #[tokio::test]
    async fn structured_round_trips_through_schema_type() {
        #[derive(serde::Deserialize, schemars::JsonSchema, PartialEq, Debug)]
        struct Out {
            n: u32,
        }
        let engine = FauxEngine::new();
        engine.faux_structured(serde_json::json!({"n": 7}));
        let out: StructuredOutput<Out> = engine
            .complete_structured(StructuredRequest::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(out.value, Out { n: 7 });
    }
}
