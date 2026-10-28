use std::path::PathBuf;

use super::tools::{self, ASK, FIND, GREP, LS, PROBE, PROBE_DONE, READ, RECALL};
use crate::workbench;

use super::{Agent, Call, Effect, Session, ToolSpec};

pub struct ProberAgent {
    pub root: PathBuf,
}

impl Agent for ProberAgent {
    fn role_prompt(&self, _: &Session) -> String {
        include_str!("prompts/prober.md").to_string()
    }

    fn tools(&self) -> Vec<&'static ToolSpec> {
        vec![
            &PROBE,
            &RECALL,
            &ASK,
            &PROBE_DONE,
            &READ,
            &LS,
            &FIND,
            &GREP,
            &workbench::RESEARCH,
            &workbench::PDF,
            &workbench::LIST_SESSIONS,
            &workbench::LIST_PROPOSITIONS,
        ]
    }

    fn exec(&self, s: &mut Session, call: &Call) -> Result<Effect, String> {
        match call.name.as_str() {
            "probe" => tools::probe(call),
            "recall" => tools::recall(call),
            "ask_user_question" => tools::ask(call),
            "probe_done" => tools::probe_done(s, call),
            _ => workbench::exec(s, call)
                .or_else(|| tools::file_tool(&self.root, call))
                .unwrap_or_else(|| Err(format!("unknown tool `{}`", call.name))),
        }
    }
}
