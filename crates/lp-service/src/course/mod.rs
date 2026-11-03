//! The course service: placement, the background Author, the stored-lesson session runner,
//! code-graded answers on shared FSRS cards, follow-ups, refills and the strong-call budget.
//!
//! Spec: `brain/cheap-tutor/{architecture,memory-system,model-routing,hour-session}.md`. Everything a
//! step needs is stored, so `next` and `answer` never call a model; the only model calls are placement
//! (cheap), the Author and refills (strong, at most [`STRONG_CAP`] per rolling hour per workspace),
//! follow-ups, answer checks and re-teaching (cheap). Blocking store work runs in `spawn_blocking`,
//! one lock per course so concurrent answers lose nothing.

mod progress;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use lp_agent::course::{
    author_two_stage, check_answer, follow_up, refill, reteach, write_course, AuthorPath, Check,
    PlacementResult,
};
use lp_cli_driver::accounts::AccountPool;
use lp_cli_driver::engine::LlmEngine;
use lp_core::course::{
    fold_learner, match_answer, next_step, rating_for, rating_from_grade, render_packet,
    wants_refill, AskKind, Course, CourseEvent, Item, ItemKind, Lesson, Match, Step,
};
use lp_core::ports::{FlashCard, StoreError, TrackerStore};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};

use crate::chat::runtime::MAX_TEXT_CHARS;
use crate::chat::Engines;
use crate::engine::CliPoolEngine;
use crate::review;
use crate::settings::Settings;
use crate::stores::Stores;
use progress::{lesson_round, pending_reexplain, Round};

/// Strong (Author) calls allowed per rolling hour per workspace.
pub const STRONG_CAP: usize = 2;
const HOUR_MS: i64 = 3_600_000;
/// After a failed refill, wait this long before trying again.
const REFILL_BACKOFF_MS: i64 = 300_000;
/// A session ends after this much silence; the next step opens a new one with due reviews first.
const SESSION_IDLE_MS: i64 = 1_800_000;
/// Session-start reviews served before lessons go on (matches `next_step`'s cap).
const START_REVIEWS: usize = 10;
/// Lessons written per Author call.
const LESSONS: usize = 10;
/// Cards for unstudied items are parked here until their first review replays real FSRS state.
const PARKED_DUE: i64 = 253_402_300_799_000;
const KEEP_CHARS: usize = 500;

// ------------------------------------------------------------------ public types

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum CourseError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    /// No AI could answer. Nothing was left behind.
    #[error("{0}")]
    Unavailable(String),
    #[error("storage failure: {0}")]
    Store(String),
}

impl From<StoreError> for CourseError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::NotFound(m) => Self::NotFound(m),
            StoreError::Backend(m) => Self::Store(m),
        }
    }
}

impl From<std::io::Error> for CourseError {
    fn from(e: std::io::Error) -> Self {
        Self::Store(e.to_string())
    }
}

type Res<T> = Result<T, CourseError>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CourseStatus {
    Writing,
    Ready,
    Failed {
        reason: String,
    },
    /// A refill is running or queued behind the strong-call cap; study goes on meanwhile.
    Refilling,
}

/// A placement question as the learner sees it: never the answer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlacementItem {
    pub id: String,
    pub kind: ItemKind,
    pub q: String,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Created {
    pub id: String,
    pub items: Vec<PlacementItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementAnswer {
    pub item: String,
    pub text: String,
}

/// One graded placement answer, shown right after submitting.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Graded {
    pub item: String,
    pub correct: bool,
    pub answer: String,
    pub why: String,
}

/// A placement miss, shown as the first mini lesson while the course is being written.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Miss {
    pub q: String,
    pub answer: String,
    pub why: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LessonView {
    pub id: String,
    pub title: String,
    pub objective: String,
    pub explain: String,
    pub key_facts: Vec<String>,
}

/// A question as the learner sees it: never the answer or the `why`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ItemView {
    pub id: String,
    pub lesson: String,
    pub kind: ItemKind,
    pub q: String,
    pub options: Vec<String>,
    pub ask: AskKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepView {
    Explain(LessonView),
    Ask(ItemView),
    /// Nothing stored is left (or the course is still being written); a refill is on its way.
    Waiting,
    Done,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Feedback {
    pub correct: bool,
    /// A typed answer that is near the right one: the learner confirms with a self-grade.
    pub close: bool,
    pub answer: String,
    pub why: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Usage {
    pub strong_in: u64,
    pub strong_out: u64,
    pub cheap_in: u64,
    pub cheap_out: u64,
    /// Strong calls in the last rolling hour, across the workspace.
    pub strong_calls_this_hour: u32,
    /// Set while a refill waits for a strong slot: when one frees up (epoch ms).
    pub refill_waiting_until: Option<i64>,
}

// ------------------------------------------------------------------ persisted files

#[derive(Serialize, Deserialize)]
struct Placement {
    goal: String,
    items: Vec<Item>,
    /// `None` until submitted.
    results: Option<Vec<Result1>>,
}

#[derive(Clone, Serialize, Deserialize)]
struct Result1 {
    item: String,
    given: String,
    correct: bool,
}

#[derive(Serialize, Deserialize)]
struct Persisted {
    state: String,
    #[serde(default)]
    reason: String,
}

// ------------------------------------------------------------------ the service

struct Session {
    last: i64,
    start: bool,
    served: usize,
}

#[derive(Default)]
struct Rt {
    writing: HashSet<String>,
    refilling: HashSet<String>,
    reteaching: HashSet<(String, String)>,
    sessions: HashMap<String, Session>,
    not_before: HashMap<String, i64>,
    waiting: HashMap<String, i64>,
    inflight_strong: usize,
}

type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;

struct Inner<E> {
    root: PathBuf,
    stores: Arc<Stores>,
    engines: Engines<E>,
    device: String,
    clock: Clock,
    rt: Mutex<Rt>,
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
}

pub struct CourseService<E> {
    inner: Arc<Inner<E>>,
}

impl<E> Clone for CourseService<E> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

/// Role engines on the CLI pool: each call reads the current settings (course roles default in), so
/// a profile change applies to the next call.
pub fn cli_engines(pool: Arc<AccountPool>, data_root: PathBuf) -> Engines<CliPoolEngine> {
    Arc::new(move |role: &str| {
        let s = Settings::load(&data_root).unwrap_or_else(|_| {
            let mut s = Settings::default();
            s.fill_course_roles();
            s
        });
        Arc::new(CliPoolEngine::for_role(pool.clone(), &s, role, s.pin(role)))
    })
}

fn valid_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 128
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-')
}

fn keep(s: &str) -> String {
    s.chars().take(KEEP_CHARS).collect()
}

fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = path.with_extension(format!("tmp{:08x}", rand::random::<u32>()));
    std::fs::write(&tmp, bytes)?;
    std::fs::rename(&tmp, path)
}

