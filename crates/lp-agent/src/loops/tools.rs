//! Tool handlers shared by the agents: question tools, plan/probe/practice tools and read-only file tools.

use std::fs::{self, File};
use std::io::Read;
use std::path::{Path, PathBuf};

use lp_core::plan::{
    ask_option_problems, parse_plan, probe_done_check, problem_input_problems, quiz_input_problems,
    ConcludeInput, PlanTree, PracticeGate, ProblemInput, QuizInput, QuizOption,
    PROBE_DONE_ACK_TEXT,
};
use lp_core::redact::redact;
use serde::de::DeserializeOwned;
use serde::Deserialize;

use super::{Call, Effect, Msg, Session, ToolSpec, HANDOFF_TEXT};

const MAX_READ_CHARS: usize = 20_000;
const MAX_HITS: usize = 100;

pub(super) const PROBE: ToolSpec = ToolSpec {
    name: "probe",
    usage: r#"multiple-choice check. {question, options:[{label,value}], correctAnswer, explanation, multiSelect?}; no type or nodeId"#,
};
pub(super) const RECALL: ToolSpec = ToolSpec {
    name: "recall",
    usage: "free-text check. {question, modelAnswer}",
};
pub(super) const ASK: ToolSpec = ToolSpec {
    name: "ask_user_question",
    usage: "ungraded question. {question, options:[{label,value}]}",
};
pub(super) const PROBE_DONE: ToolSpec = ToolSpec {
    name: "probe_done",
    usage: "end the probe once no question is unanswered. {missed?:[{question,learnerAnswer,correct}]} list every answer that was wrong or missed, including free-text recalls",
};
pub(super) const PLAN_INIT: ToolSpec = ToolSpec {
    name: "plan_init",
    usage: "author or rewrite the plan. {mermaid}",
};
pub(super) const QUIZ: ToolSpec = ToolSpec {
    name: "quiz",
    usage: r#"graded multiple choice. {type:"check"|"predict", nodeId, question, options:[{label,value}], correctAnswer, explanation, multiSelect?}"#,
};
pub(super) const EXPLAIN: ToolSpec = ToolSpec {
    name: "explain",
    usage: r#"teach a node, then let the learner think. {type:"check"|"predict", nodeId, content}"#,
};
pub(super) const PROBLEM: ToolSpec = ToolSpec {
    name: "problem",
    usage: r#"pose one problem. {statement, mode:"recall"|"discrimination"|"composition", target?:[nodeId], modelSolution}"#,
};
pub(super) const CONCLUDE: ToolSpec = ToolSpec {
    name: "conclude",
    usage: r#"close the open problem and reveal the model solution. {outcome:"solved"|"solvedWithHints"|"movedOn"|"surrendered"|"dismissed", verdict, rung:"R0".."R5"|null, missClass:"cue"|"discrimination"|"execution"|"careless"|"structural"|null, retest}"#,
};
pub(super) const READ: ToolSpec = ToolSpec {
    name: "read",
    usage: "read a workspace file. {path}",
};
pub(super) const LS: ToolSpec = ToolSpec {
    name: "ls",
    usage: "list a workspace directory. {path?}",
};
pub(super) const FIND: ToolSpec = ToolSpec {
    name: "find",
    usage: "find files whose name contains pattern. {pattern}",
};
pub(super) const GREP: ToolSpec = ToolSpec {
    name: "grep",
    usage: "find lines containing pattern. {pattern, path?}",
};

fn parse<T: DeserializeOwned>(call: &Call) -> Result<T, String> {
    serde_json::from_value(call.args.clone()).map_err(|e| format!("invalid arguments: {e}"))
}

fn check(problems: Vec<String>) -> Result<Effect, String> {
    if problems.is_empty() {
        Ok(Effect::Await)
    } else {
        Err(problems.join("; "))
    }
}

pub(super) fn probe(call: &Call) -> Result<Effect, String> {
    check(quiz_input_problems(&parse::<QuizInput>(call)?, None))
}

