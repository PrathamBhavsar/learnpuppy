//! Course agents: placement, the Author (one strong call, or a cheap two-stage fallback), refill,
//! follow-up answers, answer checking and re-teaching. Each function is one or a few single-shot
//! `stream_turn` calls with a short static system text. Claude, kiro and opencode run with no tools;
//! Codex and agy can still read files (`docs/web/12-cli-isolation-and-failures.md`, #414). All learner
//! text is redacted and fenced as data with a per-prompt random nonce.
//!
//! Model and effort come from the engine (its role profile); usage is read from the engine by the
//! service layer, since `LlmEngine` exposes none.

use std::collections::HashSet;

use lp_cli_driver::engine::{DriverError, LlmEngine, TurnRequest};
use lp_core::course::{
    gate_course, gate_item, render_digest, render_packet, Course, GateReport, Item, LearnerState,
    Lesson, Level, PACKET_NOTE,
};
use lp_core::redact::redact;
use rand::Rng;
use serde::Deserialize;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const AUTHOR: &str = include_str!("prompts/author.md");
const OUTLINE: &str = include_str!("prompts/outline.md");
const LESSON: &str = include_str!("prompts/lesson.md");
const PLACE: &str = include_str!("prompts/place.md");
const REPAIR: &str = include_str!("prompts/repair.md");
const FOLLOW_UP: &str = include_str!("prompts/follow_up.md");
const CHECK: &str = include_str!("prompts/check.md");
const RETEACH: &str = include_str!("prompts/reteach.md");
const SYSTEM: &str = include_str!("prompts/system.md");

/// Items per lesson, matching the tested prompts.
const ITEMS_PER_LESSON: usize = 6;
const PLACEMENT_QUESTIONS: usize = 5;
const MIN_PLACEMENT: usize = 3;