async fn blk<T: Send + 'static>(f: impl FnOnce() -> Res<T> + Send + 'static) -> Res<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| CourseError::Store(e.to_string()))?
}

fn card_id(course: &str, item: &str) -> String {
    format!("c:{course}:{item}")
}

fn same(a: &str, b: &str) -> bool {
    a.trim().eq_ignore_ascii_case(b.trim())
}

/// The review card for one course item. Multiple choice keeps its options. A recall item gets its
/// answer plus three other answers of the course as options (so Review, which is multiple choice,
/// can show it); with fewer than three distinct other answers it has no card.
fn flash_card(course: &Course, lesson: &Lesson, item: &Item) -> Option<FlashCard> {
    let options: Vec<String> = match item.kind {
        ItemKind::Mcq => item.options.clone(),
        ItemKind::Recall => {
            let mut opts = vec![item.answer.clone()];
            let others = lesson
                .items
                .iter()
                .chain(course.lessons.iter().flat_map(|l| &l.items))
                .map(|i| &i.answer);
            for a in others {
                if opts.len() == 4 {
                    break;
                }
                if !opts.iter().any(|o| same(o, a)) && !a.trim().is_empty() {
                    opts.push(a.clone());
                }
            }
            if opts.len() < 4 {
                return None;
            }
            let at = item.id.bytes().map(usize::from).sum::<usize>() % 4;
            opts.swap(0, at);
            opts
        }
    };
    let correct = options.iter().position(|o| o == &item.answer)?;
    let options: [String; 4] = options.try_into().ok()?;
    Some(FlashCard {
        id: card_id(&course.id, &item.id).into(),
        front: item.q.clone(),
        options,
        correct_index: u8::try_from(correct).ok()?,
        rationale: item.why.clone(),
        card: lp_core::fsrs::Card::new(PARKED_DUE),
    })
}