pub(super) fn quiz(s: &Session, call: &Call) -> Result<Effect, String> {
    let tree = plan(s)?;
    check(quiz_input_problems(&parse::<QuizInput>(call)?, Some(tree)))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Recall {
    question: String,
    model_answer: String,
}

pub(super) fn recall(call: &Call) -> Result<Effect, String> {
    let r: Recall = parse(call)?;
    let mut problems = Vec::new();
    if r.question.trim().is_empty() || r.model_answer.trim().is_empty() {
        problems.push("question and modelAnswer must be non-empty".to_string());
    }
    check(problems)
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Explain {
    #[serde(rename = "type")]
    kind: String,
    node_id: String,
    content: String,
}

pub(super) fn explain(s: &Session, call: &Call) -> Result<Effect, String> {
    let e: Explain = parse(call)?;
    let mut problems = Vec::new();
    if !matches!(e.kind.as_str(), "check" | "predict") {
        problems.push("type must be `check` or `predict`".to_string());
    }
    if !plan(s)?.nodes.iter().any(|n| n.id == e.node_id) {
        problems.push(format!("nodeId `{}` is not in the plan", e.node_id));
    }
    if e.content.trim().is_empty() {
        problems.push("content must be non-empty".to_string());
    }
    check(problems)
}

#[derive(Deserialize)]
struct Ask {
    question: String,
    options: Vec<QuizOption>,
}

pub(super) fn ask(call: &Call) -> Result<Effect, String> {
    let a: Ask = parse(call)?;
    let mut problems = ask_option_problems(&a.options);
    if a.question.trim().is_empty() {
        problems.push("question must be non-empty".to_string());
    }
    check(problems)
}

#[derive(Default, Deserialize)]
struct ProbeDone {
    #[serde(default)]
    missed: Vec<Missed>,
}

/// A probe answer the learner got wrong or missed.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Missed {
    question: String,
    learner_answer: String,
    correct: String,
}

/// Wrong `probe` answers, judged mechanically against the committed `correctAnswer`.
fn wrong_choices(s: &Session) -> Vec<Missed> {
    let label = |args: &serde_json::Value, v: &str| {
        let opt = args["options"]
            .as_array()
            .and_then(|o| o.iter().find(|o| o["value"] == v));
        opt.and_then(|o| o["label"].as_str())
            .unwrap_or(v)
            .to_string()
    };
    let mut out = Vec::new();
    for c in s.calls().filter(|c| c.name == "probe") {
        let Some(Msg::Answer { text, .. }) = s
            .messages
            .iter()
            .find(|m| matches!(m, Msg::Answer { call_id, .. } if *call_id == c.id))
        else {
            continue;
        };
        let (Some(right), Some(sel)) = (
            c.args["correctAnswer"].as_str(),
            text.strip_prefix("selected: ")
                .and_then(|r| r.split(';').next()),
        ) else {
            continue;
        };
        if sel != right {
            out.push(Missed {
                question: c.args["question"].as_str().unwrap_or_default().to_string(),
                learner_answer: label(&c.args, sel),
                correct: label(&c.args, right),
            });
        }
    }
    out
}

pub(super) fn probe_done(s: &mut Session, call: &Call) -> Result<Effect, String> {
    let given: ProbeDone = parse(call)?;
    probe_done_with(s, given.missed)
}

/// Ends the probe and queues the handoff steer, carrying every missed answer to the teacher.
pub(super) fn probe_done_with(s: &mut Session, mut missed: Vec<Missed>) -> Result<Effect, String> {
    probe_done_check(s.pending_question(), &s.outcomes()).map_err(|e| e.to_string())?;
    for w in wrong_choices(s) {
        if !missed.iter().any(|m| m.question == w.question) {
            missed.push(w);
        }
    }
    let mut text = HANDOFF_TEXT.to_string();
    if !missed.is_empty() {
        text.push_str(
            "\nThe learner missed or got wrong these probe answers. Correct each one at the plan node it belongs to (explain or quiz it there) before moving on:",
        );
        for m in &missed {
            text.push_str(&format!(
                "\n- question: {} | learner answered: {} | correct: {}",
                m.question, m.learner_answer, m.correct
            ));
        }
    }
    s.steers.push((format!("steer_probe_{}", s.id), text));
    // The turn ends here; the caller's wake delivers the handoff and the teacher starts fresh.
    Ok(Effect::End(PROBE_DONE_ACK_TEXT.to_string()))
}

#[derive(Deserialize)]
struct PlanInit {
    mermaid: String,
}

pub(super) fn plan_init(s: &mut Session, call: &Call) -> Result<Effect, String> {
    let version = s.plan.as_ref().map_or(1, |p| p.version + 1);
    let tree = parse_plan(&parse::<PlanInit>(call)?.mermaid, version).map_err(|e| e.to_string())?;
    let msg = format!("plan v{version} accepted with {} nodes", tree.nodes.len());
    s.plan = Some(tree);
    Ok(Effect::Ack(msg))
}

fn plan(s: &Session) -> Result<&PlanTree, String> {
    s.plan
        .as_ref()
        .ok_or_else(|| "no plan yet; call plan_init first".to_string())
}

pub(super) fn problem(s: &mut Session, call: &Call) -> Result<Effect, String> {
    check(problem_input_problems(&parse::<ProblemInput>(call)?))?;
    let mut gate = PracticeGate {
        problem_active: s.problem_active,
    };
    gate.problem().map_err(|e| e.to_string())?;
    s.problem_active = gate.problem_active;
    Ok(Effect::Await)
}

pub(super) fn conclude(s: &mut Session, call: &Call) -> Result<Effect, String> {
    parse::<ConcludeInput>(call)?;
    let mut gate = PracticeGate {
        problem_active: s.problem_active,
    };
    gate.conclude().map_err(|e| e.to_string())?;
    s.problem_active = gate.problem_active;
    // `modelSolution` was committed with the problem call and is revealed only here.
    let solution = s
        .messages
        .iter()
        .rev()
        .find_map(|m| match m {
            Msg::Assistant { calls, .. } => calls.iter().rev().find(|c| c.name == "problem"),
            _ => None,
        })
        .and_then(|c| c.args.get("modelSolution")?.as_str())
        .unwrap_or_default();
    Ok(Effect::End(format!("Model solution: {solution}")))
}

// ---------------------------------------------------------------- file tools

#[derive(Deserialize)]
struct PathArg {
    #[serde(default)]
    path: String,
}

#[derive(Deserialize)]
struct PatternArg {
    pattern: String,
    #[serde(default)]
    path: String,
}

/// Resolve `rel` inside `root`, refusing anything that escapes it (`..`, absolute paths, symlinks).
fn resolve(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let root = root.canonicalize().map_err(|e| e.to_string())?;
    let full = root
        .join(rel)
        .canonicalize()
        .map_err(|e| format!("{rel}: {e}"))?;
    // Checked on the canonical path so a visible symlink to `.env` is refused too.
    let inside = full
        .strip_prefix(&root)
        .map_err(|_| format!("{rel}: outside the workspace"))?;
    if inside
        .components()
        .any(|c| c.as_os_str().to_string_lossy().starts_with('.'))
    {
        return Err(format!("{rel}: hidden paths are not readable"));
    }
    Ok(full)
}

fn rel(root: &Path, p: &Path) -> String {
    p.strip_prefix(root).unwrap_or(p).display().to_string()
}

/// Every non-hidden, non-symlink regular file under `dir`, skipping build and dependency trees.
fn walk(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        let Ok(entries) = fs::read_dir(&d) else {
            continue;
        };
        for e in entries.flatten() {
            let Ok(ft) = e.file_type() else { continue };
            let name = e.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') || ft.is_symlink() {
                continue;
            }
            if ft.is_dir() {
                if name != "target" && name != "node_modules" {
                    stack.push(e.path());
                }
            } else if ft.is_file() {
                out.push(e.path());
            }
        }
    }
    out.sort();
    out
}

