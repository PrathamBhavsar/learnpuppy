//! Plan DAG Mermaid parser, 3-phase fold, question_core, Fisher-Yates shuffle, argument healing, and PracticeGate (T11).
use std::collections::{HashMap, HashSet};

use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use thiserror::Error;

// ---------------------------------------------------------------- plan DAG

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanNode {
    pub id: String,
    pub title: String,
    pub summary: String,
}

/// Structure only; completion is derived from calls, never stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanTree {
    pub nodes: Vec<PlanNode>,
    pub edges: Vec<(String, String)>,
    pub version: u32,
}

impl PlanTree {
    fn has(&self, id: &str) -> bool {
        self.nodes.iter().any(|n| n.id == id)
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PlanError {
    #[error("line {line}: {msg}")]
    Syntax { line: usize, msg: String },
    #[error("line {line}: cycle {}", .path.join(" -> "))]
    Cycle { line: usize, path: Vec<String> },
}

fn syntax(line: usize, msg: impl Into<String>) -> PlanError {
    PlanError::Syntax {
        line,
        msg: msg.into(),
    }
}

/// Parse the `plan_init` Mermaid subset (`graph TD` / `flowchart TD`).
pub fn parse_plan(src: &str, version: u32) -> Result<PlanTree, PlanError> {
    let term = Regex::new(r#"^([a-z0-9-]+)(?:\["([^"]*)"\])?$"#).expect("static regex");
    let mut lines = src
        .lines()
        .enumerate()
        .map(|(i, l)| (i + 1, l.trim()))
        .filter(|(_, l)| !l.is_empty() && !l.starts_with("%%"));

    match lines.next() {
        Some((_, "graph TD" | "flowchart TD")) => {}
        Some((n, _)) => return Err(syntax(n, "expected `graph TD` or `flowchart TD` header")),
        None => return Err(syntax(1, "empty plan")),
    }

    let mut nodes: Vec<PlanNode> = Vec::new();
    let mut declared_at: HashMap<String, usize> = HashMap::new();
    let mut refs: Vec<(usize, String)> = Vec::new();
    let mut edges: Vec<(usize, String, String)> = Vec::new();

    for (n, line) in lines {
        let mut prev: Vec<String> = Vec::new();
        for (si, seg) in line.split("-->").enumerate() {
            let mut cur = Vec::new();
            for t in seg.split('&') {
                let t = t.trim();
                let c = term
                    .captures(t)
                    .ok_or_else(|| syntax(n, format!("invalid node or edge `{t}`")))?;
                let id = c[1].to_string();
                if let Some(text) = c.get(2) {
                    if declared_at.insert(id.clone(), n).is_some() {
                        return Err(syntax(n, format!("node `{id}` declared twice")));
                    }
                    let (title, summary) = text
                        .as_str()
                        .split_once(" — ")
                        .or_else(|| text.as_str().split_once(" -- "))
                        .unwrap_or((text.as_str(), ""));
                    nodes.push(PlanNode {
                        id: id.clone(),
                        title: title.trim().to_string(),
                        summary: summary.trim().to_string(),
                    });
                } else {
                    refs.push((n, id.clone()));
                }
                cur.push(id);
            }
            if si > 0 {
                for a in &prev {
                    for b in &cur {
                        edges.push((n, a.clone(), b.clone()));
                    }
                }
            }
            prev = cur;
        }
    }

    if nodes.is_empty() {
        return Err(syntax(1, "plan has no nodes"));
    }
    for (n, id) in &refs {
        if !declared_at.contains_key(id) {
            return Err(syntax(*n, format!("undeclared node `{id}`")));
        }
    }
    if let Some((line, path)) = find_cycle(&nodes, &edges) {
        return Err(PlanError::Cycle { line, path });
    }
    Ok(PlanTree {
        nodes,
        edges: edges.into_iter().map(|(_, a, b)| (a, b)).collect(),
        version,
    })
}

/// DFS; returns the line of the closing edge and the cycle path `a -> b -> a`.
fn find_cycle(
    nodes: &[PlanNode],
    edges: &[(usize, String, String)],
) -> Option<(usize, Vec<String>)> {
    fn visit<'a>(
        id: &'a str,
        edges: &'a [(usize, String, String)],
        done: &mut HashSet<&'a str>,
        stack: &mut Vec<&'a str>,
    ) -> Option<(usize, Vec<String>)> {
        stack.push(id);
        for (line, a, b) in edges.iter().filter(|(_, a, _)| a == id) {
            let _ = a;
            if let Some(pos) = stack.iter().position(|s| s == b) {
                let mut path: Vec<String> = stack[pos..].iter().map(|s| s.to_string()).collect();
                path.push(b.clone());
                return Some((*line, path));
            }
            if !done.contains(b.as_str()) {
                if let Some(c) = visit(b, edges, done, stack) {
                    return Some(c);
                }
            }
        }
        stack.pop();
        done.insert(id);
        None
    }
    let mut done = HashSet::new();
    nodes
        .iter()
        .find_map(|n| visit(&n.id, edges, &mut done, &mut Vec::new()))
}

/// A tool call made by the assistant.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolCall {
    pub name: String,
    pub args: Value,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlanStatuses {
    pub current: Option<String>,
    pub visited: Vec<String>,
    pub upcoming: Vec<String>,
}

/// Position-from-calls: `quiz`/`explain` calls on known nodes, consecutive duplicates collapsed.
pub fn derive_plan_statuses(tree: &PlanTree, calls: &[ToolCall]) -> PlanStatuses {
    let mut runs: Vec<&str> = Vec::new();
    for c in calls
        .iter()
        .filter(|c| matches!(c.name.as_str(), "quiz" | "explain"))
    {
        if let Some(id) = c.args.get("nodeId").and_then(Value::as_str) {
            if tree.has(id) && runs.last() != Some(&id) {
                runs.push(id);
            }
        }
    }
    let current = runs.last().map(|s| s.to_string());
    let mut visited: Vec<String> = Vec::new();
    for id in &runs[..runs.len().saturating_sub(1)] {
        if Some(*id) != current.as_deref() && !visited.iter().any(|v| v == id) {
            visited.push(id.to_string());
        }
    }
    let upcoming = tree
        .nodes
        .iter()
        .filter(|n| Some(&n.id) != current.as_ref() && !visited.contains(&n.id))
        .map(|n| n.id.clone())
        .collect();
    PlanStatuses {
        current,
        visited,
        upcoming,
    }
}

// ------------------------------------------------------------------ phases

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Probe,
    Plan,
    Teach,
}

/// A finished tool call from the transcript.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolOutcome {
    pub name: String,
    pub succeeded: bool,
}