impl<E: LlmEngine + 'static> CourseService<E> {
    /// `root` is the workspace root that `stores` was opened on. Courses left in `Writing` by a
    /// previous run (no task of this service is writing them) become `Failed` so they can be retried.
    pub fn new(root: PathBuf, stores: Arc<Stores>, engines: Engines<E>, device: String) -> Self {
        let inner = Arc::new(Inner {
            root,
            stores,
            engines,
            device,
            clock: Arc::new(crate::now_ms),
            rt: Mutex::default(),
            locks: Mutex::default(),
        });
        inner.recover();
        Self { inner }
    }

    /// Replaces the wall clock (epoch ms), for tests.
    pub fn with_clock(mut self, clock: Clock) -> Self {
        if let Some(i) = Arc::get_mut(&mut self.inner) {
            i.clock = clock;
        }
        self
    }

    /// Readable courses, newest first.
    pub async fn list(&self) -> Res<Vec<lp_store::CourseSummary>> {
        let me = self.inner.clone();
        blk(move || Ok(me.stores.courses.list()?)).await
    }

    // -------------------------------------------------------------- placement

    /// Runs placement. If no AI can answer this returns `Unavailable` and leaves nothing behind.
    pub async fn create(&self, goal: &str) -> Res<Created> {
        let goal = goal.trim().to_string();
        if goal.is_empty() || goal.chars().count() > MAX_TEXT_CHARS {
            return Err(CourseError::Invalid(format!(
                "the goal must be 1 to {MAX_TEXT_CHARS} characters"
            )));
        }
        let engine = (self.inner.engines)("placer");
        let before = engine.usage();
        let items = lp_agent::course::place(&*engine, &goal)
            .await
            .map_err(|e| CourseError::Unavailable(format!("No AI could write placement: {e}")))?;
        let used = delta(before, engine.usage());
        let me = self.inner.clone();
        blk(move || {
            let now = (me.clock)();
            let id = format!("c{now:x}{:04x}", rand::random::<u16>());
            let p = Placement {
                goal,
                items,
                results: None,
            };
            let view = p
                .items
                .iter()
                .map(|i| PlacementItem {
                    id: i.id.clone(),
                    kind: i.kind,
                    q: i.q.clone(),
                    options: i.options.clone(),
                })
                .collect();
            me.write_json(&id, "placement.json", &p)?;
            me.usage_event(&id, "placer", used)?;
            Ok(Created { id, items: view })
        })
        .await
    }

    /// Grades placement with code, stores the results, saves the learner profile and starts the
    /// Author in the background. Returns at once.
    pub async fn submit_placement(
        &self,
        id: &str,
        answers: Vec<PlacementAnswer>,
    ) -> Res<Vec<Graded>> {
        let (me, cid) = (self.inner.clone(), id.to_string());
        let graded = blk(move || {
            let lk = me.lock_for(&cid);
            let _g = lk.lock();
            let mut p: Placement = me.read_json(&cid, "placement.json")?;
            if p.results.is_some() {
                return Err(CourseError::Invalid(
                    "placement was already submitted".into(),
                ));
            }
            let mut results = Vec::new();
            let mut graded = Vec::new();
            for it in &p.items {
                let given = answers
                    .iter()
                    .find(|a| a.item == it.id)
                    .map(|a| keep(&a.text))
                    .unwrap_or_default();
                let correct = grade(it, &given).0;
                graded.push(Graded {
                    item: it.id.clone(),
                    correct,
                    answer: it.answer.clone(),
                    why: it.why.clone(),
                });
                results.push(Result1 {
                    item: it.id.clone(),
                    given,
                    correct,
                });
            }
            let right = results.iter().filter(|r| r.correct).count();
            let profile = format!(
                "Goal: {}\nPlacement: {right} of {} right.",
                p.goal,
                results.len()
            );
            me.stores.courses.save_learner(&profile)?;
            p.results = Some(results);
            me.write_json(&cid, "placement.json", &p)?;
            me.rt.lock().writing.insert(cid.clone());
            me.set_state(&cid, "writing", "")?;
            Ok(graded)
        })
        .await?;
        self.spawn_write(id);
        Ok(graded)
    }

    /// The placement misses with their explanations: the first mini lesson while the Author writes.
    pub async fn placement_misses(&self, id: &str) -> Res<Vec<Miss>> {
        let (me, cid) = (self.inner.clone(), id.to_string());
        blk(move || {
            let p: Placement = me.read_json(&cid, "placement.json")?;
            let results = p.results.unwrap_or_default();
            Ok(results
                .iter()
                .filter(|r| !r.correct)
                .filter_map(|r| p.items.iter().find(|i| i.id == r.item))
                .map(|i| Miss {
                    q: i.q.clone(),
                    answer: i.answer.clone(),
                    why: i.why.clone(),
                })
                .collect())
        })
        .await
    }

    pub async fn status(&self, id: &str) -> Res<CourseStatus> {
        let (me, cid) = (self.inner.clone(), id.to_string());
        blk(move || me.status_of(&cid)).await
    }

    /// Writes a `Failed` course again from its stored placement.
    pub async fn retry(&self, id: &str) -> Res<()> {
        let (me, cid) = (self.inner.clone(), id.to_string());
        blk(move || {
            let lk = me.lock_for(&cid);
            let _g = lk.lock();
            if !matches!(me.status_of(&cid)?, CourseStatus::Failed { .. }) {
                return Err(CourseError::Invalid(
                    "the course is not in a failed state".into(),
                ));
            }
            let p: Placement = me.read_json(&cid, "placement.json")?;
            if p.results.is_none() {
                return Err(CourseError::Invalid("placement was not submitted".into()));
            }
            me.rt.lock().writing.insert(cid.clone());
            me.set_state(&cid, "writing", "")
        })
        .await?;
        self.spawn_write(id);
        Ok(())
    }

    fn spawn_write(&self, id: &str) {
        let (me, cid) = (self.inner.clone(), id.to_string());
        tokio::spawn(async move {
            let outcome = run_write(&me, &cid).await;
            let (m2, c2) = (me.clone(), cid.clone());
            let _ = blk(move || {
                let state = match &outcome {
                    Ok(()) => ("ready", String::new()),
                    Err(e) => ("failed", e.to_string()),
                };
                // Under the course lock, so a retry cannot slip in between the two.
                let lk = m2.lock_for(&c2);
                let _g = lk.lock();
                let r = m2.set_state(&c2, state.0, &state.1);
                m2.rt.lock().writing.remove(&c2);
                r
            })
            .await;
        });
    }

    // -------------------------------------------------------------- session

    /// The next step. An explanation is logged as shown the moment it is returned. Study never waits
    /// for a model: when stored lessons run out this returns `Waiting` and a refill starts.
    pub async fn next(&self, id: &str) -> Res<StepView> {
        let (me, cid) = (self.inner.clone(), id.to_string());
        let (view, refill_wanted) = blk(move || me.next_sync(&cid)).await?;
        if refill_wanted {
            self.maybe_refill(id).await;
        }
        Ok(view)
    }

    /// Grades with code, never a model, and never reveals the answer before this call.
    pub async fn answer(
        &self,
        id: &str,
        item: &str,
        text: &str,
        confidence: Option<u8>,
        ms: Option<u64>,
    ) -> Res<Feedback> {
        if text.chars().count() > MAX_TEXT_CHARS {
            return Err(CourseError::Invalid(format!(
                "the answer must be at most {MAX_TEXT_CHARS} characters"
            )));
        }
        if confidence.is_some_and(|c| !(1..=5).contains(&c)) {
            return Err(CourseError::Invalid("confidence is 1 to 5".into()));
        }
        let (me, cid, iid, text) = (
            self.inner.clone(),
            id.to_string(),
            item.to_string(),
            text.to_string(),
        );
        let (fb, reteach_lesson) =
            blk(move || me.answer_sync(&cid, &iid, &text, confidence, ms)).await?;
        if let Some(l) = reteach_lesson {
            self.spawn_reteach(id, &l);
        }
        Ok(fb)
    }

    /// The learner's own grade for a recall item (`again|hard|good|easy`).
    pub async fn self_grade(&self, id: &str, item: &str, grade: &str) -> Res<()> {
        let rating = rating_from_grade(grade)
            .ok_or_else(|| CourseError::Invalid("grade is again, hard, good or easy".into()))?;
        let (me, cid, iid, g) = (
            self.inner.clone(),
            id.to_string(),
            item.to_string(),
            grade.trim().to_lowercase(),
        );
        let lesson = blk(move || {
            let lk = me.lock_for(&cid);
            let _g = lk.lock();
            let course = me.load(&cid)?;
            me.find_item(&course, &iid)?;
            let ev = CourseEvent::SelfGraded {
                ts: (me.clock)(),
                item: iid.clone(),
                grade: g,
            };
            me.record_outcome(&course, &iid, ev, Some(rating))
        })
        .await?;
        if let Some(l) = lesson {
            self.spawn_reteach(id, &l);
        }
        Ok(())
    }

    pub async fn flag(&self, id: &str, item: &str, reason: &str) -> Res<()> {
        let (me, cid, iid, reason) = (
            self.inner.clone(),
            id.to_string(),
            item.to_string(),
            keep(reason),
        );
        blk(move || {
            let lk = me.lock_for(&cid);
            let _g = lk.lock();
            let course = me.load(&cid)?;
            me.find_item(&course, &iid)?;
            me.append(
                &cid,
                &CourseEvent::Flagged {
                    ts: (me.clock)(),
                    item: iid,
                    reason,
                },
            )
        })
        .await
    }

    // -------------------------------------------------------------- cheap model roles

    /// A short answer to a learner question, from the fixed-size packet. The question is saved even
    /// when no AI can answer now.
    pub async fn ask(&self, id: &str, question: &str) -> Res<String> {
        let question = question.trim().to_string();
        if question.is_empty() || question.chars().count() > MAX_TEXT_CHARS {
            return Err(CourseError::Invalid(format!(
                "the question must be 1 to {MAX_TEXT_CHARS} characters"
            )));
        }
        let (me, cid) = (self.inner.clone(), id.to_string());
        let (packet, lesson) = blk(move || me.packet_of(&cid)).await?;
        let engine = (self.inner.engines)("follow_up");
        let before = engine.usage();
        let reply = follow_up(&*engine, &packet, &question).await;
        let used = delta(before, engine.usage());
        let (me, cid) = (self.inner.clone(), id.to_string());
        let out = reply
            .as_ref()
            .map(|t| t.chars().count() as u32)
            .unwrap_or(0);
        blk(move || {
            let lk = me.lock_for(&cid);
            let _g = lk.lock();
            me.append(
                &cid,
                &CourseEvent::Asked {
                    ts: (me.clock)(),
                    lesson,
                    question: keep(&question),
                    answer_chars: out,
                },
            )?;
            me.usage_event(&cid, "follow_up", used)
        })
        .await?;
        reply.map_err(|_| {
            CourseError::Unavailable(
                "I can't answer questions right now, but you can keep studying.".into(),
            )
        })
    }

    /// Asks a cheap model whether a typed answer is right ("check my answer").
    pub async fn check(&self, id: &str, item: &str, text: &str) -> Res<Check> {
        if text.chars().count() > MAX_TEXT_CHARS {
            return Err(CourseError::Invalid(format!(
                "the answer must be at most {MAX_TEXT_CHARS} characters"
            )));
        }
        let (me, cid, iid) = (self.inner.clone(), id.to_string(), item.to_string());
        let it = blk(move || {
            let course = me.load(&cid)?;
            Ok(me.find_item(&course, &iid)?.1.clone())
        })
        .await?;
        let engine = (self.inner.engines)("follow_up");
        let before = engine.usage();
        let r = check_answer(&*engine, &it, text).await;
        let used = delta(before, engine.usage());
        let (me, cid) = (self.inner.clone(), id.to_string());
        blk(move || me.usage_event(&cid, "follow_up", used)).await?;
        r.map_err(|e| CourseError::Unavailable(format!("No AI could check that answer: {e}")))
    }

    fn spawn_reteach(&self, id: &str, lesson: &str) {
        let (me, cid, lid) = (self.inner.clone(), id.to_string(), lesson.to_string());
        let engines = self.inner.engines.clone();
        tokio::spawn(async move {
            let (m2, c2, l2) = (me.clone(), cid.clone(), lid.clone());
            let inputs = blk(move || {
                let course = m2.load(&c2)?;
                let state = fold_learner(&course, &m2.events(&c2)?);
                Ok((m2.find_lesson(&course, &l2)?.clone(), state))
            })
            .await;
            let mut text = None;
            let mut used = (0, 0);
            if let Ok((lesson, state)) = inputs {
                let engine = engines("follow_up");
                let before = engine.usage();
                text = reteach(&*engine, &lesson, &state).await.ok();
                used = delta(before, engine.usage());
            }
            let _ = blk(move || {
                let lk = me.lock_for(&cid);
                let _g = lk.lock();
                if let Some(t) = text {
                    let mut course = me.load(&cid)?;
                    if let Some(l) = course.lessons.iter_mut().find(|l| l.id == lid) {
                        l.explain = t;
                    }
                    me.stores.courses.save_course(&course)?;
                }
                me.usage_event(&cid, "follow_up", used)?;
                me.rt.lock().reteaching.remove(&(cid.clone(), lid));
                Ok(())
            })
            .await;
        });
    }

    // -------------------------------------------------------------- usage and refill

    pub async fn usage(&self, id: &str) -> Res<Usage> {
        let (me, cid) = (self.inner.clone(), id.to_string());
        blk(move || {
            let mut u = Usage::default();
            for ev in me.events(&cid)? {
                if let CourseEvent::Usage {
                    role,
                    tokens_in,
                    tokens_out,
                    ..
                } = ev
                {
                    if role == "author" {
                        u.strong_in += tokens_in;
                        u.strong_out += tokens_out;
                    } else {
                        u.cheap_in += tokens_in;
                        u.cheap_out += tokens_out;
                    }
                }
            }
            let now = (me.clock)();
            u.strong_calls_this_hour = me.strong_stamps(now).len() as u32;
            u.refill_waiting_until = me.rt.lock().waiting.get(&cid).copied();
            Ok(u)
        })
        .await
    }

    /// Starts a background refill unless one runs, it failed recently, or the strong cap is full
    /// (then it is queued: the wait is shown in `usage`, and a later `next` starts it).
    async fn maybe_refill(&self, id: &str) {
        let (me, cid) = (self.inner.clone(), id.to_string());
        let go = blk(move || Ok(me.begin_refill(&cid)))
            .await
            .unwrap_or(false);
        if !go {
            return;
        }
        let (me, cid) = (self.inner.clone(), id.to_string());
        tokio::spawn(async move {
            let r = run_refill(&me, &cid).await;
            let _ = blk(move || {
                let mut rt = me.rt.lock();
                rt.inflight_strong = rt.inflight_strong.saturating_sub(1);
                rt.refilling.remove(&cid);
                if r.is_err() {
                    let now = (me.clock)();
                    rt.not_before.insert(cid, now + REFILL_BACKOFF_MS);
                }
                Ok(())
            })
            .await;
        });
    }
}

