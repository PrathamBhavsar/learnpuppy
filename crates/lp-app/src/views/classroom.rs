use std::sync::Arc;

use dioxus::prelude::*;
use lp_agent::loops::{Kind, Session};
use lp_core::plan::{
    derive_plan_statuses, heal_encoded_arguments, parse_plan, phase_at_3, CorrectAnswer, Phase,
    PlanNode, PlanStatuses, PlanTree, QuizInput, QuizOption, ToolCall, ToolOutcome,
};
use lp_core::ports::{DeliveryStatus, JobKind, JobStatus};
use lp_ui::code_block::CodeBlock;
use lp_ui::icons::{
    IconBook, IconBrain, IconCheck, IconClassroom, IconCode, IconPlay, IconPlus, IconRefresh,
    IconSparkles, IconTerminal, IconX, IconZoomIn, IconZoomOut,
};
use lp_ui::overlays::{ConfidenceSlider, ScrollArea, SplitDirection, SplitPane};
use lp_ui::primitives::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card, CardContent, CardDescription,
    CardHeader, CardTitle, Progress, Separator,
};
use lp_ui::tabs::{Tabs, TabsContent, TabsList, TabsTrigger};

use crate::backend::{spawn_turn, BackendHandle, BackendResult, FauxBackend};
use crate::state::{AppState, View};

/// Filter tabs for the Left Session Drawer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SessionFilter {
    #[default]
    All,
    Teaching,
    Practice,
}

impl SessionFilter {
    pub const ALL: [Self; 3] = [Self::All, Self::Teaching, Self::Practice];

    pub const fn id(self) -> &'static str {
        match self {
            Self::All => "all",
            Self::Teaching => "teaching",
            Self::Practice => "practice",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::All => "All",
            Self::Teaching => "Teaching",
            Self::Practice => "Practice",
        }
    }

    pub const fn matches(self, kind: Kind) -> bool {
        match self {
            Self::All => true,
            Self::Teaching => matches!(kind, Kind::Teach),
            Self::Practice => matches!(kind, Kind::Practice),
        }
    }
}

/// Derived status of a single node in the Plan DAG.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanNodeStatus {
    Current,
    Visited,
    Upcoming,
}

impl PlanNodeStatus {
    pub fn from_statuses(statuses: &PlanStatuses, node_id: &str) -> Self {
        if statuses.current.as_deref() == Some(node_id) {
            Self::Current
        } else if statuses.visited.iter().any(|v| v == node_id) {
            Self::Visited
        } else {
            Self::Upcoming
        }
    }

    pub const fn slug(self) -> &'static str {
        match self {
            Self::Current => "current",
            Self::Visited => "visited",
            Self::Upcoming => "upcoming",
        }
    }

    pub const fn badge_label(self) -> &'static str {
        match self {
            Self::Current => "● Current",
            Self::Visited => "✓ Visited",
            Self::Upcoming => "○ Upcoming",
        }
    }

    pub const fn stroke_color(self) -> &'static str {
        match self {
            Self::Current => "var(--info)",
            Self::Visited => "var(--mastery-mastered)",
            Self::Upcoming => "var(--muted-foreground)",
        }
    }

    pub const fn badge_variant(self) -> BadgeVariant {
        match self {
            Self::Current => BadgeVariant::Phase,
            Self::Visited => BadgeVariant::Success,
            Self::Upcoming => BadgeVariant::Secondary,
        }
    }
}

/// Summary entry rendered in the Left Session Drawer.
#[derive(Clone, Debug, PartialEq)]
pub struct ClassroomSessionEntry {
    pub id: String,
    pub title: String,
    pub kind: Kind,
    pub outcomes: Vec<ToolOutcome>,
}

impl ClassroomSessionEntry {
    pub fn phase_label(&self) -> &'static str {
        match self.kind {
            Kind::Practice => "PRACTICE",
            Kind::Teach => match phase_at_3(&self.outcomes) {
                Phase::Probe => "PROBE",
                Phase::Plan => "PLAN",
                Phase::Teach => "TEACH",
            },
        }
    }
}

/// Visual state for one of the four async workbench sub-agents (`research`, `pdf`, `visual`, `demo`).
#[derive(Clone, Debug, PartialEq)]
pub struct WorkbenchCardState {
    pub id: String,
    pub slug: &'static str,
    pub title: &'static str,
    pub kind: JobKind,
    pub status: JobStatus,
    pub delivery: DeliveryStatus,
    pub summary: String,
    pub artifact: Option<String>,
}

impl WorkbenchCardState {
    pub const fn status_label(&self) -> &'static str {
        match self.status {
            JobStatus::Running => "Running",
            JobStatus::Done => "Done",
            JobStatus::Failed => "Failed",
        }
    }

    pub const fn status_variant(&self) -> BadgeVariant {
        match self.status {
            JobStatus::Running => BadgeVariant::Warning,
            JobStatus::Done => BadgeVariant::Success,
            JobStatus::Failed => BadgeVariant::Destructive,
        }
    }

    pub const fn delivery_label(&self) -> &'static str {
        match self.delivery {
            DeliveryStatus::Pending => "Steer pending",
            DeliveryStatus::Claimed => "Steer delivered",
        }
    }
}

/// Graded verdict after the learner submits an answer in the Question Dock.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GradedVerdict {
    pub selected_key: String,
    pub selected_value: String,
    pub selected_label: String,
    pub correct_value: String,
    pub correct_label: String,
    pub is_correct: bool,
    pub is_idk: bool,
    pub confidence: u8,
    pub reading_notes: String,
    pub reasoning: String,
    pub explanation: String,
    pub envelope: String,
}

impl GradedVerdict {
    pub const fn status_slug(&self) -> &'static str {
        if self.is_idk {
            "idk"
        } else if self.is_correct {
            "correct"
        } else {
            "incorrect"
        }
    }

    pub const fn headline(&self) -> &'static str {
        if self.is_idk {
            "Marked 'I don't know' — explanation unlocked"
        } else if self.is_correct {
            "Correct — proposition verified"
        } else {
            "Incorrect — review the derivation below"
        }
    }
}

const DEFAULT_MERMAID_PLAN: &str = r#"graph TD
    wal-basics["WAL Fundamentals — Append-only redo frames and checkpoint invariants"]
    snapshot-isolation["Snapshot Isolation — Readers pin max_frame without blocking writers"]
    fts5-sync["FTS5 Index Sync — Keeping propositions_fts consistent with JSONL ledgers"]
    dag-derivation["Position From Calls — Deriving Current, Visited, and Upcoming from tool calls"]
    wal-basics --> snapshot-isolation
    wal-basics --> fts5-sync
    snapshot-isolation & fts5-sync --> dag-derivation
"#;

const LESSON_RUST_SAMPLE: &str = r#"pub fn derive_plan_statuses(tree: &PlanTree, calls: &[ToolCall]) -> PlanStatuses {
    let mut runs: Vec<&str> = Vec::new();
    for c in calls.iter().filter(|c| matches!(c.name.as_str(), "quiz" | "explain")) {
        if let Some(id) = c.args.get("nodeId").and_then(Value::as_str) {
            if tree.has(id) && runs.last() != Some(&id) {
                runs.push(id);
            }
        }
    }
    PlanStatuses::from_runs(tree, &runs)
}"#;

fn default_sessions() -> Vec<ClassroomSessionEntry> {
    vec![
        ClassroomSessionEntry {
            id: "sess-wal-teach".to_string(),
            title: "SQLite WAL & Position-From-Calls".to_string(),
            kind: Kind::Teach,
            outcomes: vec![
                ToolOutcome {
                    name: "probe_done".to_string(),
                    succeeded: true,
                },
                ToolOutcome {
                    name: "plan_init".to_string(),
                    succeeded: true,
                },
            ],
        },
        ClassroomSessionEntry {
            id: "sess-probe-raft".to_string(),
            title: "Consensus Quorum Probing".to_string(),
            kind: Kind::Teach,
            outcomes: vec![],
        },
        ClassroomSessionEntry {
            id: "sess-plan-fsrs".to_string(),
            title: "FSRS v5 Stability Planning".to_string(),
            kind: Kind::Teach,
            outcomes: vec![ToolOutcome {
                name: "probe_done".to_string(),
                succeeded: true,
            }],
        },
        ClassroomSessionEntry {
            id: "sess-practice-drill".to_string(),
            title: "Adaptive Discrimination Drill".to_string(),
            kind: Kind::Practice,
            outcomes: vec![],
        },
    ]
}