#[derive(Debug, thiserror::Error)]
pub enum CourseAgentError {
    #[error("engine: {0}")]
    Engine(#[from] DriverError),
    #[error("unusable reply: {0}")]
    Parse(String),
    #[error("no usable lesson was written")]
    NoUsableLesson,
}

/// One placement question with the learner's answer.
#[derive(Clone, Debug, PartialEq)]
pub struct PlacementResult {
    pub item: Item,
    pub given: String,
    pub correct: bool,
}

/// A course with the gate report that produced it.
#[derive(Clone, Debug, PartialEq)]
pub struct Authored {
    pub course: Course,
    pub report: GateReport,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorPath {
    Strong,
    TwoStage,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Written {
    pub authored: Authored,
    pub path: AuthorPath,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Check {
    pub correct: bool,
    pub note: String,
}

// ------------------------------------------------------------------ plumbing

/// Single-pass `{key}` substitution: substituted text is never scanned again.
fn render(tpl: &str, vars: &[(&str, &str)]) -> String {
    let mut out = String::with_capacity(tpl.len());
    let mut rest = tpl;
    'outer: while let Some(i) = rest.find('{') {
        out.push_str(&rest[..i]);
        rest = &rest[i..];
        for (k, v) in vars {
            let pat = format!("{{{k}}}");
            if rest.starts_with(&pat) {
                out.push_str(v);
                rest = &rest[pat.len()..];
                continue 'outer;
            }
        }
        out.push('{');
        rest = &rest[1..];
    }
    out.push_str(rest);
    out
}

/// Fences untrusted text with a nonce the text cannot know, so it cannot close the block early.
struct Fence(String);

impl Fence {
    fn new() -> Self {
        Self(format!("data-{:016x}", rand::thread_rng().gen::<u64>()))
    }

    fn wrap(&self, text: &str) -> String {
        self.seal(&redact(text.trim()))
    }

    /// Redacts string values only: `redact` on raw JSON would eat the `[` after `"key_facts":`.
    fn wrap_json(&self, mut value: serde_json::Value) -> String {
        fn walk(v: &mut serde_json::Value) {
            match v {
                serde_json::Value::String(s) => *s = redact(s),
                serde_json::Value::Array(a) => a.iter_mut().for_each(walk),
                serde_json::Value::Object(o) => o.values_mut().for_each(walk),
                _ => {}
            }
        }
        walk(&mut value);
        self.seal(&value.to_string())
    }

    fn seal(&self, text: &str) -> String {
        let text = text.replace(&self.0, "");
        format!("\n<<{0}>>\n{text}\n<<{0}>>", self.0)
    }
}

async fn ask<E: LlmEngine>(engine: &E, prompt: String) -> Result<String, CourseAgentError> {
    let req = TurnRequest {
        prompt,
        system: Some(SYSTEM.trim().to_string()),
        ..TurnRequest::default()
    };
    let (tx, mut rx) = mpsc::channel(64);
    let drain = async { while rx.recv().await.is_some() {} };
    let (resp, ()) = tokio::join!(engine.stream_turn(req, tx, CancellationToken::new()), drain);
    Ok(resp?.text)
}

fn json_body(s: &str) -> &str {
    match (s.find('{'), s.rfind('}')) {
        (Some(a), Some(b)) if a < b => &s[a..=b],
        _ => s,
    }
}

fn parse<T: for<'de> Deserialize<'de>>(s: &str) -> Result<T, CourseAgentError> {
    serde_json::from_str(json_body(s)).map_err(|e| CourseAgentError::Parse(e.to_string()))
}

/// One call, parsed; a reply that does not parse gets exactly one retry. Engine errors do not retry.
async fn ask_parsed<E: LlmEngine, T>(
    engine: &E,
    prompt: &str,
    f: impl Fn(&str) -> Result<T, CourseAgentError>,
) -> Result<T, CourseAgentError> {
    let mut last = None;
    for _ in 0..2 {
        match f(&ask(engine, prompt.to_string()).await?) {
            Ok(v) => return Ok(v),
            Err(e) => last = Some(e),
        }
    }
    Err(last.unwrap_or(CourseAgentError::NoUsableLesson))
}

/// Everything the Author is told about the learner. Goes into the prompt as one fenced block.
struct Brief {
    goal: String,
    profile: String,
    results: Vec<PlacementResult>,
    /// Course-level text for a refill: digest and titles already taught.
    extra: String,
}

impl Brief {
    fn must_fix(&self) -> Vec<String> {
        self.results
            .iter()
            .filter(|r| !r.correct)
            .map(|r| r.item.q.trim().to_string())
            .collect()
    }

    fn topic(&self, fence: &Fence) -> String {
        let mut s = format!("Goal: {}\nProfile: {}\n", self.goal, self.profile);
        if !self.results.is_empty() {
            s.push_str("Placement answers:\n");
            for r in &self.results {
                s.push_str(&format!(
                    "- Q: {} | Model answer: {} | Learner answered: {} | {}\n",
                    r.item.q,
                    r.item.answer,
                    r.given,
                    if r.correct { "right" } else { "wrong" }
                ));
            }
        }
        let mf = self.must_fix();
        if !mf.is_empty() {
            s.push_str(
                "Placement mistakes (each must appear in some lesson's fixes, in these words):\n",
            );
            for m in &mf {
                s.push_str(&format!("- {m}\n"));
            }
        }
        if !self.extra.is_empty() {
            s.push_str(&self.extra);
        }
        format!("{PACKET_NOTE}{}", fence.wrap(&s))
    }
}

fn prune(course: &mut Course, report: &GateReport) {
    let bad: HashSet<&str> = report
        .needs_repair
        .iter()
        .filter_map(|f| f.lesson.as_deref())
        .collect();
    course.lessons.retain(|l| !bad.contains(l.id.as_str()));
}

fn finish(mut course: Course, mut report: GateReport) -> Result<Authored, CourseAgentError> {
    prune(&mut course, &report);
    // Lessons pruned for too few items are no longer a repair matter; uncovered must_fix stays listed.
    report.needs_repair.retain(|f| f.lesson.is_none());
    if course.lessons.is_empty() {
        return Err(CourseAgentError::NoUsableLesson);
    }
    Ok(Authored { course, report })
}

fn merge_reports(first: GateReport, second: GateReport) -> GateReport {
    let mut dropped = first.dropped;
    dropped.extend(second.dropped);
    GateReport {
        dropped,
        warnings: second.warnings,
        needs_repair: second.needs_repair,
    }
}

/// Position-based ids, as `parse_course` assigns them.
fn item_ids(lesson: &mut Lesson, n: usize) {
    for (ii, it) in lesson.items.iter_mut().enumerate() {
        it.id = format!("l{n:02}-i{ii}");
    }
}

// ------------------------------------------------------------------ placement

pub async fn place<E: LlmEngine>(engine: &E, goal: &str) -> Result<Vec<Item>, CourseAgentError> {
    #[derive(Deserialize)]
    struct Raw {
        #[serde(default)]
        items: Vec<Item>,
    }
    let fence = Fence::new();
    let prompt = render(
        PLACE,
        &[("topic", &format!("{PACKET_NOTE}{}", fence.wrap(goal)))],
    );
    ask_parsed(engine, &prompt, |s| {
        let mut items: Vec<Item> = parse::<Raw>(s)?
            .items
            .into_iter()
            .filter(|i| !gate_item(i).iter().any(|f| f.level == Level::Drop))
            .take(PLACEMENT_QUESTIONS)
            .collect();
        if items.len() < MIN_PLACEMENT {
            return Err(CourseAgentError::Parse(format!(
                "only {} usable placement questions",
                items.len()
            )));
        }
        for (i, it) in items.iter_mut().enumerate() {
            it.id = format!("p{i}");
        }
        Ok(items)
    })
    .await
}

// ------------------------------------------------------------------ Author

fn brief_for(goal: &str, results: &[PlacementResult], profile: &str) -> Brief {
    Brief {
        goal: goal.to_string(),
        profile: profile.to_string(),
        results: results.to_vec(),
        extra: String::new(),
    }
}

/// One strong call for `k` lessons, gated, with at most one repair call.
pub async fn author<E: LlmEngine>(
    engine: &E,
    goal: &str,
    placement_results: &[PlacementResult],
    profile: &str,
    k: usize,
) -> Result<Authored, CourseAgentError> {
    author_brief(
        engine,
        &brief_for(goal, placement_results, profile),
        "course",
        k,
    )
    .await
}

async fn author_brief<E: LlmEngine>(
    engine: &E,
    brief: &Brief,
    id: &str,
    k: usize,
) -> Result<Authored, CourseAgentError> {
    let fence = Fence::new();
    let topic = brief.topic(&fence);
    let prompt = render(
        AUTHOR,
        &[
            ("k", &k.to_string()),
            ("n", &ITEMS_PER_LESSON.to_string()),
            ("topic", &topic),
        ],
    );
    let must_fix = brief.must_fix();
    let mut course = ask_parsed(engine, &prompt, |s| {
        lp_core::course::parse_course(s, id, must_fix.clone())
            .map_err(|e| CourseAgentError::Parse(e.to_string()))
    })
    .await?;
    let first = gate_course(&mut course);
    if first.needs_repair.is_empty() {
        return finish(course, first);
    }
    let (repaired, report) = match repair(engine, &course, &first, &topic).await {
        Some(c) => {
            let mut c = c;
            let second = gate_course(&mut c);
            (c, merge_reports(first, second))
        }
        None => (course, first),
    };
    finish(repaired, report)
}

/// One repair call with only the failed rules and the broken lessons; `None` if it fails.
async fn repair<E: LlmEngine>(
    engine: &E,
    course: &Course,
    report: &GateReport,
    topic: &str,
) -> Option<Course> {
    #[derive(Deserialize)]
    struct Raw {
        #[serde(default)]
        lessons: Vec<Lesson>,
    }
    let broken: HashSet<&str> = report
        .needs_repair
        .iter()
        .filter_map(|f| f.lesson.as_deref())
        .collect();
    let rules: Vec<String> = report
        .needs_repair
        .iter()
        .map(|f| match &f.lesson {
            Some(l) => format!("- lesson {l}: {}", f.reason),
            None => format!("- {}", f.reason),
        })
        .collect();
    // With no lesson named, the rule is about coverage: show the lessons so one can be amended.
    let shown: Vec<&Lesson> = course
        .lessons
        .iter()
        .filter(|l| broken.is_empty() || broken.contains(l.id.as_str()))
        .collect();
    let fence = Fence::new();
    let lessons = serde_json::to_value(&shown).ok()?;
    let prompt = render(
        REPAIR,
        &[
            ("n", &ITEMS_PER_LESSON.to_string()),
            // Rules quote must_fix, which holds the learner's own placement text.
            (
                "rules",
                &format!("{PACKET_NOTE}{}", fence.wrap(&rules.join("\n"))),
            ),
            ("topic", topic),
            (
                "lessons",
                &format!("{PACKET_NOTE}{}", fence.wrap_json(lessons)),
            ),
        ],
    );
    let fixed: Raw = parse(&ask(engine, prompt).await.ok()?).ok()?;
    let mut merged = course.clone();
    let mut any = false;
    for f in fixed.lessons {
        if let Some(i) = merged.lessons.iter().position(|l| l.id == f.id) {
            let mut f = f;
            item_ids(&mut f, i);
            merged.lessons[i] = f;
            any = true;
        }
    }
    any.then_some(merged)
}

// ------------------------------------------------------------------ two-stage Author

#[derive(Deserialize)]
struct Outline {
    #[serde(default)]
    learner: String,
    #[serde(default)]
    lessons: Vec<Lesson>,
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct Pack {
    explain: String,
    items: Vec<Item>,
}

/// The cheap path: an outline call, then one call per lesson. A lesson call that fails retries once,
/// then the lesson is skipped.
pub async fn author_two_stage<E: LlmEngine>(
    cheap: &E,
    goal: &str,
    placement_results: &[PlacementResult],
    profile: &str,
    k: usize,
) -> Result<Authored, CourseAgentError> {
    two_stage_brief(
        cheap,
        &brief_for(goal, placement_results, profile),
        "course",
        k,
        &HashSet::new(),
    )
    .await
}

async fn two_stage_brief<E: LlmEngine>(
    cheap: &E,
    brief: &Brief,
    id: &str,
    k: usize,
    taken: &HashSet<String>,
) -> Result<Authored, CourseAgentError> {
    let fence = Fence::new();
    let prompt = render(
        OUTLINE,
        &[("k", &k.to_string()), ("topic", &brief.topic(&fence))],
    );
    let outline: Outline = ask_parsed(cheap, &prompt, |s| {
        let o: Outline = parse(s)?;
        if o.lessons.is_empty() {
            return Err(CourseAgentError::Parse("outline has no lessons".into()));
        }
        Ok(o)
    })
    .await?;
    let mut used: Vec<String> = Vec::new();
    let mut lessons = Vec::new();
    for mut l in outline.lessons.into_iter().take(k) {
        let fence = Fence::new();
        let spec = serde_json::json!({
            "title": l.title, "objective": l.objective, "key_facts": l.key_facts,
        });
        let learner = format!("{PACKET_NOTE}{}", fence.wrap(&outline.learner));
        let prompt = render(
            LESSON,
            &[
                ("n", &ITEMS_PER_LESSON.to_string()),
                ("learner", &learner),
                ("lesson", &fence.wrap_json(spec)),
                ("used", &fence.wrap(&used.join(" | "))),
            ],
        );
        let Ok(pack) = ask_parsed(cheap, &prompt, |s| {
            let p: Pack = parse(s)?;
            if p.explain.trim().is_empty() || p.items.is_empty() {
                return Err(CourseAgentError::Parse("empty lesson".into()));
            }
            Ok(p)
        })
        .await
        else {
            continue;
        };
        used.extend(pack.items.iter().map(|i| i.q.clone()));
        l.explain = pack.explain;
        l.items = pack.items;
        lessons.push(l);
    }
    // Ids: keep the outline's slug when unique, else position; items by position.
    let mut seen: HashSet<String> = taken.clone();
    let mut course = Course {
        id: id.to_string(),
        learner: outline.learner,
        lessons: Vec::new(),
        must_fix: brief.must_fix(),
        version: 1,
        ..Course::default()
    };
    for mut l in lessons {
        let n = course.lessons.len();
        if l.id.trim().is_empty() || !seen.insert(l.id.clone()) {
            l.id = format!("l{n:02}");
            seen.insert(l.id.clone());
        }
        item_ids(&mut l, n);
        course.lessons.push(l);
    }
    let report = gate_course(&mut course);
    finish(course, report)
}

// ------------------------------------------------------------------ write and refill

/// Strong Author first; on any failure (a limit included) the cheap two-stage path.
pub async fn write_course<S: LlmEngine, C: LlmEngine>(
    strong: &S,
    cheap: &C,
    goal: &str,
    placement_results: &[PlacementResult],
    profile: &str,
    k: usize,
) -> Result<Written, CourseAgentError> {
    match author(strong, goal, placement_results, profile, k).await {
        Ok(authored) => Ok(Written {
            authored,
            path: AuthorPath::Strong,
        }),
        Err(_) => Ok(Written {
            authored: author_two_stage(cheap, goal, placement_results, profile, k).await?,
            path: AuthorPath::TwoStage,
        }),
    }
}

/// The next `k` lessons (10) from the digest and the titles already taught; ids never collide with the
/// course's, and the version is bumped. Same fallback as `write_course`.
pub async fn refill<S: LlmEngine, C: LlmEngine>(
    strong: &S,
    cheap: &C,
    course: &Course,
    state: &LearnerState,
) -> Result<Written, CourseAgentError> {
    let titles: Vec<&str> = course.lessons.iter().map(|l| l.title.as_str()).collect();
    let brief = Brief {
        goal:
            "Continue this learner's course with new lessons that follow the ones already taught."
                .into(),
        profile: course.learner.clone(),
        results: Vec::new(),
        extra: format!(
            "Digest:\n{}\nLessons already in the course (do not repeat): {}\n",
            render_digest(course, state),
            titles.join("; ")
        ),
    };
    let k = 10;
    let mut taken: HashSet<String> = course.lessons.iter().map(|l| l.id.clone()).collect();
    let (mut authored, path) = match author_brief(strong, &brief, &course.id, k).await {
        Ok(a) => (a, AuthorPath::Strong),
        Err(_) => (
            two_stage_brief(cheap, &brief, &course.id, k, &taken).await?,
            AuthorPath::TwoStage,
        ),
    };
    let used_prefix: HashSet<String> = course
        .lessons
        .iter()
        .flat_map(|l| &l.items)
        .filter_map(|i| i.id.split_once('-').map(|(p, _)| p.to_string()))
        .collect();
    let mut merged = course.clone();
    let mut n = course.lessons.len();
    for mut l in authored.course.lessons.drain(..) {
        while used_prefix.contains(&format!("l{n:02}")) {
            n += 1;
        }
        if l.id.trim().is_empty() || !taken.insert(l.id.clone()) {
            l.id = format!("l{n:02}");
            taken.insert(l.id.clone());
        }
        item_ids(&mut l, n);
        n += 1;
        merged.lessons.push(l);
    }
    merged.version = course.version + 1;
    authored.course = merged;
    Ok(Written { authored, path })
}

// ------------------------------------------------------------------ the cheap roles

/// A short plain-text answer built from the context packet.
pub async fn follow_up<E: LlmEngine>(
    engine: &E,
    packet: &str,
    question: &str,
) -> Result<String, CourseAgentError> {
    let fence = Fence::new();
    let prompt = render(
        FOLLOW_UP,
        &[
            ("packet", &format!("{PACKET_NOTE}{}", fence.wrap(packet))),
            ("question", &fence.wrap(question)),
        ],
    );
    let text = ask(engine, prompt).await?;
    let text = text.trim();
    if text.is_empty() {
        return Err(CourseAgentError::Parse("empty answer".into()));
    }
    Ok(text.to_string())
}

/// Judges a recall answer.
pub async fn check_answer<E: LlmEngine>(
    engine: &E,
    item: &Item,
    answer: &str,
) -> Result<Check, CourseAgentError> {
    #[derive(Deserialize)]
    struct Raw {
        correct: bool,
        #[serde(default)]
        note: String,
    }
    let fence = Fence::new();
    let prompt = render(
        CHECK,
        &[
            ("q", &fence.wrap(&item.q)),
            ("answer", &fence.wrap(&item.answer)),
            ("accept", &fence.wrap(&item.accept.join("; "))),
            ("given", &fence.wrap(answer)),
        ],
    );
    let prompt = format!("{PACKET_NOTE}\n{prompt}");
    let r: Raw = ask_parsed(engine, &prompt, parse::<Raw>).await?;
    Ok(Check {
        correct: r.correct,
        note: r.note,
    })
}

/// Rewrites a lesson's explanation from its key facts.
pub async fn reteach<E: LlmEngine>(
    engine: &E,
    lesson: &Lesson,
    state: &LearnerState,
) -> Result<String, CourseAgentError> {
    let fence = Fence::new();
    let prompt = render(
        RETEACH,
        &[
            (
                "packet",
                &format!(
                    "{PACKET_NOTE}{}",
                    fence.wrap(&render_packet("", lesson, state, None))
                ),
            ),
            ("explain", &fence.wrap(&lesson.explain)),
        ],
    );
    let text = ask(engine, prompt).await?;
    let text = text.trim();
    if text.is_empty() {
        return Err(CourseAgentError::Parse("empty explanation".into()));
    }
    Ok(text.to_string())
}

#[cfg(test)]
mod tests;