fn delta(before: (u64, u64), after: (u64, u64)) -> (u64, u64) {
    (
        after.0.saturating_sub(before.0),
        after.1.saturating_sub(before.1),
    )
}

/// `(correct, close)` for a typed or chosen answer.
fn grade(item: &Item, text: &str) -> (bool, bool) {
    match item.kind {
        ItemKind::Mcq => (text.trim() == item.answer.trim(), false),
        ItemKind::Recall => match match_answer(item, text) {
            Match::Exact | Match::ExactButAccent => (true, false),
            Match::Close => (false, true),
            Match::Wrong => (false, false),
        },
    }
}

// ------------------------------------------------------------------ background tasks

async fn run_write<E: LlmEngine + 'static>(me: &Arc<Inner<E>>, id: &str) -> Res<()> {
    let (m2, cid) = (me.clone(), id.to_string());
    let (p, profile, reserved) = blk(move || {
        let p: Placement = m2.read_json(&cid, "placement.json")?;
        let profile = m2.stores.courses.load_learner()?;
        Ok((p, profile, m2.reserve_strong()))
    })
    .await?;
    let results: Vec<PlacementResult> = p
        .results
        .iter()
        .flatten()
        .filter_map(|r| {
            p.items
                .iter()
                .find(|i| i.id == r.item)
                .map(|i| PlacementResult {
                    item: i.clone(),
                    given: r.given.clone(),
                    correct: r.correct,
                })
        })
        .collect();
    let (strong, cheap) = ((me.engines)("author"), (me.engines)("placer"));
    let (sb, cb) = (strong.usage(), cheap.usage());
    // Over the strong cap the first course is written by the cheap two-stage path instead.
    let written = if reserved {
        write_course(&*strong, &*cheap, &p.goal, &results, &profile, LESSONS).await
    } else {
        author_two_stage(&*cheap, &p.goal, &results, &profile, LESSONS)
            .await
            .map(|authored| lp_agent::course::Written {
                authored,
                path: AuthorPath::TwoStage,
            })
    };
    let (su, cu) = (delta(sb, strong.usage()), delta(cb, cheap.usage()));
    let (m2, cid) = (me.clone(), id.to_string());
    blk(move || {
        let lk = m2.lock_for(&cid);
        let _g = lk.lock();
        let r = (|| match written {
            Ok(w) => {
                let mut course = w.authored.course;
                course.id = cid.clone();
                m2.save_course(&course)?;
                if w.path == AuthorPath::Strong {
                    m2.usage_event(&cid, "author", su)?;
                }
                m2.usage_event(&cid, "placer", cu)
            }
            Err(e) => Err(CourseError::Unavailable(format!(
                "No AI could write the course: {e}"
            ))),
        })();
        if reserved {
            m2.release_strong();
        }
        r
    })
    .await
}