fn node_call(name: &str, node_id: &str, kind: &str) -> ToolCall {
    ToolCall {
        name: name.to_string(),
        args: heal_encoded_arguments(format!(r#"{{"nodeId":"{node_id}","type":"{kind}"}}"#).into()),
    }
}

fn default_tool_calls() -> Vec<ToolCall> {
    vec![
        node_call("explain", "wal-basics", "check"),
        node_call("quiz", "snapshot-isolation", "predict"),
    ]
}

fn default_quiz_input() -> QuizInput {
    QuizInput {
        question: "Why does LearnPuppy derive PlanTree node statuses (Current, Visited, Upcoming) from assistant `quiz`/`explain` tool calls rather than storing `currentNodeId` inside `PlanTree`?".to_string(),
        options: vec![
            QuizOption {
                label: "Because `PlanTree` only represents DAG structure, while transcript tool calls are the single append-only source of truth for teaching progress.".to_string(),
                value: "opt_calls_truth".to_string(),
            },
            QuizOption {
                label: "Because Mermaid `graph TD` syntax forbids node IDs with hyphens unless statuses are stored in SQLite.".to_string(),
                value: "opt_mermaid_limit".to_string(),
            },
            QuizOption {
                label: "Because `ProberAgent` mutates `PlanTree` directly during the initial `probe` phase before `probe_done`.".to_string(),
                value: "opt_prober_mutates".to_string(),
            },
            QuizOption {
                label: "Because `derive_plan_statuses` randomly shuffles visited nodes using Fisher-Yates on every render.".to_string(),
                value: "opt_random_shuffle".to_string(),
            },
        ],
        correct_answer: CorrectAnswer::One("opt_calls_truth".to_string()),
        explanation: "`PlanTree` is a pure structural DAG parsed from `plan_init`. Deriving `Current`, `Visited`, and `Upcoming` via `derive_plan_statuses` over `quiz` and `explain` calls guarantees replay determinism and prevents state drift between the plan and transcript.".to_string(),
        multi_select: false,
        kind: Some("predict".to_string()),
        node_id: Some("snapshot-isolation".to_string()),
    }
}

fn default_workbench_cards() -> Vec<WorkbenchCardState> {
    vec![
        WorkbenchCardState {
            id: "job_sess-wal-teach_research_1".to_string(),
            slug: "research",
            title: "researcher",
            kind: JobKind::Research,
            status: JobStatus::Done,
            delivery: DeliveryStatus::Claimed,
            summary: "Extracted 3 SQLite WAL concurrency citations and reader-frame invariants."
                .to_string(),
            artifact: Some("~/.learnpuppy/artifacts/job_research_1.json".to_string()),
        },
        WorkbenchCardState {
            id: "job_sess-wal-teach_pdf_1".to_string(),
            slug: "pdf",
            title: "pdf-reader",
            kind: JobKind::Pdf,
            status: JobStatus::Done,
            delivery: DeliveryStatus::Pending,
            summary: "Pages read: [14, 15] · Consulted: [12, 18] — B-tree page cache & WAL index."
                .to_string(),
            artifact: Some("~/.learnpuppy/artifacts/job_pdf_1.json".to_string()),
        },
        WorkbenchCardState {
            id: "job_sess-wal-teach_visual_1".to_string(),
            slug: "visual",
            title: "visual-maker",
            kind: JobKind::Visual,
            status: JobStatus::Done,
            delivery: DeliveryStatus::Claimed,
            summary: "Generated verified SVG WAL frame timeline + Unicode Braille fallback."
                .to_string(),
            artifact: Some("~/.learnpuppy/artifacts/job_visual_1.svg".to_string()),
        },
        WorkbenchCardState {
            id: "job_sess-wal-teach_demo_1".to_string(),
            slug: "demo",
            title: "demo-builder",
            kind: JobKind::Demo,
            status: JobStatus::Running,
            delivery: DeliveryStatus::Pending,
            summary: "Building interactive checkpoint backpressure curve explorer (TuiDemoSpec)."
                .to_string(),
            artifact: Some("~/.learnpuppy/artifacts/job_demo_1.html".to_string()),
        },
    ]
}

/// Computes deterministic `(x, y)` coordinates in SVG canvas space for each `PlanNode`.
pub fn compute_dag_coordinates(tree: &PlanTree) -> Vec<(PlanNode, f64, f64)> {
    // Compute topological depth for each node so edges flow cleanly top-to-bottom.
    let mut depths: Vec<usize> = vec![0; tree.nodes.len()];
    for _ in 0..tree.nodes.len() {
        for (from_id, to_id) in &tree.edges {
            let from_idx = tree.nodes.iter().position(|n| &n.id == from_id);
            let to_idx = tree.nodes.iter().position(|n| &n.id == to_id);
            if let (Some(fi), Some(ti)) = (from_idx, to_idx) {
                if depths[ti] <= depths[fi] {
                    depths[ti] = depths[fi] + 1;
                }
            }
        }
    }

    let max_depth = depths.iter().copied().max().unwrap_or(0);
    let mut positioned = Vec::with_capacity(tree.nodes.len());

    for depth in 0..=max_depth {
        let layer_indices: Vec<usize> = depths
            .iter()
            .enumerate()
            .filter_map(|(idx, &d)| if d == depth { Some(idx) } else { None })
            .collect();
        let count = layer_indices.len().max(1);
        for (rank, node_idx) in layer_indices.into_iter().enumerate() {
            let x = if count == 1 {
                260.0
            } else {
                let span = 320.0;
                let start = 260.0 - span / 2.0;
                start + (rank as f64) * (span / (count.saturating_sub(1) as f64))
            };
            let y = 70.0 + (depth as f64) * 115.0;
            positioned.push((tree.nodes[node_idx].clone(), x, y));
        }
    }
    positioned
}

/// Grades a learner's selected option (`A`..`D` or `I` for `"__idk__"`) and builds a [`GradedVerdict`].
pub fn evaluate_question_submission(
    quiz: &QuizInput,
    selected_key: &str,
    confidence: u8,
    reading_notes: &str,
    reasoning: &str,
) -> GradedVerdict {
    let option_keys = ["A", "B", "C", "D"];
    let is_idk = selected_key.eq_ignore_ascii_case("I") || selected_key == "__idk__";

    let (selected_value, selected_label) = if is_idk {
        ("__idk__".to_string(), "I don't know".to_string())
    } else {
        let idx = option_keys
            .iter()
            .position(|k| k.eq_ignore_ascii_case(selected_key))
            .unwrap_or(0);
        quiz.options
            .get(idx)
            .map(|o| (o.value.clone(), o.label.clone()))
            .unwrap_or_else(|| (selected_key.to_string(), selected_key.to_string()))
    };

    let correct_values = quiz.correct_answer.values();
    let correct_value = correct_values.first().cloned().unwrap_or_default();
    let correct_label = quiz
        .options
        .iter()
        .find(|o| o.value == correct_value)
        .map(|o| o.label.clone())
        .unwrap_or_else(|| correct_value.clone());

    let is_correct = !is_idk && correct_values.iter().any(|c| c == &selected_value);
    let clamped_conf = confidence.clamp(1, 5);
    let envelope = format!(
        "selected: {}; confidence: {}; reading_thinking: {}; thinking: {}",
        selected_value,
        clamped_conf,
        if reading_notes.trim().is_empty() {
            "none"
        } else {
            reading_notes.trim()
        },
        if reasoning.trim().is_empty() {
            "none"
        } else {
            reasoning.trim()
        }
    );

    GradedVerdict {
        selected_key: if is_idk {
            "I".to_string()
        } else {
            selected_key.to_uppercase()
        },
        selected_value,
        selected_label,
        correct_value,
        correct_label,
        is_correct,
        is_idk,
        confidence: clamped_conf,
        reading_notes: reading_notes.trim().to_string(),
        reasoning: reasoning.trim().to_string(),
        explanation: quiz.explanation.clone(),
        envelope,
    }
}

/// Visual Classroom & Plan DAG Split View (`ClassroomView`).
#[component]
pub fn ClassroomView() -> Element {
    let state = use_context::<AppState>();
    let backend = try_use_context::<BackendHandle>()
        .unwrap_or_else(|| Arc::new(FauxBackend::default()) as BackendHandle);
    let view = (state.current_view)();

    // 1. Left Drawer Signals: Sessions & Sub-Agent Workbench
    let mut session_filter = use_signal(|| SessionFilter::All);
    let mut sessions = use_signal(default_sessions);
    let mut active_session_id = use_signal(|| "sess-wal-teach".to_string());
    let mut workbench_cards = use_signal(default_workbench_cards);
    let mut wake_notice = use_signal(|| Option::<String>::None);

    // 2. Center Lesson Stream & Async Turn Signals
    let mut prompt_draft =
        use_signal(|| "Explain how WAL readers avoid blocking writers.".to_string());
    let turn_stream_text = use_signal(String::new);
    let turn_result = use_signal(|| Option::<BackendResult<Session>>::None);
    let mut copied_snippet = use_signal(|| false);

    // 3. Question Dock Signals ([A]..[D], [I], 1..5 Confidence, Dual Thinking Tabs, Graded Verdict)
    let quiz_signal = use_signal(default_quiz_input);
    let mut selected_option = use_signal(|| "A".to_string());
    let confidence = use_signal(|| 4u8);
    let thinking_tab = use_signal(|| "notes".to_string());
    let mut reading_notes = use_signal(|| {
        "PlanTree stores only nodes and edges; tool calls carry dynamic progress.".to_string()
    });
    let mut answer_reasoning = use_signal(|| {
        "Option A matches derive_plan_statuses; B, C, and D contradict lp-core invariants."
            .to_string()
    });
    let mut graded_verdict = use_signal(|| {
        Some(evaluate_question_submission(
            &default_quiz_input(),
            "A",
            4,
            "PlanTree stores only nodes and edges; tool calls carry dynamic progress.",
            "Option A matches derive_plan_statuses; B, C, and D contradict lp-core invariants.",
        ))
    });

    // 4. Right Plan DAG Viewport Signals
    let mut plan_source = use_signal(|| DEFAULT_MERMAID_PLAN.to_string());
    let mut plan_error = use_signal(|| Option::<String>::None);
    let mut tool_calls = use_signal(default_tool_calls);
    let mut selected_dag_node = use_signal(|| "snapshot-isolation".to_string());
    let mut dag_zoom = use_signal(|| 1.0_f64);
    let mut dag_pan_x = use_signal(|| 0.0_f64);
    let mut dag_pan_y = use_signal(|| 0.0_f64);
    let mut dag_dragging = use_signal(|| false);
    let split_ratio = use_signal(|| if view == View::Plan { 0.42 } else { 0.62 });

    // Derived memos
    let filtered_sessions = use_memo(move || {
        let f = session_filter();
        sessions()
            .into_iter()
            .filter(|s| f.matches(s.kind))
            .collect::<Vec<_>>()
    });

    let active_session = use_memo(move || {
        let current_id = active_session_id();
        sessions()
            .into_iter()
            .find(|s| s.id == current_id)
            .unwrap_or_else(|| default_sessions().into_iter().next().unwrap())
    });

    let parsed_plan = use_memo(move || {
        parse_plan(&plan_source(), 1)
            .unwrap_or_else(|_| parse_plan(DEFAULT_MERMAID_PLAN, 1).expect("default valid plan"))
    });

    let plan_statuses = use_memo(move || derive_plan_statuses(&parsed_plan(), &tool_calls()));

    let running_jobs_count = use_memo(move || {
        workbench_cards()
            .iter()
            .filter(|c| c.status == JobStatus::Running)
            .count()
    });

    let active_sess = active_session();
    let current_phase_label = active_sess.phase_label();
    let tree = parsed_plan();
    let statuses = plan_statuses();
    let positioned_nodes = compute_dag_coordinates(&tree);
    let visited_pct = if tree.nodes.is_empty() {
        0.0
    } else {
        let active_bonus = usize::from(statuses.current.is_some());
        ((statuses.visited.len() + active_bonus) as f64 / tree.nodes.len() as f64) * 100.0
    };

    let quiz = quiz_signal();
    let option_keys = ["A", "B", "C", "D"];

    rsx! {
        section {
            "aria-label": view.label(),
            "data-testid": format!("view-{}", view.id()),
            class: "flex flex-col",
            style: "height: 100%; min-height: 0; gap: 0.75rem;",

            // Top Classroom Header Bar
            div {
                class: "flex items-center justify-between",
                style: "gap: 1rem; padding-bottom: 0.5rem; border-bottom: 1px solid var(--border); flex-wrap: wrap;",
                div {
                    class: "flex items-center",
                    style: "gap: 0.75rem;",
                    IconClassroom { size: 22 }
                    h1 {
                        style: "margin: 0; font-size: 1.25rem; font-weight: 700;",
                        "{view.label()}"
                    }
                    Badge {
                        variant: BadgeVariant::Phase,
                        test_id: "classroom-phase-badge",
                        aria_label: format!("Current session phase: {current_phase_label}"),
                        "{current_phase_label}"
                    }
                    p {
                        role: "status",
                        "data-testid": "classroom-action",
                        class: "text-muted-foreground",
                        style: "margin: 0; font-size: 0.8125rem;",
                        "{(state.quick_action)().label()} session"
                    }
                }

                // Phase & Turn Quick Controls
                div {
                    class: "flex items-center",
                    style: "gap: 0.5rem; flex-wrap: wrap;",
                    Button {
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::Sm,
                        test_id: "phase-step-probe-done",
                        aria_label: "Record probe_done outcome",
                        onclick: move |_| {
                            let target_id = active_session_id();
                            let mut list = sessions();
                            if let Some(entry) = list.iter_mut().find(|s| s.id == target_id) {
                                if !entry.outcomes.iter().any(|o| o.name == "probe_done" && o.succeeded) {
                                    entry.outcomes.push(ToolOutcome {
                                        name: "probe_done".to_string(),
                                        succeeded: true,
                                    });
                                }
                            }
                            sessions.set(list);
                        },
                        "probe_done"
                    }
                    Button {
                        variant: ButtonVariant::Outline,
                        size: ButtonSize::Sm,
                        test_id: "phase-step-plan-init",
                        aria_label: "Record plan_init outcome",
                        onclick: move |_| {
                            let target_id = active_session_id();
                            let mut list = sessions();
                            if let Some(entry) = list.iter_mut().find(|s| s.id == target_id) {
                                if !entry.outcomes.iter().any(|o| o.name == "probe_done" && o.succeeded) {
                                    entry.outcomes.push(ToolOutcome {
                                        name: "probe_done".to_string(),
                                        succeeded: true,
                                    });
                                }
                                if !entry.outcomes.iter().any(|o| o.name == "plan_init" && o.succeeded) {
                                    entry.outcomes.push(ToolOutcome {
                                        name: "plan_init".to_string(),
                                        succeeded: true,
                                    });
                                }
                            }
                            sessions.set(list);
                        },
                        "plan_init"
                    }
                    Button {
                        variant: ButtonVariant::Primary,
                        size: ButtonSize::Sm,
                        test_id: "classroom-run-turn",
                        aria_label: "Run classroom agent turn",
                        onclick: {
                            let backend = backend.clone();
                            move |_| {
                                let sess = Session::new(active_session_id(), active_session().kind);
                                spawn_turn(backend.clone(), sess, turn_stream_text, turn_result);
                            }
                        },
                        IconPlay { size: 14 }
                        "Run Turn"
                    }
                }
            }

            // Main 3-Zone Workspace: Left Session Drawer + SplitPane (Center Lesson Stream & Question Dock | Right Plan DAG)
            div {
                class: "flex overflow-hidden",
                style: "flex: 1; min-height: 0; gap: 0.75rem;",

                // ============================================================
                // ZONE 1: Left Session Drawer & Sub-Agent Workbench Cards
                // ============================================================
                aside {
                    role: "complementary",
                    "aria-label": "Classroom sessions and sub-agent workbench",
                    "data-testid": "session-drawer",
                    class: "flex flex-col bg-card",
                    style: "width: 18.5rem; flex-shrink: 0; border: 1px solid var(--border); border-radius: var(--radius); padding: 0.75rem; gap: 0.75rem; min-height: 0; overflow-y: auto;",

                    div {
                        class: "flex items-center justify-between",
                        strong {
                            style: "font-size: 0.875rem;",
                            "Sessions"
                        }
                        Button {
                            variant: ButtonVariant::Outline,
                            size: ButtonSize::Xs,
                            test_id: "new-session-btn",
                            aria_label: "Create new teaching session",
                            onclick: move |_| {
                                let next_idx = sessions().len() + 1;
                                let new_id = format!("sess-custom-{next_idx}");
                                let mut list = sessions();
                                list.insert(
                                    0,
                                    ClassroomSessionEntry {
                                        id: new_id.clone(),
                                        title: format!("Session #{next_idx}: Probe & Plan"),
                                        kind: Kind::Teach,
                                        outcomes: vec![],
                                    },
                                );
                                sessions.set(list);
                                active_session_id.set(new_id);
                            },
                            IconPlus { size: 12 }
                            "New"
                        }
                    }

                    // Filterable Session Tabs (All | Teaching | Practice)
                    div {
                        role: "tablist",
                        "aria-label": "Filter sessions by kind",
                        "data-testid": "session-filter-tabs",
                        class: "flex items-center bg-muted",
                        style: "gap: 0.25rem; padding: 0.25rem; border-radius: 0.375rem;",
                        for filter in SessionFilter::ALL {
                            Button {
                                key: "{filter.id()}",
                                variant: if session_filter() == filter {
                                    ButtonVariant::Secondary
                                } else {
                                    ButtonVariant::Ghost
                                },
                                size: ButtonSize::Xs,
                                test_id: format!("session-filter-{}", filter.id()),
                                aria_label: format!("Filter {} sessions", filter.label()),
                                "aria-selected": if session_filter() == filter { "true" } else { "false" },
                                onclick: move |_| session_filter.set(filter),
                                "{filter.label()}"
                            }
                        }
                    }

                    // Session List
                    ul {
                        "aria-label": "Classroom session list",
                        "data-testid": "session-list",
                        style: "list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 0.375rem;",
                        for item in filtered_sessions() {
                            {
                                let is_selected = item.id == active_session_id();
                                let phase_str = item.phase_label();
                                let item_id = item.id.clone();
                                rsx! {
                                    li {
                                        key: "{item.id}",
                                        "data-testid": "session-item",
                                        "data-session-id": "{item.id}",
                                        "data-active": if is_selected { "true" } else { "false" },
                                        button {
                                            r#type: "button",
                                            "data-testid": "session-select-{item.id}",
                                            "aria-label": "Select session {item.title}",
                                            "aria-current": if is_selected { "true" } else { "false" },
                                            style: if is_selected {
                                                "width: 100%; text-align: left; padding: 0.5rem 0.625rem; border-radius: 0.375rem; border: 1px solid var(--primary); background: var(--accent); color: var(--foreground); cursor: pointer;"
                                            } else {
                                                "width: 100%; text-align: left; padding: 0.5rem 0.625rem; border-radius: 0.375rem; border: 1px solid var(--border); background: transparent; color: var(--foreground); cursor: pointer;"
                                            },
                                            onclick: move |_| active_session_id.set(item_id.clone()),
                                            div {
                                                class: "flex items-center justify-between",
                                                style: "gap: 0.5rem;",
                                                span {
                                                    style: "font-size: 0.8125rem; font-weight: 600; overflow: hidden; text-overflow: ellipsis; white-space: nowrap;",
                                                    "{item.title}"
                                                }
                                                Badge {
                                                    variant: BadgeVariant::Phase,
                                                    test_id: format!("session-phase-{}", item.id),
                                                    "{phase_str}"
                                                }
                                            }
                                            div {
                                                class: "text-muted-foreground",
                                                style: "font-size: 0.7rem; margin-top: 0.25rem;",
                                                match item.kind {
                                                    Kind::Teach => "Teaching · 3-phase fold",
                                                    Kind::Practice => "Practice · adaptive gate",
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }

                    Separator { test_id: "drawer-workbench-separator" }

                    // Async Workbench Sub-Agent Cards (research, pdf, visual, demo)
                    div {
                        "aria-label": "Async workbench sub-agents",
                        "data-testid": "workbench-section",
                        class: "flex flex-col",
                        style: "gap: 0.5rem;",
                        div {
                            class: "flex items-center justify-between",
                            div {
                                class: "flex items-center",
                                style: "gap: 0.375rem;",
                                IconSparkles { size: 15 }
                                strong {
                                    style: "font-size: 0.8125rem;",
                                    "Sub-Agent Workbench"
                                }
                            }
                            Badge {
                                variant: BadgeVariant::Outline,
                                test_id: "workbench-semaphore-badge",
                                aria_label: "Background LLM concurrency semaphore permits",
                                "{running_jobs_count()}/3 permits"
                            }
                        }

                        for card in workbench_cards() {
                            {
                                let slug = card.slug;
                                let is_running = card.status == JobStatus::Running;
                                rsx! {
                                    Card {
                                        key: "{card.id}",
                                        test_id: format!("workbench-card-{slug}"),
                                        aria_label: format!("Sub-agent {} workbench card", card.title),
                                        style: "padding: 0.625rem;",
                                        div {
                                            class: "flex items-center justify-between",
                                            style: "gap: 0.5rem;",
                                            strong {
                                                style: "font-size: 0.78rem; font-family: var(--font-mono);",
                                                "{card.title}"
                                            }
                                            Badge {
                                                variant: card.status_variant(),
                                                test_id: format!("workbench-status-{slug}"),
                                                "{card.status_label()}"
                                            }
                                        }
                                        p {
                                            class: "text-muted-foreground",
                                            style: "font-size: 0.72rem; margin: 0.35rem 0;",
                                            "{card.summary}"
                                        }
                                        if let Some(artifact) = &card.artifact {
                                            div {
                                                "data-testid": "workbench-artifact-{slug}",
                                                style: "font-size: 0.68rem; font-family: var(--font-mono); color: var(--info); overflow-wrap: anywhere;",
                                                "{artifact}"
                                            }
                                        }
                                        div {
                                            class: "flex items-center justify-between",
                                            style: "margin-top: 0.375rem; gap: 0.375rem;",
                                            span {
                                                "data-testid": "workbench-delivery-{slug}",
                                                class: "text-muted-foreground",
                                                style: "font-size: 0.68rem;",
                                                "{card.delivery_label()}"
                                            }
                                            div {
                                                class: "flex items-center",
                                                style: "gap: 0.25rem;",
                                                if is_running {
                                                    Button {
                                                        variant: ButtonVariant::Destructive,
                                                        size: ButtonSize::Xs,
                                                        test_id: format!("workbench-abort-{slug}"),
                                                        aria_label: format!("Abort {} sub-agent job", card.title),
                                                        onclick: move |_| {
                                                            let mut cards = workbench_cards();
                                                            if let Some(c) = cards.iter_mut().find(|c| c.slug == slug) {
                                                                c.status = JobStatus::Failed;
                                                                c.delivery = DeliveryStatus::Claimed;
                                                            }
                                                            workbench_cards.set(cards);
                                                        },
                                                        "Abort"
                                                    }
                                                } else {
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Xs,
                                                        test_id: format!("workbench-dispatch-{slug}"),
                                                        aria_label: format!("Dispatch {} sub-agent job", card.title),
                                                        onclick: move |_| {
                                                            let mut cards = workbench_cards();
                                                            if let Some(c) = cards.iter_mut().find(|c| c.slug == slug) {
                                                                c.status = JobStatus::Running;
                                                                c.delivery = DeliveryStatus::Pending;
                                                            }
                                                            workbench_cards.set(cards);
                                                        },
                                                        "Dispatch"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        Button {
                            variant: ButtonVariant::Secondary,
                            size: ButtonSize::Sm,
                            test_id: "workbench-wake-btn",
                            aria_label: "Claim finished sub-agent steers via Wake Pump",
                            onclick: move |_| {
                                let mut cards = workbench_cards();
                                let mut claimed = 0usize;
                                for c in &mut cards {
                                    if c.status == JobStatus::Running {
                                        c.status = JobStatus::Done;
                                    }
                                    if c.status == JobStatus::Done && c.delivery == DeliveryStatus::Pending {
                                        c.delivery = DeliveryStatus::Claimed;
                                        claimed += 1;
                                    }
                                }
                                workbench_cards.set(cards);
                                wake_notice.set(Some(format!(
                                    "Wake pump claimed {claimed} pending steer(s) into transcript."
                                )));
                            },
                            IconRefresh { size: 14 }
                            "Pump Pending Steers"
                        }

                        if let Some(notice) = wake_notice() {
                            p {
                                role: "status",
                                "data-testid": "workbench-wake-notice",
                                style: "margin: 0; font-size: 0.72rem; color: var(--success);",
                                "{notice}"
                            }
                        }
                    }
                }

                // ============================================================
                // ZONES 2 & 3: Resizable SplitPane (Center Lesson + Question Dock | Right Plan DAG)
                // ============================================================
                div {
                    style: "flex: 1; min-width: 0; min-height: 0;",
                    SplitPane {
                        direction: SplitDirection::Horizontal,
                        ratio_signal: split_ratio,
                        test_id: "classroom-split-pane",
                        aria_label: "Resize lesson stream and Plan DAG viewport",
                        first: rsx! {
                            ScrollArea {
                                aria_label: "Lesson stream and question dock",
                                test_id: "lesson-scroll",
                                max_height: "100%",
                                class: "h-full",
                                div {
                                    "data-testid": "lesson-stream",
                                    class: "flex flex-col",
                                    style: "padding: 0.5rem 0.75rem 1.25rem 0.5rem; gap: 1rem;",

                                    // Streaming turn status / error banner from Backend::turn
                                    if !turn_stream_text().is_empty() {
                                        div {
                                            role: "status",
                                            "data-testid": "turn-stream-output",
                                            style: "padding: 0.625rem 0.875rem; border-radius: var(--radius); border: 1px solid var(--info); background: var(--secondary); font-size: 0.85rem;",
                                            strong { "Assistant Stream: " }
                                            span { "{turn_stream_text()}" }
                                        }
                                    }
                                    if let Some(Err(err)) = turn_result() {
                                        div {
                                            role: "alert",
                                            "data-testid": "turn-error",
                                            style: "padding: 0.625rem 0.875rem; border-radius: var(--radius); border: 1px solid var(--destructive); background: var(--card); color: var(--destructive); font-size: 0.85rem;",
                                            "Turn error: {err}"
                                        }
                                    }

                                    // 1. Markdown & Table Lesson Block
                                    Card {
                                        test_id: "lesson-markdown-card",
                                        aria_label: "Lesson Markdown explanation",
                                        CardHeader {
                                            div {
                                                class: "flex items-center justify-between",
                                                style: "gap: 0.5rem;",
                                                div {
                                                    class: "flex items-center",
                                                    style: "gap: 0.5rem;",
                                                    IconBook { size: 18 }
                                                    CardTitle {
                                                        "Node: Snapshot Isolation & Position-From-Calls"
                                                    }
                                                }
                                                Badge {
                                                    variant: BadgeVariant::Secondary,
                                                    test_id: "lesson-active-node-badge",
                                                    "nodeId: {selected_dag_node()}"
                                                }
                                            }
                                            CardDescription {
                                                "Stateless per-turn TeacherAgent lesson block with Markdown, comparison table, code, and SVG diagram."
                                            }
                                        }
                                        CardContent {
                                            div {
                                                "data-testid": "lesson-markdown",
                                                style: "display: flex; flex-direction: column; gap: 0.75rem; font-size: 0.875rem; line-height: 1.55;",
                                                p {
                                                    style: "margin: 0;",
                                                    "In LearnPuppy, neither ProberAgent nor TeacherAgent persists mutable state in RAM across turns. Every session phase (`PROBE`, `PLAN`, `TEACH`) and every Plan DAG node status (`Current`, `Visited`, `Upcoming`) is folded deterministically from the append-only transcript."
                                                }
                                                table {
                                                    "data-testid": "lesson-table",
                                                    "aria-label": "Phase and tool invariants table",
                                                    style: "width: 100%; border-collapse: collapse; font-size: 0.8rem;",
                                                    thead {
                                                        tr {
                                                            style: "border-bottom: 1px solid var(--border); text-align: left;",
                                                            th { style: "padding: 0.375rem 0.5rem;", "Phase" }
                                                            th { style: "padding: 0.375rem 0.5rem;", "Question Tools" }
                                                            th { style: "padding: 0.375rem 0.5rem;", "Transition Gate" }
                                                        }
                                                    }
                                                    tbody {
                                                        tr {
                                                            style: "border-bottom: 1px solid var(--border);",
                                                            td { style: "padding: 0.375rem 0.5rem; font-family: var(--font-mono);", "Probe" }
                                                            td { style: "padding: 0.375rem 0.5rem; font-family: var(--font-mono);", "probe, recall" }
                                                            td { style: "padding: 0.375rem 0.5rem;", "probe_done (no open question)" }
                                                        }
                                                        tr {
                                                            style: "border-bottom: 1px solid var(--border);",
                                                            td { style: "padding: 0.375rem 0.5rem; font-family: var(--font-mono);", "Plan" }
                                                            td { style: "padding: 0.375rem 0.5rem; font-family: var(--font-mono);", "ask_user_question" }
                                                            td { style: "padding: 0.375rem 0.5rem;", "plan_init (acyclic Mermaid TD)" }
                                                        }
                                                        tr {
                                                            td { style: "padding: 0.375rem 0.5rem; font-family: var(--font-mono);", "Teach" }
                                                            td { style: "padding: 0.375rem 0.5rem; font-family: var(--font-mono);", "quiz, explain" }
                                                            td { style: "padding: 0.375rem 0.5rem;", "derive_plan_statuses over calls" }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // 2. CodeBlock Lesson Section
                                    div {
                                        "data-testid": "lesson-code-section",
                                        CodeBlock {
                                            code: LESSON_RUST_SAMPLE,
                                            language: "rust",
                                            title: "crates/lp-core/src/plan.rs",
                                            highlight_lines: vec![3, 4, 5],
                                            test_id: "lesson-code-block",
                                            on_copy: move |_| copied_snippet.set(true),
                                        }
                                        if copied_snippet() {
                                            span {
                                                role: "status",
                                                "data-testid": "lesson-code-copied-status",
                                                class: "text-muted-foreground",
                                                style: "font-size: 0.72rem;",
                                                "Snippet copied to session scratchpad."
                                            }
                                        }
                                    }

                                    // 3. Visual Diagram Block (Inline SVG + visual-maker steer output)
                                    Card {
                                        test_id: "lesson-diagram-card",
                                        aria_label: "Visual diagram block",
                                        CardHeader {
                                            div {
                                                class: "flex items-center justify-between",
                                                div {
                                                    class: "flex items-center",
                                                    style: "gap: 0.5rem;",
                                                    IconCode { size: 16 }
                                                    CardTitle { "Visual Diagram · WAL Snapshot & Call Fold" }
                                                }
                                                Badge {
                                                    variant: BadgeVariant::Outline,
                                                    test_id: "diagram-subagent-badge",
                                                    "visual-maker SVG"
                                                }
                                            }
                                        }
                                        CardContent {
                                            div {
                                                "data-testid": "lesson-diagram",
                                                style: "display: flex; flex-direction: column; gap: 0.5rem;",
                                                svg {
                                                    xmlns: "http://www.w3.org/2000/svg",
                                                    view_box: "0 0 520 96",
                                                    width: "100%",
                                                    height: "96",
                                                    role: "img",
                                                    "aria-label": "WAL frames to PlanStatuses pipeline diagram",
                                                    "data-testid": "lesson-diagram-svg",
                                                    rect { x: "10", y: "18", width: "135", height: "60", rx: "8", fill: "var(--secondary)", stroke: "var(--info)", stroke_width: "1.5" }
                                                    text { x: "77", y: "45", text_anchor: "middle", fill: "var(--foreground)", font_size: "11", font_weight: "600", "Transcript Calls" }
                                                    text { x: "77", y: "63", text_anchor: "middle", fill: "var(--muted-foreground)", font_size: "10", "quiz / explain" }

                                                    path { d: "M 145 48 C 165 48, 175 48, 195 48", stroke: "var(--primary)", stroke_width: "2", fill: "none" }

                                                    rect { x: "195", y: "18", width: "135", height: "60", rx: "8", fill: "var(--secondary)", stroke: "var(--primary)", stroke_width: "1.5" }
                                                    text { x: "262", y: "45", text_anchor: "middle", fill: "var(--foreground)", font_size: "11", font_weight: "600", "Collapse Runs" }
                                                    text { x: "262", y: "63", text_anchor: "middle", fill: "var(--muted-foreground)", font_size: "10", "dedup consecutive" }

                                                    path { d: "M 330 48 C 350 48, 360 48, 380 48", stroke: "var(--success)", stroke_width: "2", fill: "none" }

                                                    rect { x: "380", y: "18", width: "130", height: "60", rx: "8", fill: "var(--secondary)", stroke: "var(--success)", stroke_width: "1.5" }
                                                    text { x: "445", y: "45", text_anchor: "middle", fill: "var(--foreground)", font_size: "11", font_weight: "600", "PlanStatuses" }
                                                    text { x: "445", y: "63", text_anchor: "middle", fill: "var(--muted-foreground)", font_size: "10", "Current · Visited" }
                                                }
                                            }
                                        }
                                    }

                                    // ============================================================
                                    // 4. Interactive Question Dock ([A]..[D], [I], 1..5 Confidence, Dual Thinking Tabs, Graded Verdict)
                                    // ============================================================
                                    Card {
                                        test_id: "question-dock",
                                        aria_label: "Interactive Question Dock",
                                        CardHeader {
                                            div {
                                                class: "flex items-center justify-between",
                                                style: "gap: 0.5rem; flex-wrap: wrap;",
                                                div {
                                                    class: "flex items-center",
                                                    style: "gap: 0.5rem;",
                                                    IconBrain { size: 18 }
                                                    CardTitle { "Question Dock" }
                                                }
                                                div {
                                                    class: "flex items-center",
                                                    style: "gap: 0.375rem;",
                                                    if let Some(kind) = &quiz.kind {
                                                        Badge {
                                                            variant: BadgeVariant::Phase,
                                                            test_id: "question-kind-badge",
                                                            "{kind}"
                                                        }
                                                    }
                                                    if let Some(node_id) = &quiz.node_id {
                                                        Badge {
                                                            variant: BadgeVariant::Outline,
                                                            test_id: "question-node-badge",
                                                            "{node_id}"
                                                        }
                                                    }
                                                }
                                            }
                                            CardDescription {
                                                test_id: "question-prompt-text",
                                                "{quiz.question}"
                                            }
                                        }
                                        CardContent {
                                            form {
                                                "data-testid": "question-form",
                                                "aria-label": "Answer submission form",
                                                style: "display: flex; flex-direction: column; gap: 1rem;",
                                                onsubmit: move |evt| {
                                                    evt.prevent_default();
                                                    let verdict = evaluate_question_submission(
                                                        &quiz_signal(),
                                                        &selected_option(),
                                                        confidence(),
                                                        &reading_notes(),
                                                        &answer_reasoning(),
                                                    );
                                                    graded_verdict.set(Some(verdict));
                                                },

                                                // Options [A]..[D] + [I] I don't know
                                                div {
                                                    role: "radiogroup",
                                                    "aria-label": "Question answer options",
                                                    "data-testid": "question-options",
                                                    style: "display: flex; flex-direction: column; gap: 0.5rem;",
                                                    for (idx, key_letter) in option_keys.iter().enumerate() {
                                                        if let Some(opt) = quiz.options.get(idx) {
                                                            {
                                                                let key_str = (*key_letter).to_string();
                                                                let is_chosen = selected_option() == key_str;
                                                                let opt_label = opt.label.clone();
                                                                let opt_val = opt.value.clone();
                                                                rsx! {
                                                                    button {
                                                                        key: "{key_str}",
                                                                        r#type: "button",
                                                                        role: "radio",
                                                                        "aria-checked": if is_chosen { "true" } else { "false" },
                                                                        "aria-label": "Option [{key_str}]: {opt_label}",
                                                                        "data-testid": "option-{key_str}",
                                                                        "data-option-key": "{key_str}",
                                                                        "data-option-value": "{opt_val}",
                                                                        style: if is_chosen {
                                                                            "display: flex; align-items: flex-start; gap: 0.625rem; width: 100%; text-align: left; padding: 0.625rem 0.75rem; border-radius: var(--radius); border: 1.5px solid var(--primary); background: var(--accent); color: var(--foreground); cursor: pointer;"
                                                                        } else {
                                                                            "display: flex; align-items: flex-start; gap: 0.625rem; width: 100%; text-align: left; padding: 0.625rem 0.75rem; border-radius: var(--radius); border: 1px solid var(--border); background: var(--background); color: var(--foreground); cursor: pointer;"
                                                                        },
                                                                        onclick: move |_| selected_option.set(key_str.clone()),
                                                                        span {
                                                                            style: "font-family: var(--font-mono); font-weight: 700; font-size: 0.78rem; color: var(--primary); flex-shrink: 0;",
                                                                            "[{key_letter}]"
                                                                        }
                                                                        span {
                                                                            style: "font-size: 0.82rem; line-height: 1.4;",
                                                                            "{opt_label}"
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }

                                                    // [I] I don't know option
                                                    {
                                                        let is_idk_chosen = selected_option() == "I";
                                                        rsx! {
                                                            button {
                                                                r#type: "button",
                                                                role: "radio",
                                                                "aria-checked": if is_idk_chosen { "true" } else { "false" },
                                                                "aria-label": "Option [I]: I don't know",
                                                                "data-testid": "option-I",
                                                                "data-option-key": "I",
                                                                "data-option-value": "__idk__",
                                                                style: if is_idk_chosen {
                                                                    "display: inline-flex; align-items: center; gap: 0.5rem; padding: 0.5rem 0.75rem; border-radius: var(--radius); border: 1.5px solid var(--warning); background: var(--secondary); color: var(--foreground); cursor: pointer;"
                                                                } else {
                                                                    "display: inline-flex; align-items: center; gap: 0.5rem; padding: 0.5rem 0.75rem; border-radius: var(--radius); border: 1px solid var(--border); background: transparent; color: var(--muted-foreground); cursor: pointer;"
                                                                },
                                                                onclick: move |_| selected_option.set("I".to_string()),
                                                                span {
                                                                    style: "font-family: var(--font-mono); font-weight: 700; font-size: 0.78rem;",
                                                                    "[I]"
                                                                }
                                                                span {
                                                                    style: "font-size: 0.8125rem;",
                                                                    "I don't know (reveal explanation without guessing)"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                // 1..5 Confidence Slider from lp-ui
                                                ConfidenceSlider {
                                                    value_signal: confidence,
                                                    test_id: "confidence-slider",
                                                }

                                                // Dual Thinking Tabs (Reading Notes | Answer Reasoning)
                                                Tabs {
                                                    active_signal: thinking_tab,
                                                    default_value: "notes",
                                                    tabs: vec!["notes".to_string(), "reasoning".to_string()],
                                                    test_id: "thinking-tabs",
                                                    TabsList {
                                                        aria_label: "Learner thinking modes",
                                                        test_id: "thinking-tabs-list",
                                                        TabsTrigger {
                                                            value: "notes",
                                                            test_id: "tab-trigger-notes",
                                                            "Reading Notes"
                                                        }
                                                        TabsTrigger {
                                                            value: "reasoning",
                                                            test_id: "tab-trigger-reasoning",
                                                            "Answer Reasoning"
                                                        }
                                                    }
                                                    TabsContent {
                                                        value: "notes",
                                                        force_mount: true,
                                                        test_id: "tab-content-notes",
                                                        textarea {
                                                            "aria-label": "Reading notes while studying the lesson",
                                                            "data-testid": "reading-notes-input",
                                                            rows: "2",
                                                            value: "{reading_notes()}",
                                                            placeholder: "Jot key invariants from the lesson stream…",
                                                            style: "width: 100%; border-radius: var(--radius); border: 1px solid var(--border); background: var(--background); color: var(--foreground); padding: 0.5rem; font-size: 0.8125rem; font-family: var(--font-sans);",
                                                            oninput: move |evt| reading_notes.set(evt.value()),
                                                        }
                                                    }
                                                    TabsContent {
                                                        value: "reasoning",
                                                        force_mount: true,
                                                        test_id: "tab-content-reasoning",
                                                        textarea {
                                                            "aria-label": "Answer reasoning before grading",
                                                            "data-testid": "answer-reasoning-input",
                                                            rows: "2",
                                                            value: "{answer_reasoning()}",
                                                            placeholder: "Why is your chosen option right and why do the distractors fail?",
                                                            style: "width: 100%; border-radius: var(--radius); border: 1px solid var(--border); background: var(--background); color: var(--foreground); padding: 0.5rem; font-size: 0.8125rem; font-family: var(--font-sans);",
                                                            oninput: move |evt| answer_reasoning.set(evt.value()),
                                                        }
                                                    }
                                                }

                                                div {
                                                    class: "flex items-center justify-between",
                                                    style: "gap: 0.75rem; flex-wrap: wrap;",
                                                    span {
                                                        class: "text-muted-foreground",
                                                        style: "font-size: 0.75rem;",
                                                        "Selected: [{selected_option()}] · Confidence: {confidence()}/5"
                                                    }
                                                    Button {
                                                        button_type: "submit",
                                                        variant: ButtonVariant::Primary,
                                                        size: ButtonSize::Default,
                                                        test_id: "submit-answer-btn",
                                                        aria_label: "Submit answer and grade verdict",
                                                        IconCheck { size: 16 }
                                                        "Grade Answer"
                                                    }
                                                }
                                            }

                                            // Instant Graded Verdict Banner
                                            if let Some(verdict) = graded_verdict() {
                                                div {
                                                    role: "status",
                                                    "aria-label": "Graded answer verdict",
                                                    "data-testid": "graded-verdict",
                                                    "data-verdict": "{verdict.status_slug()}",
                                                    style: "margin-top: 1rem; padding: 0.875rem; border-radius: var(--radius); border: 1px solid var(--border); background: var(--secondary); display: flex; flex-direction: column; gap: 0.5rem;",
                                                    div {
                                                        class: "flex items-center justify-between",
                                                        style: "gap: 0.5rem; flex-wrap: wrap;",
                                                        div {
                                                            class: "flex items-center",
                                                            style: "gap: 0.5rem;",
                                                            if verdict.is_correct {
                                                                IconCheck { size: 18 }
                                                            } else {
                                                                IconX { size: 18 }
                                                            }
                                                            strong {
                                                                "data-testid": "verdict-headline",
                                                                style: "font-size: 0.875rem;",
                                                                "{verdict.headline()}"
                                                            }
                                                        }
                                                        Badge {
                                                            variant: if verdict.is_correct {
                                                                BadgeVariant::Success
                                                            } else if verdict.is_idk {
                                                                BadgeVariant::Warning
                                                            } else {
                                                                BadgeVariant::Destructive
                                                            },
                                                            test_id: "verdict-badge",
                                                            "Choice [{verdict.selected_key}] · Conf {verdict.confidence}/5"
                                                        }
                                                    }
                                                    p {
                                                        "data-testid": "verdict-explanation",
                                                        style: "margin: 0; font-size: 0.8125rem; line-height: 1.5;",
                                                        "{verdict.explanation}"
                                                    }
                                                    div {
                                                        "data-testid": "verdict-envelope",
                                                        class: "text-muted-foreground",
                                                        style: "font-family: var(--font-mono); font-size: 0.7rem;",
                                                        "{verdict.envelope}"
                                                    }
                                                }
                                            }
                                        }
                                    }

                                    // 5. Follow-up Learner Prompt Form (prevent_default enforced)
                                    form {
                                        "data-testid": "classroom-prompt-form",
                                        "aria-label": "Send follow-up question to teacher agent",
                                        class: "flex items-center",
                                        style: "gap: 0.5rem;",
                                        onsubmit: {
                                            let backend = backend.clone();
                                            move |evt| {
                                                evt.prevent_default();
                                                let mut sess = Session::new(active_session_id(), active_session().kind);
                                                sess.push_user(prompt_draft());
                                                spawn_turn(backend.clone(), sess, turn_stream_text, turn_result);
                                            }
                                        },
                                        input {
                                            r#type: "text",
                                            "aria-label": "Ask a follow-up question in the classroom",
                                            "data-testid": "classroom-prompt-input",
                                            value: "{prompt_draft()}",
                                            placeholder: "Ask a follow-up or request a sub-agent visual…",
                                            style: "flex: 1; height: 2.25rem; border-radius: var(--radius); border: 1px solid var(--border); background: var(--card); color: var(--foreground); padding: 0 0.75rem; font-size: 0.8125rem;",
                                            oninput: move |evt| prompt_draft.set(evt.value()),
                                        }
                                        Button {
                                            button_type: "submit",
                                            variant: ButtonVariant::Secondary,
                                            size: ButtonSize::Default,
                                            test_id: "classroom-send-turn",
                                            aria_label: "Send prompt to classroom agent",
                                            IconTerminal { size: 15 }
                                            "Ask Teacher"
                                        }
                                    }
                                }
                            }
                        },
                        second: rsx! {
                            // ============================================================
                            // ZONE 3: Right Plan DAG SVG Pane (Current / Visited / Upcoming)
                            // ============================================================
                            div {
                                role: "region",
                                "aria-label": "Plan DAG viewport",
                                "data-testid": "plan-dag-pane",
                                class: "flex flex-col bg-card",
                                style: "height: 100%; min-height: 0; border: 1px solid var(--border); border-radius: var(--radius); padding: 0.75rem; gap: 0.625rem; overflow-y: auto;",

                                // Plan DAG Header & Progress
                                div {
                                    class: "flex items-center justify-between",
                                    style: "gap: 0.5rem; flex-wrap: wrap;",
                                    div {
                                        class: "flex items-center",
                                        style: "gap: 0.5rem;",
                                        IconSparkles { size: 17 }
                                        strong {
                                            style: "font-size: 0.875rem;",
                                            "Plan DAG (v{tree.version})"
                                        }
                                    }
                                    div {
                                        class: "flex items-center",
                                        style: "gap: 0.25rem;",
                                        Button {
                                            variant: ButtonVariant::Ghost,
                                            size: ButtonSize::Xs,
                                            test_id: "dag-zoom-in",
                                            aria_label: "Zoom in Plan DAG",
                                            onclick: move |_| dag_zoom.set((dag_zoom() + 0.15).clamp(0.5, 2.5)),
                                            IconZoomIn { size: 14 }
                                        }
                                        Button {
                                            variant: ButtonVariant::Ghost,
                                            size: ButtonSize::Xs,
                                            test_id: "dag-zoom-out",
                                            aria_label: "Zoom out Plan DAG",
                                            onclick: move |_| dag_zoom.set((dag_zoom() - 0.15).clamp(0.5, 2.5)),
                                            IconZoomOut { size: 14 }
                                        }
                                        Button {
                                            variant: ButtonVariant::Ghost,
                                            size: ButtonSize::Xs,
                                            test_id: "dag-zoom-reset",
                                            aria_label: "Reset Plan DAG camera",
                                            onclick: move |_| {
                                                dag_zoom.set(1.0);
                                                dag_pan_x.set(0.0);
                                                dag_pan_y.set(0.0);
                                            },
                                            IconRefresh { size: 14 }
                                        }
                                    }
                                }

                                Progress {
                                    value: visited_pct,
                                    max: 100.0,
                                    aria_label: "Plan DAG teaching completion progress",
                                    test_id: "plan-dag-progress",
                                }

                                // Status Legend (Current / Visited / Upcoming)
                                div {
                                    "data-testid": "plan-status-legend",
                                    class: "flex items-center justify-between",
                                    style: "gap: 0.375rem; flex-wrap: wrap; font-size: 0.72rem;",
                                    Badge {
                                        variant: BadgeVariant::Phase,
                                        test_id: "legend-status-current",
                                        "● Current ({usize::from(statuses.current.is_some())})"
                                    }
                                    Badge {
                                        variant: BadgeVariant::Success,
                                        test_id: "legend-status-visited",
                                        "✓ Visited ({statuses.visited.len()})"
                                    }
                                    Badge {
                                        variant: BadgeVariant::Secondary,
                                        test_id: "legend-status-upcoming",
                                        "○ Upcoming ({statuses.upcoming.len()})"
                                    }
                                }

                                // Interactive SVG Node-Link Diagram
                                div {
                                    style: "position: relative; border: 1px solid var(--border); border-radius: var(--radius); background: var(--background); overflow: hidden; min-height: 21rem;",
                                    svg {
                                        xmlns: "http://www.w3.org/2000/svg",
                                        view_box: "0 0 520 340",
                                        width: "100%",
                                        height: "340",
                                        role: "img",
                                        "aria-label": "Lesson Plan DAG dependency graph",
                                        "data-testid": "plan-dag-svg",
                                        "data-zoom": "{dag_zoom():.2}",
                                        onwheel: move |evt| {
                                            let dy = evt.delta().strip_units().y;
                                            let next = if dy < 0.0 {
                                                dag_zoom() + 0.1
                                            } else {
                                                dag_zoom() - 0.1
                                            };
                                            dag_zoom.set(next.clamp(0.5, 2.5));
                                        },
                                        onmousedown: move |_| dag_dragging.set(true),
                                        onmouseup: move |_| dag_dragging.set(false),
                                        onmousemove: move |evt| {
                                            if dag_dragging() {
                                                let coords = evt.element_coordinates();
                                                dag_pan_x.set((coords.x - 260.0) * 0.15);
                                                dag_pan_y.set((coords.y - 170.0) * 0.15);
                                            }
                                        },

                                        g {
                                            "data-testid": "plan-dag-camera",
                                            transform: "translate({dag_pan_x():.1}, {dag_pan_y():.1}) scale({dag_zoom():.2})",

                                            // Cubic-Bezier Directed Edges
                                            for (from_id, to_id) in &tree.edges {
                                                {
                                                    let from_pos = positioned_nodes.iter().find(|(n, _, _)| &n.id == from_id);
                                                    let to_pos = positioned_nodes.iter().find(|(n, _, _)| &n.id == to_id);
                                                    if let (Some((_, x1, y1)), Some((_, x2, y2))) = (from_pos, to_pos) {
                                                        let mid_y = (y1 + y2) / 2.0;
                                                        let path_d = format!(
                                                            "M {x1:.1} {:.1} C {x1:.1} {mid_y:.1}, {x2:.1} {mid_y:.1}, {x2:.1} {:.1}",
                                                            y1 + 26.0,
                                                            y2 - 26.0
                                                        );
                                                        rsx! {
                                                            path {
                                                                key: "{from_id}->{to_id}",
                                                                d: "{path_d}",
                                                                fill: "none",
                                                                stroke: "var(--border)",
                                                                stroke_width: "2",
                                                                "data-testid": "dag-edge-{from_id}-{to_id}",
                                                            }
                                                        }
                                                    } else {
                                                        rsx! {}
                                                    }
                                                }
                                            }

                                            // Plan Nodes with Status Rings (Current / Visited / Upcoming)
                                            for (node, x, y) in &positioned_nodes {
                                                {
                                                    let status = PlanNodeStatus::from_statuses(&statuses, &node.id);
                                                    let is_selected = selected_dag_node() == node.id;
                                                    let node_id = node.id.clone();
                                                    let stroke_col = status.stroke_color();
                                                    let rx = x - 88.0;
                                                    let ry = y - 24.0;
                                                    rsx! {
                                                        g {
                                                            key: "{node.id}",
                                                            role: "button",
                                                            tabindex: "0",
                                                            "aria-label": "Plan node {node.title} ({status.badge_label()})",
                                                            "data-testid": "dag-node-{node.id}",
                                                            "data-node-id": "{node.id}",
                                                            "data-status": "{status.slug()}",
                                                            style: "cursor: pointer;",
                                                            onclick: move |_| selected_dag_node.set(node_id.clone()),
                                                            rect {
                                                                x: "{rx:.1}",
                                                                y: "{ry:.1}",
                                                                width: "176",
                                                                height: "48",
                                                                rx: "10",
                                                                fill: "var(--card)",
                                                                stroke: "{stroke_col}",
                                                                stroke_width: if is_selected || status == PlanNodeStatus::Current { "2.5" } else { "1.5" },
                                                            }
                                                            circle {
                                                                cx: "{rx + 16.0:.1}",
                                                                cy: "{y:.1}",
                                                                r: "6",
                                                                fill: "{stroke_col}",
                                                            }
                                                            text {
                                                                x: "{rx + 30.0:.1}",
                                                                y: "{y - 4.0:.1}",
                                                                fill: "var(--foreground)",
                                                                font_size: "11",
                                                                font_weight: "600",
                                                                "{node.title}"
                                                            }
                                                            text {
                                                                x: "{rx + 30.0:.1}",
                                                                y: "{y + 12.0:.1}",
                                                                fill: "{stroke_col}",
                                                                font_size: "9.5",
                                                                font_weight: "600",
                                                                "{status.badge_label()}"
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // Node Inspector & Position-From-Calls Stepper
                                div {
                                    "data-testid": "dag-node-list",
                                    class: "flex flex-col",
                                    style: "gap: 0.375rem;",
                                    for node in &tree.nodes {
                                        {
                                            let status = PlanNodeStatus::from_statuses(&statuses, &node.id);
                                            let node_id = node.id.clone();
                                            rsx! {
                                                div {
                                                    key: "{node.id}",
                                                    "data-testid": "dag-node-row-{node.id}",
                                                    "data-status": "{status.slug()}",
                                                    class: "flex items-center justify-between",
                                                    style: "padding: 0.45rem 0.625rem; border-radius: var(--radius); border: 1px solid var(--border); background: var(--background); gap: 0.5rem;",
                                                    div {
                                                        style: "min-width: 0; flex: 1;",
                                                        div {
                                                            class: "flex items-center",
                                                            style: "gap: 0.375rem;",
                                                            strong {
                                                                style: "font-size: 0.78rem;",
                                                                "{node.title}"
                                                            }
                                                            Badge {
                                                                variant: status.badge_variant(),
                                                                test_id: format!("dag-status-badge-{}", node.id),
                                                                "{status.badge_label()}"
                                                            }
                                                        }
                                                        if !node.summary.is_empty() {
                                                            p {
                                                                class: "text-muted-foreground",
                                                                style: "margin: 0.15rem 0 0 0; font-size: 0.7rem;",
                                                                "{node.summary}"
                                                            }
                                                        }
                                                    }
                                                    Button {
                                                        variant: ButtonVariant::Outline,
                                                        size: ButtonSize::Xs,
                                                        test_id: format!("dag-focus-node-{}", node.id),
                                                        aria_label: format!("Teach node {} via quiz tool call", node.title),
                                                        onclick: move |_| {
                                                            selected_dag_node.set(node_id.clone());
                                                            let mut calls = tool_calls();
                                                            calls.push(node_call(
                                                                "quiz",
                                                                &node_id,
                                                                "check",
                                                            ));
                                                            tool_calls.set(calls);
                                                        },
                                                        "Teach Node"
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                // Mermaid Plan Source Editor Form (prevent_default enforced)
                                form {
                                    "data-testid": "plan-mermaid-form",
                                    "aria-label": "Update plan_init Mermaid source",
                                    class: "flex flex-col",
                                    style: "gap: 0.375rem; margin-top: 0.25rem;",
                                    onsubmit: move |evt| {
                                        evt.prevent_default();
                                        match parse_plan(&plan_source(), 2) {
                                            Ok(_) => plan_error.set(None),
                                            Err(err) => plan_error.set(Some(err.to_string())),
                                        }
                                    },
                                    textarea {
                                        "aria-label": "Mermaid graph TD plan source",
                                        "data-testid": "plan-mermaid-input",
                                        rows: "3",
                                        value: "{plan_source()}",
                                        style: "width: 100%; border-radius: var(--radius); border: 1px solid var(--border); background: var(--background); color: var(--foreground); padding: 0.4rem; font-size: 0.72rem; font-family: var(--font-mono);",
                                        oninput: move |evt| plan_source.set(evt.value()),
                                    }
                                    if let Some(err) = plan_error() {
                                        p {
                                            role: "alert",
                                            "data-testid": "plan-mermaid-error",
                                            style: "margin: 0; font-size: 0.72rem; color: var(--destructive);",
                                            "Plan syntax error: {err}"
                                        }
                                    }
                                    Button {
                                        button_type: "submit",
                                        variant: ButtonVariant::Outline,
                                        size: ButtonSize::Xs,
                                        test_id: "plan-mermaid-validate-btn",
                                        aria_label: "Validate Mermaid plan_init DAG",
                                        "Validate Mermaid DAG"
                                    }
                                }
                            }
                        },
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AppWithBackend, AppWithBackendProps};

    fn classroom_dom(view: View, backend: FauxBackend) -> VirtualDom {
        VirtualDom::new_with_props(
            AppWithBackend,
            AppWithBackendProps {
                backend: Arc::new(backend),
                initial_view: view,
                accounts_open: false,
            },
        )
    }

    #[test]
    fn classroom_view_renders_drawer_lesson_question_dock_plan_dag_and_workbench() {
        let mut dom = classroom_dom(View::Classroom, FauxBackend::default());
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        // Outer view & action status
        assert!(html.contains(r#"data-testid="view-classroom""#));
        assert!(html.contains(r#"data-testid="classroom-action""#));
        assert!(html.contains(r#"data-testid="classroom-phase-badge""#));
        assert!(html.contains("TEACH"));

        // Left Session Drawer & filters
        assert!(html.contains(r#"data-testid="session-drawer""#));
        assert!(html.contains(r#"data-testid="session-filter-all""#));
        assert!(html.contains(r#"data-testid="session-filter-teaching""#));
        assert!(html.contains(r#"data-testid="session-filter-practice""#));
        assert!(html.contains(r#"data-testid="session-list""#));

        // 4 Sub-Agent Workbench cards + semaphore badge
        assert!(html.contains(r#"data-testid="workbench-section""#));
        assert!(html.contains(r#"data-testid="workbench-semaphore-badge""#));
        for slug in ["research", "pdf", "visual", "demo"] {
            assert!(
                html.contains(&format!(r#"data-testid="workbench-card-{slug}""#)),
                "Missing workbench card for {slug}"
            );
        }

        // Center Lesson Stream (Markdown, Table, CodeBlock, SVG diagram)
        assert!(html.contains(r#"data-testid="lesson-stream""#));
        assert!(html.contains(r#"data-testid="lesson-markdown""#));
        assert!(html.contains(r#"data-testid="lesson-table""#));
        assert!(html.contains(r#"data-testid="lesson-code-block""#));
        assert!(html.contains(r#"data-testid="lesson-diagram-svg""#));

        // Interactive Question Dock ([A]..[D], [I], 1..5 Confidence Slider, Dual Thinking Tabs, Graded Verdict)
        assert!(html.contains(r#"data-testid="question-dock""#));
        for opt_key in ["A", "B", "C", "D", "I"] {
            assert!(
                html.contains(&format!(r#"data-testid="option-{opt_key}""#)),
                "Missing option-{opt_key}"
            );
        }
        assert!(html.contains(r#"data-testid="confidence-slider""#));
        assert!(html.contains(r#"data-testid="confidence-slider-step-4""#));
        assert!(html.contains(r#"data-testid="thinking-tabs""#));
        assert!(html.contains(r#"data-testid="tab-trigger-notes""#));
        assert!(html.contains(r#"data-testid="tab-trigger-reasoning""#));
        assert!(html.contains(r#"data-testid="reading-notes-input""#));
        assert!(html.contains(r#"data-testid="answer-reasoning-input""#));
        assert!(html.contains(r#"data-testid="graded-verdict""#));
        assert!(html.contains(r#"data-verdict="correct""#));
        assert!(html.contains(r#"data-testid="verdict-explanation""#));

        // Right Plan DAG SVG Pane (Current, Visited, Upcoming derived via derive_plan_statuses)
        assert!(html.contains(r#"data-testid="plan-dag-pane""#));
        assert!(html.contains(r#"data-testid="plan-dag-svg""#));
        assert!(html.contains(r#"data-testid="dag-node-wal-basics""#));
        assert!(html.contains(r#"data-testid="dag-node-snapshot-isolation""#));
        assert!(html.contains(r#"data-testid="dag-node-fts5-sync""#));
        assert!(html.contains(r#"data-testid="dag-node-dag-derivation""#));
        assert!(html.contains(r#"data-status="visited""#));
        assert!(html.contains(r#"data-status="current""#));
        assert!(html.contains(r#"data-status="upcoming""#));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn plan_view_route_renders_classroom_plan_dag_with_view_plan_testid() {
        let mut dom = classroom_dom(View::Plan, FauxBackend::default());
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);

        assert!(html.contains(r#"data-testid="view-plan""#));
        assert!(html.contains(r#"data-testid="plan-dag-svg""#));
    }

    #[test]
    fn question_grading_handles_correct_incorrect_and_idk_options() {
        let quiz = default_quiz_input();

        let right = evaluate_question_submission(&quiz, "A", 5, "notes", "because A");
        assert!(right.is_correct);
        assert!(!right.is_idk);
        assert_eq!(right.status_slug(), "correct");
        assert!(right.envelope.contains("selected: opt_calls_truth"));
        assert!(right.envelope.contains("confidence: 5"));

        let wrong = evaluate_question_submission(&quiz, "B", 2, "", "");
        assert!(!wrong.is_correct);
        assert!(!wrong.is_idk);
        assert_eq!(wrong.status_slug(), "incorrect");
        assert!(wrong.envelope.contains("selected: opt_mermaid_limit"));

        let idk = evaluate_question_submission(&quiz, "I", 1, "", "");
        assert!(!idk.is_correct);
        assert!(idk.is_idk);
        assert_eq!(idk.status_slug(), "idk");
        assert_eq!(idk.selected_value, "__idk__");
    }

    #[test]
    fn dag_coordinates_and_derived_statuses_follow_tool_calls() {
        let tree = parse_plan(DEFAULT_MERMAID_PLAN, 1).unwrap();
        let coords = compute_dag_coordinates(&tree);
        assert_eq!(coords.len(), 4);
        // Root node `wal-basics` sits above leaf `dag-derivation`
        let root_y = coords
            .iter()
            .find(|(n, _, _)| n.id == "wal-basics")
            .unwrap()
            .2;
        let leaf_y = coords
            .iter()
            .find(|(n, _, _)| n.id == "dag-derivation")
            .unwrap()
            .2;
        assert!(root_y < leaf_y);

        let statuses = derive_plan_statuses(&tree, &default_tool_calls());
        assert_eq!(
            PlanNodeStatus::from_statuses(&statuses, "wal-basics"),
            PlanNodeStatus::Visited
        );
        assert_eq!(
            PlanNodeStatus::from_statuses(&statuses, "snapshot-isolation"),
            PlanNodeStatus::Current
        );
        assert_eq!(
            PlanNodeStatus::from_statuses(&statuses, "fts5-sync"),
            PlanNodeStatus::Upcoming
        );
    }

    #[tokio::test]
    async fn faux_backend_turn_streams_into_classroom_signal() {
        #[component]
        fn TurnHarness() -> Element {
            let backend: BackendHandle = Arc::new(FauxBackend::default());
            let text = use_signal(String::new);
            let result = use_signal(|| Option::<BackendResult<Session>>::None);
            let mut started = use_signal(|| false);
            if !started() {
                started.set(true);
                spawn_turn(
                    backend,
                    Session::new("sess-wal-teach", Kind::Teach),
                    text,
                    result,
                );
            }
            rsx! {
                div {
                    "data-testid": "harness-stream",
                    "{text()}"
                }
            }
        }

        let mut dom = VirtualDom::new(TurnHarness);
        dom.rebuild_in_place();
        let html = tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                let rendered = dioxus::ssr::render(&dom);
                if rendered.contains("Ready to learn.") {
                    return rendered;
                }
                dom.wait_for_work().await;
                dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
            }
        })
        .await
        .expect("faux backend turn should stream into signal");
        assert!(html.contains("Ready to learn."));
    }
}