pub fn phase_at_3(outcomes: &[ToolOutcome]) -> Phase {
    let ok = |name: &str| outcomes.iter().any(|o| o.succeeded && o.name == name);
    if ok("plan_init") {
        Phase::Teach
    } else if ok("probe_done") {
        Phase::Plan
    } else {
        Phase::Probe
    }
}

/// Fixed model-facing ack so the assessment is not duplicated in context.
pub const PROBE_DONE_ACK_TEXT: &str = "Probe complete. Handoff queued.";

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProbeDoneError {
    #[error("probe_done refused: a question is still unanswered")]
    PendingQuestion,
    #[error("probe_done refused: already succeeded")]
    AlreadyDone,
}

pub fn probe_done_check(
    pending_question: bool,
    outcomes: &[ToolOutcome],
) -> Result<(), ProbeDoneError> {
    if pending_question {
        Err(ProbeDoneError::PendingQuestion)
    } else if outcomes
        .iter()
        .any(|o| o.succeeded && o.name == "probe_done")
    {
        Err(ProbeDoneError::AlreadyDone)
    } else {
        Ok(())
    }
}

// ------------------------------------------------------------ question core

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuizOption {
    pub label: String,
    pub value: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CorrectAnswer {
    One(String),
    Many(Vec<String>),
}

impl CorrectAnswer {
    fn values(&self) -> &[String] {
        match self {
            Self::One(s) => std::slice::from_ref(s),
            Self::Many(v) => v,
        }
    }
}

/// `probe` (no `kind`/`node_id`) and `quiz` (both required) share this shape.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuizInput {
    pub question: String,
    pub options: Vec<QuizOption>,
    pub correct_answer: CorrectAnswer,
    pub explanation: String,
    #[serde(default)]
    pub multi_select: bool,
    #[serde(default, rename = "type")]
    pub kind: Option<String>,
    #[serde(default)]
    pub node_id: Option<String>,
}