async fn run_refill<E: LlmEngine + 'static>(me: &Arc<Inner<E>>, id: &str) -> Res<()> {
    let (m2, cid) = (me.clone(), id.to_string());
    let (course, state) = blk(move || {
        let c = m2.load(&cid)?;
        let st = fold_learner(&c, &m2.events(&cid)?);
        Ok((c, st))
    })
    .await?;
    let (strong, cheap) = ((me.engines)("author"), (me.engines)("placer"));
    let (sb, cb) = (strong.usage(), cheap.usage());
    let written = refill(&*strong, &*cheap, &course, &state).await;
    let (su, cu) = (delta(sb, strong.usage()), delta(cb, cheap.usage()));
    let (m2, cid) = (me.clone(), id.to_string());
    blk(move || {
        let lk = m2.lock_for(&cid);
        let _g = lk.lock();
        match written {
            Ok(w) => {
                // Study may have changed the course (a re-taught lesson): add only the new lessons.
                let mut fresh = m2.load(&cid)?;
                let known: HashSet<String> = fresh.lessons.iter().map(|l| l.id.clone()).collect();
                let added: Vec<Lesson> = w
                    .authored
                    .course
                    .lessons
                    .into_iter()
                    .filter(|l| !known.contains(&l.id))
                    .collect();
                if added.is_empty() {
                    return Err(CourseError::Unavailable("the refill added nothing".into()));
                }
                fresh.lessons.extend(added);
                fresh.version += 1;
                m2.save_course(&fresh)?;
                if w.path == AuthorPath::Strong {
                    m2.usage_event(&cid, "author", su)?;
                }
                m2.usage_event(&cid, "placer", cu)?;
                Ok(())
            }
            Err(e) => Err(CourseError::Unavailable(format!("refill failed: {e}"))),
        }
    })
    .await
}

