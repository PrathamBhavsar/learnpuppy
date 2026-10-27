use std::path::PathBuf;

use super::tools::{self, CONCLUDE, FIND, GREP, LS, PROBLEM, READ};
use super::{Agent, Call, Effect, Session, ToolSpec};

pub struct PracticeAgent {
    pub root: PathBuf,
}

impl Agent for PracticeAgent {
    fn role_prompt(&self, _: &Session) -> String {
        include_str!("prompts/practice.md").to_string()
    }

    fn tools(&self) -> Vec<&'static ToolSpec> {
        vec![&PROBLEM, &CONCLUDE, &READ, &LS, &FIND, &GREP]
    }

    fn exec(&self, s: &mut Session, call: &Call) -> Result<Effect, String> {
        match call.name.as_str() {
            "problem" => tools::problem(s, call),
            "conclude" => tools::conclude(s, call),
            _ => tools::file_tool(&self.root, call)
                .unwrap_or_else(|| Err(format!("unknown tool `{}`", call.name))),
        }
    }
}