/// At most `MAX_READ_CHARS` chars of a regular file, secrets masked; never reads the rest.
fn read_capped(path: &Path) -> Result<String, String> {
    let file = File::open(path).map_err(|e| format!("{}: {e}", path.display()))?;
    if !file.metadata().is_ok_and(|m| m.is_file()) {
        return Err(format!("{}: not a regular file", path.display()));
    }
    let mut buf = Vec::new();
    // 4 bytes per char bounds the cap; a cut multi-byte tail becomes U+FFFD and is trimmed below.
    file.take(MAX_READ_CHARS as u64 * 4)
        .read_to_end(&mut buf)
        .map_err(|e| format!("{}: {e}", path.display()))?;
    if buf.contains(&0) {
        return Err(format!("{}: binary file", path.display()));
    }
    let text: String = String::from_utf8_lossy(&buf)
        .chars()
        .take(MAX_READ_CHARS)
        .collect();
    Ok(redact(&text))
}

pub(super) fn read(root: &Path, call: &Call) -> Result<Effect, String> {
    let path = resolve(root, &parse::<PathArg>(call)?.path)?;
    Ok(Effect::Ack(read_capped(&path)?))
}

pub(super) fn ls(root: &Path, call: &Call) -> Result<Effect, String> {
    let dir = resolve(root, &parse::<PathArg>(call)?.path)?;
    let mut names: Vec<String> = fs::read_dir(&dir)
        .map_err(|e| format!("{}: {e}", dir.display()))?
        .flatten()
        .map(|e| {
            let slash = if e.path().is_dir() { "/" } else { "" };
            format!("{}{slash}", e.file_name().to_string_lossy())
        })
        .collect();
    names.sort();
    Ok(Effect::Ack(names.join("\n")))
}

pub(super) fn find(root: &Path, call: &Call) -> Result<Effect, String> {
    let a: PatternArg = parse(call)?;
    let base = resolve(root, &a.path)?;
    let hits: Vec<String> = walk(&base)
        .iter()
        .filter(|p| {
            p.file_name()
                .is_some_and(|n| n.to_string_lossy().contains(&a.pattern))
        })
        .take(MAX_HITS)
        .map(|p| rel(&base, p))
        .collect();
    Ok(Effect::Ack(hits.join("\n")))
}

pub(super) fn grep(root: &Path, call: &Call) -> Result<Effect, String> {
    let a: PatternArg = parse(call)?;
    let base = resolve(root, &a.path)?;
    let mut hits = Vec::new();
    'files: for p in walk(&base) {
        let Ok(text) = read_capped(&p) else {
            continue;
        };
        for (i, line) in text.lines().enumerate() {
            if line.contains(&a.pattern) {
                hits.push(format!("{}:{}: {line}", rel(&base, &p), i + 1));
                if hits.len() >= MAX_HITS {
                    break 'files;
                }
            }
        }
    }
    Ok(Effect::Ack(hits.join("\n")))
}

/// `read`/`ls`/`find`/`grep`, or `None` if `name` is not a file tool.
pub(super) fn file_tool(root: &Path, call: &Call) -> Option<Result<Effect, String>> {
    Some(match call.name.as_str() {
        "read" => read(root, call),
        "ls" => ls(root, call),
        "find" => find(root, call),
        "grep" => grep(root, call),
        _ => return None,
    })
}