// ------------------------------------------------------------------ blocking internals

impl<E> Inner<E> {
    fn dir(&self, id: &str) -> Res<PathBuf> {
        if valid_id(id) {
            Ok(self.root.join("courses").join(id))
        } else {
            Err(CourseError::NotFound(id.into()))
        }
    }

    fn lock_for(&self, id: &str) -> Arc<Mutex<()>> {
        self.locks.lock().entry(id.to_string()).or_default().clone()
    }

    fn read_json<T: for<'de> Deserialize<'de>>(&self, id: &str, file: &str) -> Res<T> {
        let path = self.dir(id)?.join(file);
        match std::fs::read(&path) {
            Ok(b) => serde_json::from_slice(&b).map_err(|e| CourseError::Store(e.to_string())),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(CourseError::NotFound(id.into()))
            }
            Err(e) => Err(e.into()),
        }
    }

    fn write_json<T: Serialize>(&self, id: &str, file: &str, v: &T) -> Res<()> {
        let bytes = serde_json::to_vec_pretty(v).map_err(|e| CourseError::Store(e.to_string()))?;
        Ok(write_atomic(&self.dir(id)?.join(file), &bytes)?)
    }

    fn set_state(&self, id: &str, state: &str, reason: &str) -> Res<()> {
        self.write_json(
            id,
            "status.json",
            &Persisted {
                state: state.into(),
                reason: reason.into(),
            },
        )
    }

    fn load(&self, id: &str) -> Res<Course> {
        valid_id(id)
            .then_some(())
            .ok_or_else(|| CourseError::NotFound(id.into()))?;
        Ok(self.stores.courses.load_course(id)?)
    }

    fn events(&self, id: &str) -> Res<Vec<CourseEvent>> {
        valid_id(id)
            .then_some(())
            .ok_or_else(|| CourseError::NotFound(id.into()))?;
        Ok(self.stores.courses.events(id)?)
    }

    fn append(&self, id: &str, ev: &CourseEvent) -> Res<()> {
        Ok(self.stores.courses.append_event(id, ev)?)
    }

    fn usage_event(&self, id: &str, role: &str, used: (u64, u64)) -> Res<()> {
        if used == (0, 0) {
            return Ok(());
        }
        self.append(
            id,
            &CourseEvent::Usage {
                ts: (self.clock)(),
                role: role.into(),
                model: String::new(),
                tokens_in: used.0,
                tokens_out: used.1,
            },
        )
    }

    fn find_lesson<'a>(&self, c: &'a Course, id: &str) -> Res<&'a Lesson> {
        c.lessons
            .iter()
            .find(|l| l.id == id)
            .ok_or_else(|| CourseError::NotFound(id.into()))
    }

    fn find_item<'a>(&self, c: &'a Course, id: &str) -> Res<(&'a Lesson, &'a Item)> {
        c.lessons
            .iter()
            .find_map(|l| l.items.iter().find(|i| i.id == id).map(|i| (l, i)))
            .ok_or_else(|| CourseError::NotFound(id.into()))
    }

    /// Saves the course and gives every gated item a review card, keeping cards that exist (their
    /// FSRS history stays). Unstudied cards are parked far in the future until first answered.
    fn save_course(&self, course: &Course) -> Res<()> {
        self.stores.courses.save_course(course)?;
        let t = &self.stores.tracker;
        for l in &course.lessons {
            for it in &l.items {
                let Some(card) = flash_card(course, l, it) else {
                    continue;
                };
                if t.card(&card.id)?.is_none() {
                    t.upsert_card(&card)?;
                }
            }
        }
        Ok(())
    }

    fn grade_card(&self, course: &str, item: &str, rating: lp_core::fsrs::Rating) -> Res<()> {
        match review::grade(
            &*self.stores.tracker,
            &self.device,
            &card_id(course, item),
            rating,
            (self.clock)(),
        ) {
            Ok(()) | Err(StoreError::NotFound(_)) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }

    /// Items of this course that are due, oldest first.
    fn due_items(&self, id: &str, now: i64) -> Res<Vec<String>> {
        let prefix = format!("c:{id}:");
        Ok(review::due_cards(&*self.stores.tracker, now)?
            .into_iter()
            .filter_map(|c| c.id.strip_prefix(&prefix).map(String::from))
            .collect())
    }

    fn status_of(&self, id: &str) -> Res<CourseStatus> {
        let dir = self.dir(id)?;
        if !dir.is_dir() {
            return Err(CourseError::NotFound(id.into()));
        }
        let p: Option<Persisted> = match self.read_json(id, "status.json") {
            Ok(p) => Some(p),
            Err(CourseError::NotFound(_)) => None,
            Err(e) => return Err(e),
        };
        let rt = self.rt.lock();
        Ok(match p.as_ref().map(|p| p.state.as_str()) {
            Some("writing") if rt.writing.contains(id) => CourseStatus::Writing,
            Some("writing") => CourseStatus::Failed {
                reason: "The app stopped while the course was being written.".into(),
            },
            Some("failed") => CourseStatus::Failed {
                reason: p.map(|p| p.reason).unwrap_or_default(),
            },
            _ if !dir.join("course.json").is_file() => CourseStatus::Failed {
                reason: "The course has not been written yet.".into(),
            },
            _ if rt.refilling.contains(id) || rt.waiting.contains_key(id) => {
                CourseStatus::Refilling
            }
            _ => CourseStatus::Ready,
        })
    }

    /// Courses left `writing` by a run that is gone become `failed` on disk.
    fn recover(&self) {
        let Ok(rd) = std::fs::read_dir(self.root.join("courses")) else {
            return;
        };
        for e in rd.flatten() {
            let Some(id) = e.file_name().to_str().map(String::from) else {
                continue;
            };
            if let Ok(Persisted { state, .. }) = self.read_json(&id, "status.json") {
                if state == "writing" && !self.rt.lock().writing.contains(&id) {
                    let _ = self.set_state(
                        &id,
                        "failed",
                        "The app stopped while the course was being written.",
                    );
                }
            }
        }
    }

    // ---- strong-call budget

    /// Timestamps of the Author calls in the last rolling hour, across the workspace, oldest first.
    fn strong_stamps(&self, now: i64) -> Vec<i64> {
        let Ok(rd) = std::fs::read_dir(self.root.join("courses")) else {
            return vec![];
        };
        let mut out: Vec<i64> = rd
            .flatten()
            .filter_map(|e| e.file_name().to_str().map(String::from))
            .filter_map(|id| self.events(&id).ok())
            .flatten()
            .filter_map(|ev| match ev {
                CourseEvent::Usage { ts, role, .. } if role == "author" && ts > now - HOUR_MS => {
                    Some(ts)
                }
                _ => None,
            })
            .collect();
        out.sort_unstable();
        out
    }

    /// Takes one strong slot for a call about to start; false when the cap is full.
    fn reserve_strong(&self) -> bool {
        let now = (self.clock)();
        let mut rt = self.rt.lock();
        if self.strong_stamps(now).len() + rt.inflight_strong >= STRONG_CAP {
            return false;
        }
        rt.inflight_strong += 1;
        true
    }

    fn release_strong(&self) {
        let mut rt = self.rt.lock();
        rt.inflight_strong = rt.inflight_strong.saturating_sub(1);
    }

    /// True when a refill task should start now (its strong slot is reserved). When the cap is full
    /// the refill is queued: `waiting` holds when a slot frees.
    fn begin_refill(&self, id: &str) -> bool {
        let now = (self.clock)();
        let mut rt = self.rt.lock();
        if rt.refilling.contains(id) || rt.not_before.get(id).is_some_and(|t| *t > now) {
            return false;
        }
        let stamps = self.strong_stamps(now);
        if stamps.len() + rt.inflight_strong >= STRONG_CAP {
            let free = stamps
                .len()
                .checked_sub(STRONG_CAP)
                .and_then(|i| stamps.get(i))
                .map_or(now + 5_000, |t| t + HOUR_MS);
            rt.waiting.insert(id.to_string(), free);
            return false;
        }
        rt.waiting.remove(id);
        rt.inflight_strong += 1;
        rt.refilling.insert(id.to_string());
        true
    }

    // ---- session

    /// `(step, refill wanted)`.
    fn next_sync(&self, id: &str) -> Res<(StepView, bool)> {
        let lk = self.lock_for(id);
        let _g = lk.lock();
        let course = match self.load(id) {
            Ok(c) => c,
            Err(CourseError::NotFound(_)) => {
                // Still being written (or failed): nothing to study yet.
                return if self.dir(id)?.is_dir() {
                    Ok((StepView::Waiting, false))
                } else {
                    Err(CourseError::NotFound(id.into()))
                };
            }
            Err(e) => return Err(e),
        };
        let events = self.events(id)?;
        let state = fold_learner(&course, &events);
        let now = (self.clock)();
        let reteaching: HashSet<String> = self
            .rt
            .lock()
            .reteaching
            .iter()
            .filter(|(c, _)| c == id)
            .map(|(_, l)| l.clone())
            .collect();
        let step = match pending_reexplain(&course, &events, &reteaching) {
            Some(lesson) => Step::Explain { lesson },
            None => {
                let start = {
                    let mut rt = self.rt.lock();
                    let s = rt.sessions.entry(id.to_string()).or_insert(Session {
                        last: i64::MIN,
                        start: true,
                        served: 0,
                    });
                    if now.saturating_sub(s.last) > SESSION_IDLE_MS {
                        *s = Session {
                            last: now,
                            start: true,
                            served: 0,
                        };
                    }
                    s.last = now;
                    s.start
                };
                let due = if start {
                    self.due_items(id, now)?
                } else {
                    vec![]
                };
                let step = next_step(&course, &state, &events, &due, start);
                if start {
                    let mut rt = self.rt.lock();
                    if let Some(s) = rt.sessions.get_mut(id) {
                        match step {
                            Step::Ask {
                                kind: AskKind::Review,
                                ..
                            } => {
                                s.served += 1;
                                s.start = s.served < START_REVIEWS;
                            }
                            _ => s.start = false,
                        }
                    }
                }
                step
            }
        };
        let refill_wanted = wants_refill(&course, &state);
        let view = match step {
            Step::Explain { lesson } => {
                let l = self.find_lesson(&course, &lesson)?;
                self.append(
                    id,
                    &CourseEvent::Shown {
                        ts: now,
                        lesson: lesson.clone(),
                    },
                )?;
                StepView::Explain(LessonView {
                    id: l.id.clone(),
                    title: l.title.clone(),
                    objective: l.objective.clone(),
                    explain: l.explain.clone(),
                    key_facts: l.key_facts.clone(),
                })
            }
            Step::Ask { item, kind } => {
                let (l, it) = self.find_item(&course, &item)?;
                StepView::Ask(ItemView {
                    id: it.id.clone(),
                    lesson: l.id.clone(),
                    kind: it.kind,
                    q: it.q.clone(),
                    options: it.options.clone(),
                    ask: kind,
                })
            }
            Step::Refill => StepView::Waiting,
            Step::Done => StepView::Done,
        };
        Ok((view, refill_wanted))
    }

    fn answer_sync(
        &self,
        id: &str,
        item: &str,
        text: &str,
        confidence: Option<u8>,
        ms: Option<u64>,
    ) -> Res<(Feedback, Option<String>)> {
        let lk = self.lock_for(id);
        let _g = lk.lock();
        let course = self.load(id)?;
        let (_, it) = self.find_item(&course, item)?;
        let (correct, close) = grade(it, text);
        let ev = CourseEvent::Answered {
            ts: (self.clock)(),
            item: item.into(),
            correct,
            chosen: keep(text),
            confidence,
            ms,
        };
        // A typed answer that is not an exact hit waits for the learner's own grade before it is
        // scheduled, so one answer makes one FSRS review.
        let rating = (it.kind == ItemKind::Mcq || correct).then(|| rating_for(correct, confidence));
        let reteach = self.record_outcome(&course, item, ev, rating)?;
        Ok((
            Feedback {
                correct,
                close,
                answer: it.answer.clone(),
                why: it.why.clone(),
            },
            reteach,
        ))
    }

    /// Appends an answer-like event, reviews the item's card, and closes the lesson when all its
    /// items have been answered. Returns a lesson to re-teach with a cheap call, if one is due.
    fn record_outcome(
        &self,
        course: &Course,
        item: &str,
        ev: CourseEvent,
        rating: Option<lp_core::fsrs::Rating>,
    ) -> Res<Option<String>> {
        let id = course.id.as_str();
        let mut events = self.events(id)?;
        self.append(id, &ev)?;
        events.push(ev);
        if let Some(r) = rating {
            self.grade_card(id, item, r)?;
        }
        let (lesson, _) = self.find_item(course, item)?;
        let flagged = fold_learner(course, &events).flagged;
        let now = (self.clock)();
        let Some(round) = lesson_round(lesson, &flagged, &events) else {
            return Ok(None);
        };
        let (score, second) = match round {
            Round::First(s) => (s, false),
            Round::Second(s) => (s, true),
        };
        self.append(
            id,
            &CourseEvent::LessonDone {
                ts: now,
                lesson: lesson.id.clone(),
                correct: score.correct,
                total: score.total,
            },
        )?;
        if second && score.correct * 2 < score.total {
            self.rt
                .lock()
                .reteaching
                .insert((id.to_string(), lesson.id.clone()));
            return Ok(Some(lesson.id.clone()));
        }
        Ok(None)
    }

    /// The context packet for a follow-up and the lesson it is about.
    fn packet_of(&self, id: &str) -> Res<(String, String)> {
        let course = self.load(id)?;
        let events = self.events(id)?;
        let state = fold_learner(&course, &events);
        let lesson_id = events.iter().rev().find_map(|e| match e {
            CourseEvent::Shown { lesson, .. } => Some(lesson.clone()),
            CourseEvent::Answered { item, .. } => self
                .find_item(&course, item)
                .ok()
                .map(|(l, _)| l.id.clone()),
            _ => None,
        });
        let lesson = lesson_id
            .and_then(|l| self.find_lesson(&course, &l).ok())
            .or_else(|| course.lessons.first())
            .ok_or_else(|| CourseError::NotFound(id.into()))?;
        let last = events.iter().rev().find_map(|e| match e {
            CourseEvent::Answered { item, chosen, .. } => lesson
                .items
                .iter()
                .find(|i| &i.id == item)
                .map(|i| (i.q.as_str(), chosen.as_str())),
            _ => None,
        });
        Ok((
            render_packet(&course.learner, lesson, &state, last),
            lesson.id.clone(),
        ))
    }
}