pub fn ask_option_problems(options: &[QuizOption]) -> Vec<String> {
    let mut out = Vec::new();
    if options.len() < 2 {
        out.push("need at least 2 options".to_string());
    }
    let (mut labels, mut values) = (HashSet::new(), HashSet::new());
    for o in options {
        if o.label.trim().is_empty() || o.value.trim().is_empty() {
            out.push("option label and value must be non-empty".to_string());
        }
        if !labels.insert(o.label.as_str()) {
            out.push(format!("duplicate option label `{}`", o.label));
        }
        if !values.insert(o.value.as_str()) {
            out.push(format!("duplicate option value `{}`", o.value));
        }
    }
    out
}

/// Authoring validation. Pass `tree` for `quiz`/`explain` (requires `type` + `nodeId`); `None` for `probe`.
pub fn quiz_input_problems(q: &QuizInput, tree: Option<&PlanTree>) -> Vec<String> {
    let mut out = ask_option_problems(&q.options);
    if q.question.trim().is_empty() {
        out.push("question must be non-empty".to_string());
    }
    let answers = q.correct_answer.values();
    if answers.is_empty() || (!q.multi_select && answers.len() != 1) {
        out.push("correctAnswer must name exactly one option unless multiSelect".to_string());
    }
    for a in answers {
        if !q.options.iter().any(|o| &o.value == a) {
            out.push(format!("correctAnswer `{a}` is not an option value"));
        }
    }
    // Uniform option length law: the longest option must not be the tell.
    if let [only] = answers {
        let others: Vec<usize> = q
            .options
            .iter()
            .filter(|o| &o.value != only)
            .map(|o| o.label.chars().count())
            .collect();
        if let Some(c) = q.options.iter().find(|o| &o.value == only) {
            if !others.is_empty() {
                let mean = others.iter().sum::<usize>() as f64 / others.len() as f64;
                if c.label.chars().count() as f64 > 1.6 * mean + 10.0 {
                    out.push("correct option is conspicuously longer than the others".to_string());
                }
            }
        }
    }
    match tree {
        Some(t) => {
            if !matches!(q.kind.as_deref(), Some("check" | "predict")) {
                out.push("type must be `check` or `predict`".to_string());
            }
            match q.node_id.as_deref() {
                Some(id) if t.has(id) => {}
                Some(id) => out.push(format!("nodeId `{id}` is not in the plan")),
                None => out.push("nodeId is required".to_string()),
            }
        }
        None => {
            if q.kind.is_some() || q.node_id.is_some() {
                out.push("probe takes no type or nodeId".to_string());
            }
        }
    }
    out
}

/// Deterministic Fisher-Yates keyed on `seed` (the tool call id): stable across re-renders and restarts.
pub fn shuffle_options(options: &[QuizOption], seed: &str) -> Vec<QuizOption> {
    let mut out = options.to_vec();
    for i in (1..out.len()).rev() {
        let h = Sha256::new()
            .chain_update(seed.as_bytes())
            .chain_update((i as u64).to_le_bytes())
            .finalize();
        let r = u64::from_le_bytes(h[..8].try_into().expect("8 bytes"));
        out.swap(i, (r % (i as u64 + 1)) as usize);
    }
    out
}

/// Strip copy-syntax debris from a bare token: surrounding quotes and a leading `:`.
pub fn strip_model_value_debris(s: &str) -> String {
    let mut t = s.trim();
    while let Some(r) = t
        .strip_prefix(':')
        .or_else(|| t.strip_prefix('"').and_then(|r| r.strip_suffix('"')))
        .or_else(|| t.strip_prefix('\'').and_then(|r| r.strip_suffix('\'')))
    {
        t = r.trim();
    }
    t.to_string()
}

/// Pre-validation heal: parse stringified arrays/objects, and de-debris whitespace-free string tokens.
pub fn heal_encoded_arguments(v: Value) -> Value {
    match v {
        Value::String(s) => {
            let t = s.trim();
            if t.starts_with(['[', '{']) {
                if let Ok(parsed) = serde_json::from_str::<Value>(t) {
                    return heal_encoded_arguments(parsed);
                }
            }
            if t.chars().any(char::is_whitespace) {
                Value::String(s)
            } else {
                Value::String(strip_model_value_debris(t))
            }
        }
        Value::Array(a) => Value::Array(a.into_iter().map(heal_encoded_arguments).collect()),
        Value::Object(m) => Value::Object(
            m.into_iter()
                .map(|(k, v)| (k, heal_encoded_arguments(v)))
                .collect(),
        ),
        other => other,
    }
}

