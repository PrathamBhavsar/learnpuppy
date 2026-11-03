//! Real-CLI run of `write_course`.
//!
//! cargo run -p lp-agent --example course_real -- <provider> <model> [--effort low] [--goal TEXT] \
//!     [--k 10] [--cheap-model MODEL] [--two-stage]
//!
//! Default: one Author call on `<provider>/<model>`; the cheap fallback is switched off unless
//! `--cheap-model` is given. `--two-stage` runs only the cheap outline-then-lessons path on `<model>`.
//! Placement answers are scripted (two right, two wrong) so the run needs no input.

use std::sync::atomic::Ordering;

use async_trait::async_trait;
use lp_agent::course::{author_two_stage, write_course, Authored, PlacementResult};
use lp_cli_driver::engine::{
    CliKind, DriverError, LlmEngine, StreamDelta, StructuredOutput, StructuredRequest,
    SubprocessEngine, TurnRequest, TurnResponse,
};
use lp_core::course::{Item, ItemKind};
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

/// Cheap path switched off: fails at once so a strong failure is reported, not retried on a model.
struct Off;

#[async_trait]
impl LlmEngine for Off {
    async fn stream_turn(
        &self,
        _: TurnRequest,
        _: mpsc::Sender<StreamDelta>,
        _: CancellationToken,
    ) -> Result<TurnResponse, DriverError> {
        Err(DriverError::Failed("cheap fallback disabled".into()))
    }
    async fn complete_structured<T: DeserializeOwned + JsonSchema + Send>(
        &self,
        _: StructuredRequest,
        _: CancellationToken,
    ) -> Result<StructuredOutput<T>, DriverError> {
        Err(DriverError::Failed("cheap fallback disabled".into()))
    }
}

fn engine(kind: &str, model: &str, effort: Option<&str>) -> SubprocessEngine {
    let kind = match kind {
        "claude" => CliKind::Claude,
        "codex" => CliKind::Codex,
        "agy" => CliKind::Agy,
        "opencode" => CliKind::Opencode,
        "kiro" => CliKind::Kiro,
        other => panic!("unknown provider {other}"),
    };
    let mut e = SubprocessEngine::new(kind).expect("engine");
    e.model = Some(model.to_string());
    e.effort = effort.map(String::from);
    e
}

fn arg(args: &[String], name: &str) -> Option<String> {
    args.iter()
        .position(|a| a == name)
        .and_then(|i| args.get(i + 1))
        .cloned()
}

fn placement() -> Vec<PlacementResult> {
    let q = |q: &str, a: &str, given: &str, correct: bool| PlacementResult {
        item: Item {
            kind: ItemKind::Recall,
            q: q.into(),
            answer: a.into(),
            ..Item::default()
        },
        given: given.into(),
        correct,
    };
    vec![
        q(
            "What does `let x = 5;` do in Rust?",
            "binds an immutable x to 5",
            "binds x to 5",
            true,
        ),
        q("Which keyword makes a binding mutable?", "mut", "mut", true),
        q(
            "What happens to a String when it is moved into a function?",
            "the caller can no longer use it",
            "it is copied",
            false,
        ),
        q(
            "What does `&mut T` give you?",
            "an exclusive mutable borrow",
            "a pointer like C",
            false,
        ),
    ]
}

fn report(label: &str, a: &Authored, e: &SubprocessEngine) {
    let u = &e.usage;
    println!(
        "{label}: lessons={} items={} dropped={} warnings={} needs_repair={} usage in={} out={}",
        a.course.lessons.len(),
        a.course
            .lessons
            .iter()
            .map(|l| l.items.len())
            .sum::<usize>(),
        a.report.dropped.len(),
        a.report.warnings.len(),
        a.report.needs_repair.len(),
        u[0].load(Ordering::Relaxed),
        u[1].load(Ordering::Relaxed),
    );
    for f in a.report.dropped.iter().chain(&a.report.needs_repair) {
        println!("  {:?}/{:?}: {}", f.lesson, f.item, f.reason);
    }
    for l in &a.course.lessons {
        println!("  - {} ({} items)", l.title, l.items.len());
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (provider, model) = (args.first().expect("provider"), args.get(1).expect("model"));
    let effort = arg(&args, "--effort");
    let k: usize = arg(&args, "--k").and_then(|k| k.parse().ok()).unwrap_or(10);
    let goal = arg(&args, "--goal").unwrap_or_else(|| {
        "I know some Python and want to learn Rust ownership and borrowing properly".into()
    });
    let results = placement();
    let profile = "Knows Python; new to Rust.";
    let strong = engine(provider, model, effort.as_deref());
    let t = std::time::Instant::now();
    if args.iter().any(|a| a == "--two-stage") {
        let a = author_two_stage(&strong, &goal, &results, profile, k)
            .await
            .expect("two-stage");
        report("two-stage", &a, &strong);
    } else if let Some(cheap) = arg(&args, "--cheap-model") {
        let cheap = engine(provider, &cheap, None);
        let w = write_course(&strong, &cheap, &goal, &results, profile, k)
            .await
            .expect("write_course");
        report(&format!("{:?}", w.path), &w.authored, &strong);
        println!(
            "cheap usage in={} out={}",
            cheap.usage[0].load(Ordering::Relaxed),
            cheap.usage[1].load(Ordering::Relaxed)
        );
    } else {
        let w = write_course(&strong, &Off, &goal, &results, profile, k)
            .await
            .expect("write_course");
        report(&format!("{:?}", w.path), &w.authored, &strong);
    }
    println!("elapsed {:.1}s", t.elapsed().as_secs_f32());
}
