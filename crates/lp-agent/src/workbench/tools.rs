//! Tool-side hooks of the agent loops: job dispatch requests and the graph `list_*` tools.

use std::sync::Arc;

use lp_core::ports::{GraphStore, JobKind};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::extract::audit;
use crate::loops::{Call, Effect, Session, ToolSpec};

use super::agent::str_arg;

pub const RESEARCH: ToolSpec = ToolSpec {
    name: "research",
    usage: "background web research, answered later by a steer. {question}",
};
pub const PDF: ToolSpec = ToolSpec {
    name: "pdf",
    usage: "background PDF reading, answered later by a steer. {path, question}",
};
pub const VISUAL: ToolSpec = ToolSpec {
    name: "visual",
    usage:
        "background diagram (SVG plus terminal fallback), delivered later by a steer. {description}",
};
pub const DEMO: ToolSpec = ToolSpec {
    name: "demo",
    usage: "background interactive demo, delivered later by a steer. {description}",
};
pub const LIST_SESSIONS: ToolSpec = ToolSpec {
    name: "list_sessions",
    usage: "list the sessions that contributed to the knowledge graph. {}",
};
pub const LIST_PROPOSITIONS: ToolSpec = ToolSpec {
    name: "list_propositions",
    usage: "list the first propositions of one graph session. {session}",
};

/// A dispatch the model asked for; `Workbench::launch` starts it once the turn is over.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct JobRequest {
    pub call_id: String,
    pub kind: JobKind,
    pub args: Value,
}

/// Read-only graph access for `list_*`; not part of the persisted session.
#[derive(Clone)]
pub struct GraphHandle(pub Arc<dyn GraphStore>);

impl std::fmt::Debug for GraphHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("GraphHandle")
    }
}

impl PartialEq for GraphHandle {
    fn eq(&self, o: &Self) -> bool {
        Arc::ptr_eq(&self.0, &o.0)
    }
}

/// Deterministic, so re-launching the same call can never create a second job.
pub fn job_id(chat_id: &str, call_id: &str) -> String {
    let h = hex::encode(Sha256::digest(format!("{chat_id}\n{call_id}")));
    format!("job_{}", &h[..12])
}

fn dispatch(
    s: &mut Session,
    call: &Call,
    kind: JobKind,
    required: &[&str],
) -> Result<Effect, String> {
    for key in required {
        str_arg(&call.args, key)?;
    }
    s.requests.push(JobRequest {
        call_id: call.id.clone(),
        kind,
        args: call.args.clone(),
    });
    Ok(Effect::Ack(format!(
        "dispatched as {} — continue your turn; the result arrives as a steer",
        job_id(&s.id, &call.id)
    )))
}

fn list(s: &Session, call: &Call) -> Result<Effect, String> {
    let g = &s.graph.as_ref().ok_or("no knowledge graph is attached")?.0;
    let be = |e: lp_core::ports::StoreError| e.to_string();
    let lines: Vec<String> = if call.name == "list_sessions" {
        audit::list_sessions(g.as_ref())
            .map_err(be)?
            .into_iter()
            .collect()
    } else {
        let session = str_arg(&call.args, "session")?;
        audit::list_propositions(g.as_ref(), session)
            .map_err(be)?
            .iter()
            .map(|n| format!("{}: {} - {}", n.id, n.name, n.statement))
            .collect()
    };
    Ok(Effect::Ack(lines.join("\n")))
}

/// Workbench tools for the agents' `exec`, or `None` if `call` is not one.
pub fn exec(s: &mut Session, call: &Call) -> Option<Result<Effect, String>> {
    Some(match call.name.as_str() {
        "research" => dispatch(s, call, JobKind::Research, &["question"]),
        "pdf" => dispatch(s, call, JobKind::Pdf, &["path", "question"]),
        "visual" => dispatch(s, call, JobKind::Visual, &["description"]),
        "demo" => dispatch(s, call, JobKind::Demo, &["description"]),
        "list_sessions" | "list_propositions" => list(s, call),
        _ => return None,
    })
}

impl Session {
    /// Attach the knowledge graph the `list_*` tools read.
    pub fn with_graph(mut self, graph: Arc<dyn GraphStore>) -> Self {
        self.graph = Some(GraphHandle(graph));
        self
    }
}