// ------------------------------------------------------------ PracticeGate

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ProblemMode {
    Recall,
    Discrimination,
    Composition,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProblemInput {
    pub statement: String,
    pub mode: ProblemMode,
    #[serde(default)]
    pub target: Vec<String>,
    pub model_solution: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    Solved,
    SolvedWithHints,
    MovedOn,
    Surrendered,
    Dismissed,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Rung {
    R0,
    R1,
    R2,
    R3,
    R4,
    R5,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MissClass {
    Cue,
    Discrimination,
    Execution,
    Careless,
    Structural,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConcludeInput {
    pub outcome: Outcome,
    pub verdict: String,
    pub rung: Option<Rung>,
    pub miss_class: Option<MissClass>,
    pub retest: bool,
}

pub fn problem_input_problems(p: &ProblemInput) -> Vec<String> {
    let mut out = Vec::new();
    if p.statement.trim().is_empty() {
        out.push("statement must be non-empty".to_string());
    }
    if p.model_solution.trim().is_empty() {
        out.push("modelSolution must be committed up front".to_string());
    }
    out
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GateError {
    #[error("a problem is already open; conclude it first")]
    ProblemActive,
    #[error("no open problem to conclude")]
    NoProblem,
}

/// Intra-turn gate: `problem` and `conclude` must alternate.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PracticeGate {
    pub problem_active: bool,
}

impl PracticeGate {
    pub fn problem(&mut self) -> Result<(), GateError> {
        if self.problem_active {
            return Err(GateError::ProblemActive);
        }
        self.problem_active = true;
        Ok(())
    }

    pub fn conclude(&mut self) -> Result<(), GateError> {
        if !self.problem_active {
            return Err(GateError::NoProblem);
        }
        self.problem_active = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;
    use serde_json::json;

    const PLAN: &str = r#"graph TD
    a["Basics — start here"]
    b["Mid -- the middle"]
    c["End"]
    a --> b --> c
"#;

    fn call(name: &str, node: &str) -> ToolCall {
        ToolCall {
            name: name.into(),
            args: json!({ "nodeId": node }),
        }
    }

    fn opt(l: &str, v: &str) -> QuizOption {
        QuizOption {
            label: l.into(),
            value: v.into(),
        }
    }

    fn quiz() -> QuizInput {
        QuizInput {
            question: "q?".into(),
            options: vec![opt("one", "a"), opt("two", "b"), opt("six", "c")],
            correct_answer: CorrectAnswer::One("a".into()),
            explanation: "because".into(),
            multi_select: false,
            kind: Some("check".into()),
            node_id: Some("a".into()),
        }
    }

    #[test]
    fn parses_chain_and_splits_title() {
        let t = parse_plan(PLAN, 1).unwrap();
        assert_eq!(t.nodes.len(), 3);
        assert_eq!(
            (t.nodes[0].title.as_str(), t.nodes[0].summary.as_str()),
            ("Basics", "start here")
        );
        assert_eq!(t.nodes[1].summary, "the middle");
        assert_eq!(
            t.edges,
            vec![("a".into(), "b".into()), ("b".into(), "c".into())]
        );
    }

    #[test]
    fn parses_fan_in_and_out() {
        let t = parse_plan(
            "flowchart TD\na[\"A\"]\nb[\"B\"]\nc[\"C\"]\nd[\"D\"]\na & b --> c & d",
            1,
        )
        .unwrap();
        assert_eq!(t.edges.len(), 4);
    }

    #[test]
    fn malformed_mermaid_reports_lines() {
        let e = |s| parse_plan(s, 1).unwrap_err();
        assert_eq!(
            e("graph LR\na[\"A\"]"),
            syntax(1, "expected `graph TD` or `flowchart TD` header")
        );
        assert!(matches!(e(""), PlanError::Syntax { line: 1, .. }));
        assert!(matches!(e("graph TD\n"), PlanError::Syntax { .. }));
        assert!(matches!(
            e("graph TD\na[\"A\"]\nA --> a"),
            PlanError::Syntax { line: 3, .. }
        ));
        assert!(matches!(
            e("graph TD\na[\"A\"]\na --> ghost"),
            PlanError::Syntax { line: 3, .. }
        ));
        assert!(matches!(
            e("graph TD\na[\"A\"]\na[\"B\"]"),
            PlanError::Syntax { line: 3, .. }
        ));
        assert!(matches!(
            e("graph TD\na[\"A\"\n"),
            PlanError::Syntax { line: 2, .. }
        ));
    }

    #[test]
    fn cycle_reports_path_and_line() {
        let e = parse_plan("graph TD\na[\"A\"]\nb[\"B\"]\na --> b\nb --> a", 1).unwrap_err();
        assert_eq!(
            e,
            PlanError::Cycle {
                line: 5,
                path: vec!["a".into(), "b".into(), "a".into()]
            }
        );
        assert_eq!(e.to_string(), "line 5: cycle a -> b -> a");
        assert!(matches!(
            parse_plan("graph TD\na[\"A\"]\na --> a", 1),
            Err(PlanError::Cycle { line: 3, .. })
        ));
    }

    #[test]
    fn statuses_from_calls() {
        let t = parse_plan(PLAN, 1).unwrap();
        let s = derive_plan_statuses(&t, &[]);
        assert_eq!((s.current, s.upcoming.len()), (None, 3));

        // consecutive same-node calls collapse; unknown node and other tools ignored
        let s = derive_plan_statuses(
            &t,
            &[
                call("quiz", "a"),
                call("explain", "a"),
                call("quiz", "zzz"),
                call("read", "b"),
            ],
        );
        assert_eq!(s.current.as_deref(), Some("a"));
        assert!(s.visited.is_empty());
        assert_eq!(s.upcoming, vec!["b", "c"]);

        // revisit: a -> b -> a
        let s = derive_plan_statuses(
            &t,
            &[call("quiz", "a"), call("quiz", "b"), call("explain", "a")],
        );
        assert_eq!(s.current.as_deref(), Some("a"));
        assert_eq!(s.visited, vec!["b"]);
        assert_eq!(s.upcoming, vec!["c"]);
    }

    #[test]
    fn phases_and_probe_done() {
        let o = |n: &str, ok| ToolOutcome {
            name: n.into(),
            succeeded: ok,
        };
        assert_eq!(phase_at_3(&[]), Phase::Probe);
        assert_eq!(phase_at_3(&[o("probe_done", false)]), Phase::Probe);
        assert_eq!(phase_at_3(&[o("probe_done", true)]), Phase::Plan);
        assert_eq!(
            phase_at_3(&[o("probe_done", true), o("plan_init", false)]),
            Phase::Plan
        );
        assert_eq!(
            phase_at_3(&[o("probe_done", true), o("plan_init", true)]),
            Phase::Teach
        );

        assert_eq!(
            probe_done_check(true, &[]),
            Err(ProbeDoneError::PendingQuestion)
        );
        assert_eq!(
            probe_done_check(false, &[o("probe_done", true)]),
            Err(ProbeDoneError::AlreadyDone)
        );
        assert_eq!(probe_done_check(false, &[o("probe_done", false)]), Ok(()));
    }

    #[test]
    fn quiz_validation() {
        let t = parse_plan(PLAN, 1).unwrap();
        assert!(quiz_input_problems(&quiz(), Some(&t)).is_empty());

        let mut q = quiz();
        q.options = vec![opt("x", "a")];
        q.correct_answer = CorrectAnswer::One("zz".into());
        assert!(quiz_input_problems(&q, Some(&t)).len() >= 2);

        let mut q = quiz();
        q.options = vec![opt("same", "a"), opt("same", "a")];
        assert_eq!(ask_option_problems(&q.options).len(), 2);

        let mut q = quiz();
        q.node_id = Some("nope".into());
        q.kind = None;
        assert_eq!(quiz_input_problems(&q, Some(&t)).len(), 2);

        let mut q = quiz();
        q.options[0].label = "a very long correct answer that gives it away".into();
        assert_eq!(quiz_input_problems(&q, Some(&t)).len(), 1);

        // probe: no type / nodeId allowed
        assert_eq!(quiz_input_problems(&quiz(), None).len(), 1);
        let mut q = quiz();
        q.kind = None;
        q.node_id = None;
        assert!(quiz_input_problems(&q, None).is_empty());

        // multiSelect accepts several answers
        let mut q = quiz();
        q.multi_select = true;
        q.correct_answer = CorrectAnswer::Many(vec!["a".into(), "b".into()]);
        assert!(quiz_input_problems(&q, Some(&t)).is_empty());
        q.multi_select = false;
        assert_eq!(quiz_input_problems(&q, Some(&t)).len(), 1);
    }

    #[test]
    fn shuffle_is_stable_and_seed_dependent() {
        let opts: Vec<_> = (0..8)
            .map(|i| opt(&format!("l{i}"), &format!("v{i}")))
            .collect();
        assert_eq!(
            shuffle_options(&opts, "call_1"),
            shuffle_options(&opts, "call_1")
        );
        assert!((0..20).any(|i| shuffle_options(&opts, &format!("call_{i}")) != opts));
        assert!(shuffle_options(&[], "x").is_empty());
    }

    proptest! {
        #[test]
        fn shuffle_is_a_permutation(n in 0usize..16, seed in ".{0,12}") {
            let opts: Vec<_> = (0..n).map(|i| opt(&format!("l{i}"), &format!("v{i}"))).collect();
            let mut got = shuffle_options(&opts, &seed);
            prop_assert_eq!(got.len(), n);
            got.sort_by_key(|o| o.value.clone());
            let mut want = opts.clone();
            want.sort_by_key(|o| o.value.clone());
            prop_assert_eq!(got, want);
        }

        #[test]
        fn acyclic_chains_parse(n in 1usize..8) {
            let mut s = String::from("graph TD\n");
            for i in 0..n { s += &format!("n{i}[\"N{i}\"]\n"); }
            s += &(0..n).map(|i| format!("n{i}")).collect::<Vec<_>>().join(" --> ");
            prop_assert_eq!(parse_plan(&s, 1).unwrap().edges.len(), n - 1);
        }

        #[test]
        fn heal_never_panics_and_is_idempotent(s in ".{0,24}") {
            let once = heal_encoded_arguments(json!({ "k": s }));
            prop_assert_eq!(heal_encoded_arguments(once.clone()), once);
        }
    }

    #[test]
    fn heals_double_encoded_and_debris() {
        let raw = json!({
            "options": "[{\"label\":\"A\",\"value\":\"a\"}]",
            "correctAnswer": "[\"a\", \"b\"]",
            "nodeId": ":node-id",
            "type": "\"check\"",
            "nested": "{\"x\": \"[1,2]\"}",
            "question": "keep: this \"as is\"",
            "n": 3
        });
        let h = heal_encoded_arguments(raw);
        assert_eq!(h["options"][0]["value"], "a");
        assert_eq!(h["correctAnswer"], json!(["a", "b"]));
        assert_eq!(h["nodeId"], "node-id");
        assert_eq!(h["type"], "check");
        assert_eq!(h["nested"]["x"], json!([1, 2]));
        assert_eq!(h["question"], "keep: this \"as is\"");
        assert_eq!(h["n"], 3);
        // malformed JSON string stays a string
        assert_eq!(heal_encoded_arguments(json!("[oops")), json!("[oops"));
        let q: QuizInput = serde_json::from_value(heal_encoded_arguments(json!({
            "question": "q", "options": "[{\"label\":\"A\",\"value\":\"a\"},{\"label\":\"B\",\"value\":\"b\"}]",
            "correctAnswer": "a", "explanation": "e"
        })))
        .unwrap();
        assert!(quiz_input_problems(&q, None).is_empty());
    }

    #[test]
    fn practice_gate_alternates() {
        let mut g = PracticeGate::default();
        assert_eq!(g.conclude(), Err(GateError::NoProblem));
        assert_eq!(g.problem(), Ok(()));
        assert_eq!(g.problem(), Err(GateError::ProblemActive));
        assert_eq!(g.conclude(), Ok(()));
        assert_eq!(g.problem(), Ok(()));
    }

    #[test]
    fn practice_contracts_parse() {
        let p: ProblemInput = serde_json::from_value(json!({
            "statement": "s", "mode": "discrimination", "modelSolution": "m"
        }))
        .unwrap();
        assert!(problem_input_problems(&p).is_empty());
        let mut bad = p.clone();
        bad.model_solution = " ".into();
        assert_eq!(problem_input_problems(&bad).len(), 1);
        let c: ConcludeInput = serde_json::from_value(json!({
            "outcome": "solvedWithHints", "verdict": "v", "rung": "R2", "missClass": "cue", "retest": true
        }))
        .unwrap();
        assert_eq!(
            (c.outcome, c.rung, c.miss_class),
            (
                Outcome::SolvedWithHints,
                Some(Rung::R2),
                Some(MissClass::Cue)
            )
        );
        assert!(serde_json::from_value::<ConcludeInput>(
            json!({"outcome":"bogus","verdict":"v","rung":null,"missClass":null,"retest":false})
        )
        .is_err());
    }
}
