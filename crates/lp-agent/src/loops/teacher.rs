use std::path::PathBuf;

use lp_core::plan::{derive_plan_statuses, ToolCall};

use super::tools::{self, ASK, EXPLAIN, FIND, GREP, LS, PLAN_INIT, QUIZ, READ};
use super::{Agent, Call, Effect, Session, ToolSpec};

pub struct TeacherAgent {
    pub root: PathBuf,
}

impl Agent for TeacherAgent {
    fn role_prompt(&self, s: &Session) -> String {
        let mut out = include_str!("prompts/teacher.md").to_string();
        match &s.plan {
            None => out.push_str("\nMode: planning. No plan exists yet; call plan_init first.\n"),
            Some(tree) => {
                let calls: Vec<ToolCall> = s
                    .calls()
                    .map(|c| ToolCall {
                        name: c.name.clone(),
                        args: c.args.clone(),
                    })
                    .collect();
                let st = derive_plan_statuses(tree, &calls);
                out.push_str(&format!("\nMode: teaching. Plan v{}.\n", tree.version));
                for n in &tree.nodes {
                    out.push_str(&format!("- {}: {} ({})\n", n.id, n.title, n.summary));
                }
                out.push_str(&format!(
                    "Current: {}. Visited: {}. Upcoming: {}.\n",
                    st.current.as_deref().unwrap_or("none"),
                    st.visited.join(", "),
                    st.upcoming.join(", ")
                ));
            }
        }
        out
    }

    fn tools(&self) -> Vec<&'static ToolSpec> {
        vec![&PLAN_INIT, &QUIZ, &EXPLAIN, &ASK, &READ, &LS, &FIND, &GREP]
    }

    fn exec(&self, s: &mut Session, call: &Call) -> Result<Effect, String> {
        match call.name.as_str() {
            "plan_init" => tools::plan_init(s, call),
            "quiz" => tools::quiz(s, call),
            "explain" => tools::explain(s, call),
            "ask_user_question" => tools::ask(call),
            _ => tools::file_tool(&self.root, call)
                .unwrap_or_else(|| Err(format!("unknown tool `{}`", call.name))),
        }
    }
}
