//! Faux scenario scripts (docs/web/07 §Fixtures): JSON files that queue scripted model turns on the
//! shared `FauxEngine` and engine failures (rate limit, crash) on the [`FaultPlan`], in call order.
//!
//! ```json
//! { "name": "teach", "steps": [
//!     { "turn": [ { "thinking": "…" }, { "text": "Hi" },
//!                 { "tool": { "name": "probe", "input": { … } } } ] },
//!     { "fault": { "kind": "rateLimit", "message": "rate limit reached; try again in 1 seconds" } },
//!     { "turn": [ { "tool": { "name": "quiz", "input": "{\"encoded\": \"args\"}" } } ], "hold": true }
//! ] }
//! ```
//! A tool `input` given as a string is sent as-is, like a CLI that double-encodes its arguments
//! (the agent loop heals it). `hold` stalls the turn after its first delta until released.
//!
//! Every engine call of a chat turn counts as one call, retries included; step `i` of a script
//! applied on an idle engine is call `i` from then on.

use std::collections::BTreeMap;
use std::sync::atomic::{AtomicU64, Ordering};

use lp_cli_driver::engine::{DriverError, FauxEngine, FauxStreamControl, StreamDelta, ToolCall};
use parking_lot::Mutex;
use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Script {
    pub name: String,
    #[serde(default)]
    pub description: String,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Step {
    Turn {
        turn: Vec<Delta>,
        #[serde(default)]
        hold: bool,
    },
    Fault {
        fault: Fault,
    },
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub enum Delta {
    Text(String),
    Thinking(String),
    Tool(ToolStep),
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ToolStep {
    pub name: String,
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub input: Value,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FaultKind {
    /// The CLI reports a usage/rate limit; the turn queues and retries.
    RateLimit,
    /// The CLI dies mid-turn; the turn ends in a retryable error.
    Crash,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Fault {
    pub kind: FaultKind,
    #[serde(default)]
    pub message: Option<String>,
    /// Text emitted before a simulated process crash.
    #[serde(default, rename = "partialText")]
    pub partial_text: Option<String>,
}

impl Fault {
    /// The driver error a real CLI would produce for this fault.
    pub fn error(&self) -> DriverError {
        let (code, default) = match self.kind {
            FaultKind::RateLimit => (1, "rate limit reached; try again in 1 seconds"),
            FaultKind::Crash => (101, "the CLI process crashed"),
        };
        DriverError::Exit {
            code: Some(code),
            stderr: self.message.clone().unwrap_or_else(|| default.to_owned()),
        }
    }
}

/// Engine failures scheduled by call number.
#[derive(Default)]
pub struct FaultPlan {
    calls: AtomicU64,
    at: Mutex<BTreeMap<u64, Fault>>,
}

impl FaultPlan {
    /// Counts one engine call; the fault scheduled for it, if any.
    pub fn next(&self) -> Option<Fault> {
        let n = self.calls.fetch_add(1, Ordering::AcqRel);
        self.at.lock().remove(&n)
    }

    /// Fails the engine call `offset` calls from now (0 = the next one).
    pub fn schedule(&self, offset: u64, fault: Fault) {
        let n = self.calls.load(Ordering::Acquire) + offset;
        self.at.lock().insert(n, fault);
    }

    /// Faults still waiting.
    pub fn pending(&self) -> usize {
        self.at.lock().len()
    }
}

impl Script {
    pub fn parse(json: &str) -> serde_json::Result<Self> {
        serde_json::from_str(json)
    }

    /// Queues the script; returns the release handles of held turns in script order.
    pub fn apply(&self, faux: &FauxEngine, faults: &FaultPlan) -> Vec<FauxStreamControl> {
        let mut holds = Vec::new();
        for (i, step) in self.steps.iter().enumerate() {
            match step {
                Step::Turn { turn, hold } => {
                    let deltas: Vec<StreamDelta> = turn
                        .iter()
                        .map(|d| match d {
                            Delta::Text(t) => StreamDelta::Text(t.clone()),
                            Delta::Thinking(t) => StreamDelta::Thinking(t.clone()),
                            Delta::Tool(t) => StreamDelta::ToolCall(ToolCall {
                                id: t.id.clone().unwrap_or_else(|| format!("faux_{}", t.name)),
                                name: t.name.clone(),
                                input: t.input.clone(),
                            }),
                        })
                        .collect();
                    if *hold {
                        holds.push(faux.faux_gated_turn(deltas));
                    } else {
                        faux.faux_turn(deltas);
                    }
                }
                Step::Fault { fault } => faults.schedule(i as u64, fault.clone()),
            }
        }
        holds
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_cli_driver::engine::{LlmEngine, TurnRequest};
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    #[tokio::test]
    async fn steps_queue_turns_and_faults_in_call_order() {
        let script = Script::parse(
            r#"{"name":"t","steps":[
                {"turn":[{"text":"a"},{"tool":{"name":"probe","input":{"q":1}}}]},
                {"fault":{"kind":"rateLimit"}},
                {"turn":[{"thinking":"hm"},{"tool":{"name":"quiz","id":"x","input":"{\"k\":2}"}}]}
            ]}"#,
        )
        .unwrap();
        let (faux, plan) = (FauxEngine::new(), FaultPlan::default());
        assert!(script.apply(&faux, &plan).is_empty());
        let mut seen = Vec::new();
        for _ in 0..3 {
            if let Some(f) = plan.next() {
                seen.push(f.error().to_string());
                continue;
            }
            let (tx, _rx) = mpsc::channel(8);
            let r = faux
                .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
                .await
                .unwrap();
            seen.push(format!("{}|{}", r.text, r.tool_calls[0].name));
            if r.tool_calls[0].id == "x" {
                assert_eq!(r.tool_calls[0].input, Value::String("{\"k\":2}".into()));
            }
        }
        assert_eq!(seen[0], "a|probe");
        assert!(seen[1].contains("rate limit"), "{}", seen[1]);
        assert_eq!(seen[2], "|quiz");
        assert_eq!(plan.pending(), 0);
    }

    #[test]
    fn bad_scripts_are_rejected() {
        assert!(Script::parse(r#"{"name":"t","steps":[{"turn":[{"bogus":1}]}]}"#).is_err());
        assert!(Script::parse(r#"{"name":"t","steps":[{"fault":{"kind":"nope"}}]}"#).is_err());
        assert!(Script::parse(r#"{"steps":[]}"#).is_err());
    }
}
