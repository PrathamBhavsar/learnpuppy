use std::collections::HashSet;
use std::sync::atomic::{AtomicI64, AtomicUsize, Ordering};

use async_trait::async_trait;
use lp_cli_driver::engine::{
    DriverError, LlmEngine, StreamDelta, StructuredOutput, StructuredRequest, TurnRequest,
    TurnResponse,
};
use lp_core::course::AskKind;
use lp_core::fsrs::{Rating, State};
use lp_core::ports::TrackerStore;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::json;
use tokio::sync::{mpsc, Semaphore};
use tokio_util::sync::CancellationToken;

use super::*;

const T0: i64 = 1_800_000_000_000;
const DAY: i64 = 86_400_000;
const WHY: &str = "I want to read other people's code";

// ------------------------------------------------------------------ scripted engine

/// State shared by every engine of one workspace: the call log and the faults to inject.
struct Shared {
    /// Roles of the model calls, in order.
    calls: Mutex<Vec<String>>,
    /// Roles that fail with a rate limit; `*` means all of them.
    limited: Mutex<HashSet<String>>,
    /// Roles that answer with unusable text (tokens are still spent).
    garbage: Mutex<HashSet<String>>,
    /// The Author waits for a permit (tests start it empty to hold the course in `Writing`).
    gate: Semaphore,
    batch: AtomicUsize,
    /// The strand probe call answers with unusable text, so placement falls back to the plain quiz.
    plain_only: std::sync::atomic::AtomicBool,
    /// Every clock read moves time on by 1 ms (so concurrent reviews get distinct timestamps).
    tick: std::sync::atomic::AtomicBool,
}

impl Shared {
    fn new(hold: bool) -> Arc<Self> {
        Arc::new(Self {
            calls: Mutex::default(),
            limited: Mutex::default(),
            garbage: Mutex::default(),
            gate: Semaphore::new(if hold { 0 } else { 10_000 }),
            batch: AtomicUsize::new(0),
            plain_only: std::sync::atomic::AtomicBool::new(false),
            tick: std::sync::atomic::AtomicBool::new(false),
        })
    }
    fn count(&self, role: &str) -> usize {
        self.calls.lock().iter().filter(|r| *r == role).count()
    }
    fn total(&self) -> usize {
        self.calls.lock().len()
    }
    fn limit(&self, role: &str, on: bool) {
        let mut l = self.limited.lock();
        if on {
            l.insert(role.into());
        } else {
            l.remove(role);
        }
    }
}

struct Mock {
    role: String,
    shared: Arc<Shared>,
    calls: AtomicUsize,
}

fn lesson_json(id: &str) -> serde_json::Value {
    let items: Vec<_> = (0..6)
        .map(|i| {
            if i < 4 {
                json!({"type":"mcq","q":format!("Which statement about {id} number {i} holds?"),
                    "options":["red apple","blue plum","pink pear","gray kiwi"],
                    "answer":"red apple","why":format!("because {id} {i}"),"tests":0})
            } else {
                json!({"type":"recall","q":format!("State fact {i} of {id}."),
                    "answer":format!("answer {id} {i}"),"accept":["a fact"],
                    "why":format!("because {id} {i}"),"tests":1})
            }
        })
        .collect();
    json!({"id":id,"title":format!("Title {id}"),"objective":"obj",
        "explain":"red apple and the fact are explained here",
        "key_facts":["red apple","the fact"],"fixes":[],"items":items})
}

fn course_reply(batch: usize, n: usize) -> String {
    let lessons: Vec<_> = (0..n)
        .map(|i| lesson_json(&format!("b{batch}l{i}")))
        .collect();
    json!({"learner":"knows little","lessons":lessons}).to_string()
}

fn place_reply() -> String {
    let mut items: Vec<_> = (0..3)
        .map(|i| {
            json!({"type":"mcq","q":format!("Which statement about placement number {i} holds?"),
                "options":["red apple","blue plum","pink pear","gray kiwi"],
                "answer":"red apple","why":format!("placement why {i}")})
        })
        .collect();
    for i in 3..5 {
        items.push(json!({"type":"recall","q":format!("State placement fact {i}."),
            "answer":format!("placement answer {i}"),"accept":[],"why":format!("placement why {i}")}));
    }
    json!({"items":items}).to_string()
}

/// Three strands, each with an mcq at levels 1 to 3; every answer is "red apple".
fn strand_reply() -> String {
    let strands: Vec<_> = (0..3)
        .map(|si| {
            let items: Vec<_> = (1..=3)
                .map(|l| {
                    json!({"type":"mcq","q":format!("Which statement about strand {si} level {l} holds?"),
                        "options":["red apple","blue plum","pink pear","gray kiwi"],
                        "answer":"red apple","why":format!("placement why s{si} l{l}"),"level":l})
                })
                .collect();
            json!({"name":format!("Strand {si}"),"items":items})
        })
        .collect();
    json!({"strands":strands}).to_string()
}

#[async_trait]
impl LlmEngine for Mock {
    async fn stream_turn(
        &self,
        req: TurnRequest,
        _tx: mpsc::Sender<StreamDelta>,
        _c: CancellationToken,
    ) -> Result<TurnResponse, DriverError> {
        self.shared.calls.lock().push(self.role.clone());
        {
            let l = self.shared.limited.lock();
            if l.contains("*") || l.contains(&self.role) {
                return Err(DriverError::Failed("rate limit reached".into()));
            }
        }
        // Only calls that were served spend tokens.
        self.calls.fetch_add(1, Ordering::Relaxed);
        if self.shared.garbage.lock().contains(&self.role) {
            return Ok(TurnResponse {
                text: "this is not json".into(),
                tool_calls: vec![],
            });
        }
        let p = req.prompt;
        let text = if p.contains("Write a strand probe") {
            if self.shared.plain_only.load(Ordering::Relaxed) {
                "not json".into()
            } else {
                strand_reply()
            }
        } else if p.contains("Write 5 placement questions") {
            place_reply()
        } else if p.contains("Plan a course and write") {
            self.shared.gate.acquire().await.unwrap().forget();
            course_reply(self.shared.batch.fetch_add(1, Ordering::Relaxed), 10)
        } else if p.contains("Plan a course for this learner") {
            let b = self.shared.batch.fetch_add(1, Ordering::Relaxed);
            let lessons: Vec<_> = (0..2)
                .map(|i| {
                    json!({"id":format!("t{b}l{i}"),"title":format!("T{b}{i}"),"objective":"o",
                    "teach_notes":"n","key_facts":["red apple","the fact"],"fixes":[]})
                })
                .collect();
            json!({"learner":"x","must_fix":[],"lessons":lessons}).to_string()
        } else if p.contains("Write one lesson") {
            let b = self.shared.batch.fetch_add(1, Ordering::Relaxed);
            let l = lesson_json(&format!("p{b}"));
            json!({"explain":l["explain"],"items":l["items"]}).to_string()
        } else if p.contains("Answer the learner's question") {
            "A short answer.".into()
        } else if p.contains("Judge the learner's answer") {
            json!({"correct":true,"note":"fine"}).to_string()
        } else if p.contains("The learner did not get this lesson") {
            "A new explanation from another angle.".into()
        } else {
            return Err(DriverError::Failed("unexpected prompt".into()));
        };
        Ok(TurnResponse {
            text,
            tool_calls: vec![],
        })
    }

    async fn complete_structured<T: DeserializeOwned + JsonSchema + Send>(
        &self,
        _r: StructuredRequest,
        _c: CancellationToken,
    ) -> Result<StructuredOutput<T>, DriverError> {
        Err(DriverError::FauxExhausted)
    }

    fn usage(&self) -> (u64, u64) {
        let n = self.calls.load(Ordering::Relaxed) as u64;
        (n * 100, n * 50)
    }
}

// ------------------------------------------------------------------ world

struct World {
    svc: CourseService<Mock>,
    stores: Arc<Stores>,
    shared: Arc<Shared>,
    now: Arc<AtomicI64>,
    dir: tempfile::TempDir,
}

fn service_on(
    root: &Path,
    stores: Arc<Stores>,
    shared: &Arc<Shared>,
    now: &Arc<AtomicI64>,
) -> CourseService<Mock> {
    let sh = shared.clone();
    let engines: Engines<Mock> = Arc::new(move |role: &str| {
        Arc::new(Mock {
            role: role.into(),
            shared: sh.clone(),
            calls: AtomicUsize::new(0),
        })
    });
    let (n, sh) = (now.clone(), shared.clone());
    CourseService::new(root.to_path_buf(), stores, engines, "dev1".into()).with_clock(Arc::new(
        move || {
            if sh.tick.load(Ordering::Relaxed) {
                n.fetch_add(1, Ordering::Relaxed)
            } else {
                n.load(Ordering::Relaxed)
            }
        },
    ))
}

fn world_with(hold: bool) -> World {
    let dir = tempfile::tempdir().unwrap();
    let stores = Arc::new(Stores::open(dir.path()).unwrap());
    let shared = Shared::new(hold);
    let now = Arc::new(AtomicI64::new(T0));
    let svc = service_on(dir.path(), stores.clone(), &shared, &now);
    World {
        svc,
        stores,
        shared,
        now,
        dir,
    }
}

fn world() -> World {
    world_with(false)
}

impl World {
    fn advance(&self, ms: i64) {
        self.now.fetch_add(ms, Ordering::Relaxed);
    }
    fn now(&self) -> i64 {
        self.now.load(Ordering::Relaxed)
    }
    fn inner(&self) -> &Arc<Inner<Mock>> {
        &self.svc.inner
    }

    /// Creates a course and answers every placement round (the first `misses` questions of the
    /// first round wrongly, all later ones rightly); does not wait for the Author.
    async fn start(&self, misses: usize) -> String {
        let created = self.svc.create("learn spanish", WHY).await.unwrap();
        self.answer_all(&created.id, misses).await;
        created.id
    }

    /// Answers placement rounds until the probe is over. Returns how many rounds it took.
    async fn answer_all(&self, id: &str, misses: usize) -> usize {
        let mut rounds = 0;
        loop {
            let pending = self.svc.pending_placement(id).await.unwrap();
            if pending.is_empty() {
                return rounds;
            }
            let p: Placement = self.inner().read_json(id, "placement.json").unwrap();
            let answers = pending
                .iter()
                .enumerate()
                .map(|(i, it)| PlacementAnswer {
                    item: it.id.clone(),
                    text: if rounds == 0 && i < misses {
                        "wrong".into()
                    } else {
                        p.items
                            .iter()
                            .find(|x| x.id == it.id)
                            .unwrap()
                            .answer
                            .clone()
                    },
                    ..PlacementAnswer::default()
                })
                .collect();
            let round = self.svc.submit_placement(id, answers).await.unwrap();
            rounds += 1;
            if round.next.is_empty() {
                return rounds;
            }
        }
    }

    async fn ready(&self) -> String {
        let id = self.start(0).await;
        self.wait_ready(&id).await;
        id
    }

    async fn wait_ready(&self, id: &str) {
        for _ in 0..400 {
            if self.svc.status(id).await.unwrap() != CourseStatus::Writing {
                break;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        assert_eq!(self.svc.status(id).await.unwrap(), CourseStatus::Ready);
    }

    /// Waits until no refill task of the course is running.
    async fn settle_refill(&self, id: &str) {
        for _ in 0..400 {
            if !self.inner().rt.lock().refilling.contains(id) {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        panic!("refill did not finish");
    }

    async fn settle_reteach(&self, id: &str) {
        for _ in 0..400 {
            if !self
                .inner()
                .rt
                .lock()
                .reteaching
                .iter()
                .any(|(c, _)| c == id)
            {
                return;
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
        panic!("reteach did not finish");
    }

    fn course(&self, id: &str) -> Course {
        self.stores.courses.load_course(id).unwrap()
    }

    fn events(&self, id: &str) -> Vec<CourseEvent> {
        self.stores.courses.events(id).unwrap()
    }

    fn item(&self, id: &str, item: &str) -> Item {
        self.course(id)
            .lessons
            .iter()
            .flat_map(|l| &l.items)
            .find(|i| i.id == item)
            .unwrap()
            .clone()
    }

    /// The id of item `i` of lesson `l`.
    fn ith(&self, id: &str, l: usize, i: usize) -> String {
        self.course(id).lessons[l].items[i].id.clone()
    }

    /// One step with every answer right. Returns the step.
    async fn step(&self, id: &str) -> StepView {
        let v = self.svc.next(id).await.unwrap();
        if let StepView::Explain(l) = &v {
            self.svc.seen(id, &l.id).await.unwrap();
        }
        if let StepView::Ask(a) = &v {
            let right = self.item(id, &a.id).answer;
            let fb = self
                .svc
                .answer(id, &a.id, &right, Some(4), Some(900))
                .await
                .unwrap();
            assert!(fb.correct);
        }
        self.advance(5_000);
        v
    }

    fn seed_strong(&self, id: &str, ts: i64) {
        self.stores
            .courses
            .append_event(
                id,
                &CourseEvent::Usage {
                    ts,
                    role: "author".into(),
                    model: String::new(),
                    tokens_in: 1,
                    tokens_out: 1,
                },
            )
            .unwrap();
    }
}

// ------------------------------------------------------------------ tests

#[tokio::test]
async fn a_sixty_step_hour_makes_at_most_three_model_calls() {
    let w = world();
    let id = w.ready().await;
    assert_eq!(w.shared.total(), 2, "placement and the Author");
    for n in 0..60 {
        let v = w.step(&id).await;
        assert!(
            matches!(v, StepView::Explain(_) | StepView::Ask(_)),
            "step {n}: {v:?}"
        );
    }
    w.settle_refill(&id).await;
    assert!(w.shared.total() <= 3, "calls: {:?}", w.shared.calls.lock());
    // Every answer was graded by code: no follow-up or check call at all.
    assert_eq!(w.shared.count("follow_up"), 0);
    assert!(w.svc.usage(&id).await.unwrap().strong_calls_this_hour <= 2);
    assert!(w.stores.tracker.reviews().unwrap().len() >= 50);
}

#[tokio::test]
async fn when_every_provider_is_limited_study_runs_to_the_end_of_stored_lessons_and_reviews() {
    let w = world();
    let id = w.ready().await;
    for _ in 0..12 {
        w.step(&id).await;
    }
    w.shared.limit("*", true);
    // A returning session: three days on, due reviews come first, without any model.
    w.advance(3 * DAY);
    let first = w.svc.next(&id).await.unwrap();
    let StepView::Ask(a) = &first else {
        panic!("expected a review, got {first:?}")
    };
    assert_eq!(a.ask, AskKind::Review);
    let before = w.shared.total();
    let mut answered = 0;
    for _ in 0..300 {
        match w.step(&id).await {
            StepView::Ask(_) => answered += 1,
            StepView::Waiting | StepView::Done => break,
            StepView::Explain(_) => {}
        }
        w.settle_refill(&id).await;
    }
    assert!(answered >= 50, "answered {answered}");
    assert_eq!(w.svc.next(&id).await.unwrap(), StepView::Waiting);
    // The only calls were failed refill attempts, and the course never failed.
    assert_eq!(w.svc.status(&id).await.unwrap(), CourseStatus::Ready);
    assert!(w.shared.total() - before <= 8);
    // Questions say so plainly, and are kept.
    assert!(matches!(
        w.svc.ask(&id, "why?").await,
        Err(CourseError::Unavailable(_))
    ));
    assert!(w
        .events(&id)
        .iter()
        .any(|e| matches!(e, CourseEvent::Asked { question, .. } if question == "why?")));
}

#[tokio::test]
async fn a_failed_refill_keeps_studying_and_retries_after_the_backoff() {
    let w = world();
    let id = w.ready().await;
    w.shared.limit("*", true);
    let mut waiting = false;
    for _ in 0..200 {
        if w.step(&id).await == StepView::Waiting {
            waiting = true;
            break;
        }
        w.settle_refill(&id).await;
    }
    assert!(waiting);
    assert_eq!(w.svc.status(&id).await.unwrap(), CourseStatus::Ready);
    let calls = w.shared.total();
    // Inside the backoff nothing is retried.
    w.svc.next(&id).await.unwrap();
    w.settle_refill(&id).await;
    assert_eq!(w.shared.total(), calls);
    // After it, with the AIs back, the refill merges 10 new lessons and bumps the version.
    w.shared.limit("*", false);
    w.advance(REFILL_BACKOFF_MS + 1);
    assert_eq!(w.svc.next(&id).await.unwrap(), StepView::Waiting);
    w.settle_refill(&id).await;
    let c = w.course(&id);
    assert_eq!(c.lessons.len(), 20);
    assert_eq!(c.version, 2);
    assert!(matches!(
        w.svc.next(&id).await.unwrap(),
        StepView::Explain(_) | StepView::Ask(_)
    ));
    assert_eq!(w.stores.tracker.cards().unwrap().len(), 120);
}

#[tokio::test]
async fn the_third_strong_call_in_an_hour_queues_while_study_continues() {
    let w = world();
    let id = w.ready().await;
    // The course's Author was strong call 1; one more fills the cap.
    w.seed_strong(&id, w.now() + 1_000);
    assert_eq!(w.svc.usage(&id).await.unwrap().strong_calls_this_hour, 2);
    let authors = w.shared.count("author");
    let mut waiting = false;
    for _ in 0..200 {
        let v = w.step(&id).await;
        if v == StepView::Waiting {
            waiting = true;
            break;
        }
    }
    assert!(waiting);
    w.settle_refill(&id).await;
    assert_eq!(w.shared.count("author"), authors, "no third strong call");
    assert_eq!(w.svc.status(&id).await.unwrap(), CourseStatus::Refilling);
    assert!(w
        .svc
        .usage(&id)
        .await
        .unwrap()
        .refill_waiting_until
        .is_some());
    // The oldest call is the course's own Author; the slot frees an hour after it.
    w.advance(HOUR_MS);
    w.svc.next(&id).await.unwrap();
    w.settle_refill(&id).await;
    assert_eq!(w.shared.count("author"), authors + 1);
    assert_eq!(w.course(&id).lessons.len(), 20);
    assert_eq!(w.svc.status(&id).await.unwrap(), CourseStatus::Ready);
}

#[tokio::test]
async fn a_cheap_fallback_never_counts_as_strong() {
    let w = world();
    w.shared.limit("author", true);
    let id = w.ready().await;
    // The strong call hit a limit and the two-stage cheap path wrote the course.
    assert_eq!(w.course(&id).lessons.len(), 2);
    let u = w.svc.usage(&id).await.unwrap();
    assert_eq!(u.strong_calls_this_hour, 0);
    assert_eq!((u.strong_in, u.strong_out), (0, 0));
    assert!(u.cheap_in > 100);
}

#[tokio::test]
async fn over_the_cap_a_new_course_is_written_cheaply() {
    let w = world();
    let first = w.ready().await;
    w.seed_strong(&first, w.now());
    let authors = w.shared.count("author");
    let second = w.ready().await;
    assert_eq!(w.shared.count("author"), authors);
    assert_eq!(
        w.svc.usage(&second).await.unwrap().strong_calls_this_hour,
        2
    );
    assert!(!w.course(&second).lessons.is_empty());
}

#[tokio::test]
async fn usage_splits_strong_and_cheap_tokens_by_role() {
    let w = world();
    let id = w.ready().await;
    let u = w.svc.usage(&id).await.unwrap();
    assert_eq!((u.strong_in, u.strong_out), (100, 50));
    assert_eq!((u.cheap_in, u.cheap_out), (100, 50));
    assert_eq!(u.strong_calls_this_hour, 1);
    w.step(&id).await;
    assert_eq!(
        w.svc.ask(&id, "why red apple?").await.unwrap(),
        "A short answer."
    );
    let u = w.svc.usage(&id).await.unwrap();
    assert_eq!((u.cheap_in, u.cheap_out), (200, 100));
    assert_eq!(w.shared.count("follow_up"), 1);
    assert!(matches!(
        w.svc.ask(&id, &"x".repeat(MAX_TEXT_CHARS + 1)).await,
        Err(CourseError::Invalid(_))
    ));
    let recall = w.ith(&id, 0, 4);
    assert!(
        w.svc
            .check(&id, &recall, "something")
            .await
            .unwrap()
            .correct
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_answers_lose_nothing() {
    let w = world();
    let id = w.ready().await;
    w.shared.tick.store(true, Ordering::Relaxed);
    let items: Vec<String> = w
        .course(&id)
        .lessons
        .iter()
        .take(5)
        .flat_map(|l| l.items.iter().map(|i| i.id.clone()))
        .collect();
    let mut set = tokio::task::JoinSet::new();
    // Thirty distinct items plus the same item four more times, all at once.
    for item in items.iter().chain(std::iter::repeat_n(&items[0], 4)) {
        let (svc, id, item) = (w.svc.clone(), id.clone(), item.clone());
        let right = w.item(&id, &item).answer;
        set.spawn(async move { svc.answer(&id, &item, &right, Some(3), None).await.unwrap() });
    }
    while let Some(r) = set.join_next().await {
        assert!(r.unwrap().correct);
    }
    let answered = w
        .events(&id)
        .iter()
        .filter(|e| matches!(e, CourseEvent::Answered { .. }))
        .count();
    assert_eq!(answered, items.len() + 4);
    assert_eq!(w.stores.tracker.reviews().unwrap().len(), items.len() + 4);
}

#[tokio::test]
async fn a_course_stuck_in_writing_after_a_restart_fails_and_can_be_retried() {
    let w = world_with(true);
    let id = w.start(0).await;
    assert_eq!(w.svc.status(&id).await.unwrap(), CourseStatus::Writing);
    // "Restart": a new service on the same files, whose AIs work.
    let open = Shared::new(false);
    let b = service_on(w.dir.path(), w.stores.clone(), &open, &w.now);
    assert!(matches!(
        b.status(&id).await.unwrap(),
        CourseStatus::Failed { .. }
    ));
    assert_eq!(b.next(&id).await.unwrap(), StepView::Waiting);
    b.retry(&id).await.unwrap();
    assert!(matches!(b.retry(&id).await, Err(CourseError::Invalid(_))));
    for _ in 0..400 {
        if b.status(&id).await.unwrap() != CourseStatus::Writing {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(b.status(&id).await.unwrap(), CourseStatus::Ready);
    assert!(matches!(b.next(&id).await.unwrap(), StepView::Explain(_)));
}

#[tokio::test]
async fn a_failed_write_is_retried_from_the_stored_placement() {
    let w = world();
    let created = w.svc.create("learn rust", WHY).await.unwrap();
    w.shared.limit("*", true);
    w.answer_all(&created.id, 0).await;
    for _ in 0..400 {
        if w.svc.status(&created.id).await.unwrap() != CourseStatus::Writing {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(matches!(
        w.svc.status(&created.id).await.unwrap(),
        CourseStatus::Failed { .. }
    ));
    w.shared.limit("*", false);
    w.svc.retry(&created.id).await.unwrap();
    w.wait_ready(&created.id).await;
}

#[tokio::test]
async fn create_fails_soft_and_leaves_no_course_behind() {
    let w = world();
    w.shared.limit("*", true);
    let e = w.svc.create("learn spanish", WHY).await.unwrap_err();
    assert!(matches!(e, CourseError::Unavailable(_)), "{e:?}");
    assert!(w.svc.list().await.unwrap().is_empty());
    let courses = w.dir.path().join("courses");
    assert!(!courses.exists() || std::fs::read_dir(courses).unwrap().next().is_none());
}

#[tokio::test]
async fn a_course_stays_listed_before_it_is_written() {
    let w = world_with(true);
    assert!(w.svc.list().await.unwrap().is_empty());
    let placing = w.svc.create("learn spanish", WHY).await.unwrap();
    let listed = w.svc.list().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(
        (listed[0].id.as_str(), listed[0].title.as_str()),
        (placing.id.as_str(), "learn spanish")
    );
}

#[tokio::test]
async fn a_written_course_is_listed_by_its_goal() {
    let w = world();
    let id = w.ready().await;
    let listed = w.svc.list().await.unwrap();
    assert_eq!(listed[0].id, id);
    assert_eq!(listed[0].title, "learn spanish");
}

#[tokio::test]
async fn a_newly_written_course_follows_the_learning_method() {
    // Found by `scripts/real_smoke.py`: courses were written at version 0, so the plan route
    // answered 404 and the app never reached the bout shell or the method gates.
    let w = world();
    let id = w.ready().await;
    assert_eq!(w.course(&id).method_version, 1);
}

#[tokio::test]
async fn pending_placement_survives_a_reload_and_empties_once_submitted() {
    let w = world_with(true);
    let created = w.svc.create("learn spanish", WHY).await.unwrap();
    // A reloaded page gets the same questions in the same option order.
    assert_eq!(
        w.svc.pending_placement(&created.id).await.unwrap(),
        created.items
    );
    assert!(matches!(
        w.svc.pending_placement("nope").await,
        Err(CourseError::NotFound(_))
    ));
    w.answer_all(&created.id, 0).await;
    assert!(w
        .svc
        .pending_placement(&created.id)
        .await
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn placement_misses_are_a_mini_lesson_while_the_course_is_written() {
    let w = world_with(true);
    let id = w.start(2).await;
    assert_eq!(w.svc.status(&id).await.unwrap(), CourseStatus::Writing);
    let misses = w.svc.placement_misses(&id).await.unwrap();
    assert_eq!(misses.len(), 2);
    assert_eq!(misses[0].why, "placement why s0 l2");
    assert_eq!(misses[0].answer, "red apple");
    assert_eq!(w.svc.next(&id).await.unwrap(), StepView::Waiting);
    // The profile was saved and placement cannot be submitted twice.
    assert!(w.stores.courses.load_learner().unwrap().contains("4 of 6"));
    assert!(matches!(
        w.svc.submit_placement(&id, vec![]).await,
        Err(CourseError::Invalid(_))
    ));
    w.shared.gate.add_permits(10);
    w.wait_ready(&id).await;
    assert_eq!(w.svc.placement_misses(&id).await.unwrap().len(), 2);
}

fn answers_for(
    w: &World,
    id: &str,
    pending: &[PlacementItem],
    text: impl Fn(usize) -> (String, bool, bool),
) -> Vec<PlacementAnswer> {
    let p: Placement = w.inner().read_json(id, "placement.json").unwrap();
    pending
        .iter()
        .enumerate()
        .map(|(i, it)| {
            let (given, guessed, blocked) = text(i);
            let given = if given == "right" {
                p.items
                    .iter()
                    .find(|x| x.id == it.id)
                    .unwrap()
                    .answer
                    .clone()
            } else {
                given
            };
            PlacementAnswer {
                item: it.id.clone(),
                text: given,
                guessed,
                blocked,
            }
        })
        .collect()
}

#[tokio::test]
async fn the_probe_asks_the_middle_level_of_every_strand_first() {
    let w = world_with(true);
    let created = w.svc.create("learn spanish", WHY).await.unwrap();
    let ids: Vec<_> = created.items.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, ["s0-l2", "s1-l2", "s2-l2"]);
}

#[tokio::test]
async fn the_probe_follows_each_strand_up_or_down_and_never_asks_more_than_six() {
    let w = world_with(true);
    let created = w.svc.create("learn spanish", WHY).await.unwrap();
    let id = created.id;
    // Strand 0 right, strand 1 wrong, strand 2 right but guessing.
    let a = answers_for(&w, &id, &created.items, |i| match i {
        0 => ("right".into(), false, false),
        1 => ("wrong".into(), false, false),
        _ => ("right".into(), true, false),
    });
    let r1 = w.svc.submit_placement(&id, a).await.unwrap();
    assert_eq!(r1.graded.len(), 3);
    // Strand 0 goes up to level 3, strand 1 down to level 1, strand 2 is settled.
    let ids: Vec<_> = r1.next.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, ["s0-l3", "s1-l1"]);
    assert_eq!(w.svc.pending_placement(&id).await.unwrap(), r1.next);
    let a = answers_for(&w, &id, &r1.next, |i| {
        (if i == 0 { "right" } else { "wrong" }.into(), false, false)
    });
    let r2 = w.svc.submit_placement(&id, a).await.unwrap();
    assert!(r2.next.is_empty());
    let p: Placement = w.inner().read_json(&id, "placement.json").unwrap();
    assert_eq!(p.results.as_ref().unwrap().len(), 5);
    let labels: Vec<_> = strand_labels(&p).into_iter().map(|s| s.label).collect();
    use lp_core::method::probe::StrandLabel::*;
    assert_eq!(labels, [Known, Unknown, Edge]);
    // The writing starts only now, and the course carries the why and the strands.
    w.shared.gate.add_permits(10);
    w.wait_ready(&id).await;
    let c = w.course(&id);
    assert_eq!(c.why, WHY);
    assert_eq!(
        c.strands.iter().map(|s| s.label).collect::<Vec<_>>(),
        labels
    );
    assert!(w
        .stores
        .courses
        .load_learner()
        .unwrap()
        .contains("Strand Strand 1: unknown"));
}

#[tokio::test]
async fn a_repeated_or_partial_round_is_refused_and_changes_nothing() {
    let w = world_with(true);
    let created = w.svc.create("learn spanish", WHY).await.unwrap();
    let id = created.id;
    let right = |w: &World, items: &[PlacementItem]| {
        answers_for(w, &id, items, |_| ("right".into(), false, false))
    };
    // A partial round is refused.
    let mut partial = right(&w, &created.items);
    partial.pop();
    assert!(matches!(
        w.svc.submit_placement(&id, partial).await,
        Err(CourseError::Invalid(_))
    ));
    assert_eq!(w.svc.pending_placement(&id).await.unwrap(), created.items);
    // The round goes through once; sending the same answers again must not grade round two blank.
    let first = right(&w, &created.items);
    let r1 = w.svc.submit_placement(&id, first.clone()).await.unwrap();
    assert_eq!(r1.next.len(), 3);
    assert!(matches!(
        w.svc.submit_placement(&id, first).await,
        Err(CourseError::Invalid(_))
    ));
    assert_eq!(w.svc.pending_placement(&id).await.unwrap(), r1.next);
    let p: Placement = w.inner().read_json(&id, "placement.json").unwrap();
    assert!(p.strands.iter().all(|s| s.answers.len() == 1));
}

#[tokio::test]
async fn a_blocked_strand_is_labelled_blocked_and_settles_at_once() {
    let w = world_with(true);
    let created = w.svc.create("learn spanish", WHY).await.unwrap();
    let a = answers_for(&w, &created.id, &created.items, |i| {
        ("wrong".into(), false, i == 1)
    });
    let r = w.svc.submit_placement(&created.id, a).await.unwrap();
    let ids: Vec<_> = r.next.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(ids, ["s0-l1", "s2-l1"]);
    let p: Placement = w.inner().read_json(&created.id, "placement.json").unwrap();
    assert_eq!(
        strand_labels(&p)[1].label,
        lp_core::method::probe::StrandLabel::Blocked
    );
}

#[tokio::test]
async fn why_is_required_and_a_failed_strand_call_falls_back_to_the_plain_quiz() {
    let w = world_with(true);
    for why in ["", "   "] {
        assert!(matches!(
            w.svc.create("learn spanish", why).await,
            Err(CourseError::Invalid(_))
        ));
    }
    assert!(w.svc.list().await.unwrap().is_empty());
    w.shared.plain_only.store(true, Ordering::Relaxed);
    let created = w.svc.create("learn spanish", WHY).await.unwrap();
    // The plain quiz is five questions answered in one round, with no strands.
    assert_eq!(created.items.len(), 5);
    assert_eq!(w.answer_all(&created.id, 0).await, 1);
    w.shared.gate.add_permits(10);
    w.wait_ready(&created.id).await;
    let c = w.course(&created.id);
    assert!(c.strands.is_empty());
    assert_eq!(c.why, WHY);
}

#[tokio::test]
async fn two_workspaces_never_share_courses_cards_or_budget() {
    let a = world();
    let b = world();
    let ida = a.ready().await;
    a.seed_strong(&ida, a.now());
    assert!(b.svc.list().await.unwrap().is_empty());
    assert!(matches!(
        b.svc.next(&ida).await,
        Err(CourseError::NotFound(_))
    ));
    assert!(b.stores.tracker.cards().unwrap().is_empty());
    assert_eq!(a.stores.tracker.cards().unwrap().len(), 60);
    let idb = b.ready().await;
    assert_eq!(a.svc.usage(&ida).await.unwrap().strong_calls_this_hour, 2);
    assert_eq!(b.svc.usage(&idb).await.unwrap().strong_calls_this_hour, 1);
    assert_eq!(b.stores.tracker.cards().unwrap().len(), 60);
}

#[tokio::test]
async fn course_answers_and_review_share_one_fsrs_history() {
    let w = world();
    let id = w.ready().await;
    let t = &*w.stores.tracker;
    // Unstudied cards are parked, so Review lists nothing yet; recall cards are well formed.
    assert!(review::due_cards(t, w.now() + 365 * DAY)
        .unwrap()
        .is_empty());
    let recall_id = w.ith(&id, 0, 4);
    let recall = t.card(&card_id(&id, &recall_id)).unwrap().unwrap();
    assert_eq!(
        recall.options[usize::from(recall.correct_index)],
        w.item(&id, &recall_id).answer
    );
    assert_eq!(recall.options.iter().collect::<HashSet<_>>().len(), 4);

    // Course to Review: a right answer at confidence 4 is one review on the card.
    assert!(matches!(w.step(&id).await, StepView::Explain(_)));
    let StepView::Ask(a) = w.step(&id).await else {
        panic!("expected a question")
    };
    let cid = card_id(&id, &a.id);
    let card = t.card(&cid).unwrap().unwrap();
    assert_eq!(card.card.reps, 1);
    assert_ne!(card.card.state, State::New);
    assert_eq!(review::history(t, &cid).unwrap().len(), 1);
    assert!(review::due_cards(t, w.now() + 30 * DAY)
        .unwrap()
        .iter()
        .any(|c| c.id == cid.as_str()));

    // A wrong answer is Again.
    let other = w.ith(&id, 0, 1);
    let fb = w
        .svc
        .answer(&id, &other, "blue plum", Some(5), None)
        .await
        .unwrap();
    assert!(!fb.correct);
    assert_eq!(fb.answer, "red apple");
    assert_eq!(fb.why, w.item(&id, &other).why);
    let hist = review::history(t, &card_id(&id, &other)).unwrap();
    assert_eq!(hist.last().unwrap().rating, Rating::Again);

    // Review to course: grading an unstudied lesson's card in Review makes the next session serve it.
    let far = w.ith(&id, 5, 0);
    review::grade(t, "dev1", &card_id(&id, &far), Rating::Good, w.now()).unwrap();
    w.advance(10 * DAY);
    match w.svc.next(&id).await.unwrap() {
        StepView::Ask(a) => assert_eq!(a.ask, AskKind::Review),
        v => panic!("expected a review, got {v:?}"),
    }
    // ...and the far item itself comes up as a review within the session-start phase.
    let mut found = false;
    for _ in 0..10 {
        if let StepView::Ask(a) = w.svc.next(&id).await.unwrap() {
            if a.id == far {
                found = a.ask == AskKind::Review;
                break;
            }
            w.svc
                .answer(&id, &a.id, &w.item(&id, &a.id).answer, None, None)
                .await
                .unwrap();
        }
    }
    assert!(found);
}

#[tokio::test]
async fn a_close_typed_answer_waits_for_the_learners_own_grade() {
    let w = world();
    let id = w.ready().await;
    let item = w.item(&id, &w.ith(&id, 0, 4));
    let typo = format!("{}x", item.answer);
    assert_eq!(match_answer(&item, &typo), Match::Close);
    let fb = w
        .svc
        .answer(&id, &item.id, &typo, None, None)
        .await
        .unwrap();
    assert!(!fb.correct && fb.close);
    let t = &*w.stores.tracker;
    assert!(review::history(t, &card_id(&id, &item.id))
        .unwrap()
        .is_empty());
    w.svc.self_grade(&id, &item.id, "Good").await.unwrap();
    assert_eq!(
        review::history(t, &card_id(&id, &item.id)).unwrap().len(),
        1
    );
    assert!(matches!(
        w.svc.self_grade(&id, &item.id, "great").await,
        Err(CourseError::Invalid(_))
    ));
}

#[tokio::test]
async fn a_flagged_item_is_never_served_again() {
    let w = world();
    let id = w.ready().await;
    let flagged = w.ith(&id, 0, 0);
    w.svc.flag(&id, &flagged, "looks wrong").await.unwrap();
    for _ in 0..70 {
        if let StepView::Ask(a) = w.step(&id).await {
            assert_ne!(a.id, flagged);
        }
    }
}

#[tokio::test]
async fn a_lesson_under_half_is_explained_again_then_re_taught_once() {
    let w = world();
    let id = w.ready().await;
    let first = w.course(&id).lessons[0].id.clone();
    // Wrong every time, except the first two items on their retry: the lesson ends at 2 of 6.
    let wrong = |w: &World, it: &Item| {
        let tries = w
            .events(&id)
            .iter()
            .filter(|e| matches!(e, CourseEvent::Answered { item, .. } if *item == it.id))
            .count();
        let lucky = tries > 0 && (it.id.ends_with("i0") || it.id.ends_with("i1"));
        match (lucky, it.kind) {
            (true, _) => it.answer.clone(),
            (_, ItemKind::Mcq) => "blue plum".to_string(),
            (_, ItemKind::Recall) => "no idea".to_string(),
        }
    };
    let dones = |w: &World| {
        w.events(&id)
            .iter()
            .filter(|e| matches!(e, CourseEvent::LessonDone { lesson, .. } if *lesson == first))
            .count()
    };
    let mut explained = 0;
    for _ in 0..40 {
        if dones(&w) == 2 {
            break;
        }
        match w.svc.next(&id).await.unwrap() {
            StepView::Explain(l) => {
                if l.id == first {
                    explained += 1;
                }
                w.svc.seen(&id, &l.id).await.unwrap();
            }
            StepView::Ask(a) => {
                let it = w.item(&id, &a.id);
                let text = if a.lesson == first {
                    wrong(&w, &it)
                } else {
                    it.answer.clone()
                };
                w.svc.answer(&id, &a.id, &text, None, None).await.unwrap();
            }
            other => panic!("{other:?}"),
        }
        w.advance(5_000);
    }
    assert_eq!(dones(&w), 2);
    // Once at the start and once after the first round.
    assert_eq!(explained, 2);
    w.settle_reteach(&id).await;
    assert_eq!(
        w.course(&id).lessons[0].explain,
        "A new explanation from another angle."
    );
    assert_eq!(w.shared.count("follow_up"), 1);
    // The new text is shown once, and there is never a third round.
    let v = w.svc.next(&id).await.unwrap();
    match &v {
        StepView::Explain(l) => {
            assert_eq!(l.explain, "A new explanation from another angle.");
            assert_eq!(w.svc.next(&id).await.unwrap(), v);
            w.svc.seen(&id, &l.id).await.unwrap();
            w.svc.seen(&id, &l.id).await.unwrap();
        }
        v => panic!("{v:?}"),
    }
    for _ in 0..30 {
        if let StepView::Ask(a) = w.svc.next(&id).await.unwrap() {
            let it = w.item(&id, &a.id);
            let text = if a.lesson == first {
                wrong(&w, &it)
            } else {
                it.answer.clone()
            };
            w.svc.answer(&id, &a.id, &text, None, None).await.unwrap();
        }
    }
    assert_eq!(dones(&w), 2);
    assert_eq!(w.shared.count("follow_up"), 1);
}

#[tokio::test]
async fn next_is_repeatable_and_seen_acknowledges_the_explanation_once() {
    let w = world();
    let id = w.ready().await;
    let first = w.svc.next(&id).await.unwrap();
    let StepView::Explain(l) = &first else {
        panic!("{first:?}")
    };
    // A reload or a double fetch changes nothing.
    assert_eq!(w.svc.next(&id).await.unwrap(), first);
    assert_eq!(w.svc.next(&id).await.unwrap(), first);
    let shown = |w: &World| {
        w.events(&id)
            .iter()
            .filter(|e| matches!(e, CourseEvent::Shown { .. }))
            .count()
    };
    assert_eq!(shown(&w), 0);
    w.svc.seen(&id, &l.id).await.unwrap();
    w.svc.seen(&id, &l.id).await.unwrap();
    assert_eq!(shown(&w), 1);
    let next = w.svc.next(&id).await.unwrap();
    assert!(matches!(next, StepView::Ask(_)));
    assert_eq!(w.svc.next(&id).await.unwrap(), next);
    assert!(matches!(
        w.svc.seen(&id, "nope").await,
        Err(CourseError::NotFound(_))
    ));
}

#[tokio::test]
async fn repeated_next_does_not_use_up_the_session_start_reviews() {
    let w = world();
    let id = w.ready().await;
    for _ in 0..12 {
        w.step(&id).await;
    }
    w.advance(3 * DAY);
    // Many fetches with no answer: still the same review, and the phase is not spent.
    let first = w.svc.next(&id).await.unwrap();
    for _ in 0..15 {
        assert_eq!(w.svc.next(&id).await.unwrap(), first);
    }
    let StepView::Ask(a) = first else { panic!() };
    assert_eq!(a.ask, AskKind::Review);
    let right = w.item(&id, &a.id).answer;
    w.svc.answer(&id, &a.id, &right, None, None).await.unwrap();
    let second = w.svc.next(&id).await.unwrap();
    assert!(matches!(second, StepView::Ask(ref b) if b.ask == AskKind::Review));
}

#[tokio::test]
async fn strong_tokens_are_charged_even_when_the_reply_is_unusable() {
    let w = world();
    let id = w.ready().await;
    assert_eq!(w.svc.usage(&id).await.unwrap().strong_calls_this_hour, 1);
    // Opus answers with garbage (tokens spent) and the cheap path is down: the refill fails.
    w.shared.garbage.lock().insert("author".into());
    w.shared.limit("placer", true);
    let mut waiting = false;
    for _ in 0..200 {
        if w.step(&id).await == StepView::Waiting {
            waiting = true;
            break;
        }
        w.settle_refill(&id).await;
    }
    assert!(waiting);
    w.settle_refill(&id).await;
    let u = w.svc.usage(&id).await.unwrap();
    assert_eq!(u.strong_calls_this_hour, 2, "the failed refill was charged");
    assert!(u.strong_in > 100);
    assert_eq!(w.svc.status(&id).await.unwrap(), CourseStatus::Ready);
    // Past the backoff the cap holds: no further Opus call.
    let authors = w.shared.count("author");
    w.advance(REFILL_BACKOFF_MS + 1);
    w.svc.next(&id).await.unwrap();
    w.settle_refill(&id).await;
    assert_eq!(w.shared.count("author"), authors);
    assert!(w
        .svc
        .usage(&id)
        .await
        .unwrap()
        .refill_waiting_until
        .is_some());
}

#[tokio::test]
async fn an_attempt_that_spent_no_tokens_is_not_charged() {
    let w = world();
    w.shared.limit("author", true);
    let id = w.ready().await;
    assert_eq!(w.svc.usage(&id).await.unwrap().strong_calls_this_hour, 0);
}

#[tokio::test]
async fn options_are_shuffled_by_item_id_and_stable() {
    let w = world();
    let created = w.svc.create("learn spanish", WHY).await.unwrap();
    let p: Placement = w.inner().read_json(&created.id, "placement.json").unwrap();
    let id = w.ready().await;
    let c = w.course(&id);
    let mut first_correct = 0;
    let mut total = 0;
    for l in &c.lessons {
        for it in l.items.iter().filter(|i| i.kind == ItemKind::Mcq) {
            let a = shuffled(&it.options, &it.id);
            assert_eq!(a, shuffled(&it.options, &it.id));
            let mut sorted = a.clone();
            sorted.sort();
            let mut stored = it.options.clone();
            stored.sort();
            assert_eq!(sorted, stored);
            total += 1;
            first_correct += usize::from(a[0] == it.answer);
        }
    }
    // The stored order has the answer first; shown order must not always.
    assert!(first_correct < total, "{first_correct}/{total}");
    // The served view uses it, twice the same, and the placement view too.
    let v1 = w.svc.next(&id).await.unwrap();
    assert_eq!(v1, w.svc.next(&id).await.unwrap());
    let StepView::Explain(l) = v1 else { panic!() };
    w.svc.seen(&id, &l.id).await.unwrap();
    let StepView::Ask(a) = w.svc.next(&id).await.unwrap() else {
        panic!()
    };
    assert_eq!(a.options, shuffled(&w.item(&id, &a.id).options, &a.id));
    assert!(created
        .items
        .iter()
        .all(|i| p.items.iter().any(|q| q.id == i.id)));
    // Review serves cards as stored, so cards are stored shuffled, and still point at the answer.
    let t = &*w.stores.tracker;
    let cards = t.cards().unwrap();
    let firsts = cards
        .iter()
        .filter(|c| c.options[0] == c.options[usize::from(c.correct_index)])
        .count();
    assert!(firsts < cards.len());
    for card in &cards {
        let item = card.id.rsplit(':').next().unwrap();
        assert_eq!(
            card.options[usize::from(card.correct_index)],
            w.item(&id, item).answer
        );
    }
}

// ------------------------------------------------------------------ M13: simulated bout

const MIN: i64 = 60_000;
const READY_GATE: crate::learning::bout::Ready = crate::learning::bout::Ready {
    phone_away: true,
    tabs_closed: true,
    focus_seconds: 30,
};

/// A 90-minute bout from the ready gate to Close with injected misses (placement misses plus a
/// wrong answer every tenth question). `limited` limits every provider once the course is ready.
async fn simulated_bout(limited: bool) {
    use crate::learning::{LearningError, LearningService, Refusal};
    let w = world();
    let learn = LearningService::open(w.dir.path());
    let id = w.start(2).await;
    w.wait_ready(&id).await;
    if limited {
        w.shared.limit("*", true);
    }
    let calls_before = w.shared.total();

    // Ready gate first: an unfinished gate cannot open a bout.
    let not_ready = crate::learning::bout::Ready {
        phone_away: false,
        ..READY_GATE
    };
    assert_eq!(
        learn.start_bout(w.now(), &id, not_ready),
        Err(LearningError::Refused(Refusal::NotReady))
    );
    let start = w.now();
    let bout = learn.start_bout(start, &id, READY_GATE).unwrap();

    let (mut order, mut asked, mut wrong) = (Vec::new(), 0, 0);
    let mut explained: HashSet<String> = HashSet::new();
    for n in 0..60 {
        let v = w.svc.next(&id).await.unwrap();
        match &v {
            StepView::Explain(l) => {
                explained.insert(l.id.clone());
                order.push("explain");
                w.svc.seen(&id, &l.id).await.unwrap();
            }
            StepView::Ask(a) => {
                order.push("ask");
                // An item is never asked before its lesson was explained (reviews excepted).
                assert!(
                    a.ask == AskKind::Review || explained.contains(&a.lesson),
                    "step {n}: {} asked before its explanation",
                    a.lesson
                );
                asked += 1;
                let miss = asked % 10 == 0;
                let text = if miss {
                    "wrong".to_string()
                } else {
                    w.item(&id, &a.id).answer
                };
                let fb = w
                    .svc
                    .answer(&id, &a.id, &text, Some(3), Some(900))
                    .await
                    .unwrap();
                assert_eq!(fb.correct, !miss);
                wrong += usize::from(miss);
            }
            StepView::Waiting | StepView::Done => {
                assert!(limited, "step {n}: unexpected {v:?}");
                order.push("wait");
            }
        }
        w.advance(90_000);
        learn.heartbeat(w.now(), &bout).unwrap();
        if matches!(v, StepView::Waiting | StepView::Done) {
            break;
        }
    }
    w.settle_refill(&id).await;
    w.settle_reteach(&id).await;
    assert_eq!(order[0], "explain", "a bout opens with teaching: {order:?}");
    assert!(asked >= 5 && wrong >= 1, "asked {asked}, wrong {wrong}");
    if !limited {
        assert_eq!(order.len(), 60);
    }

    // Close, then the bout ends at 90 minutes and the lock follows.
    let report = super::close::close_report(
        &w.course(&id),
        &w.events(&id),
        &lp_core::method::Settings::default(),
    );
    assert!(!report.count_line.is_empty());
    let end = start + 90 * MIN;
    w.now.store(end, Ordering::Relaxed);
    let clock = learn.end_bout(end, &bout).unwrap();
    assert!(clock.bout.is_none());
    assert_eq!(
        learn.start_bout(end + 60 * MIN, &id, READY_GATE),
        Err(LearningError::Refused(Refusal::Gap {
            remaining_minutes: 60
        }))
    );
    assert!(learn.start_bout(end + 120 * MIN, &id, READY_GATE).is_ok());

    // Budget: at most 2 strong calls and 22 cheap synchronous calls, under 100k tokens.
    let strong = w.shared.count("author");
    let cheap = w.shared.total() - strong;
    assert!(strong <= 2, "strong {strong}: {:?}", w.shared.calls.lock());
    assert!(cheap <= 22, "cheap {cheap}: {:?}", w.shared.calls.lock());
    if limited {
        // Only failed attempts; the course keeps studying from stored lessons.
        assert!(w.shared.total() - calls_before <= 8);
        assert_eq!(w.svc.status(&id).await.unwrap(), CourseStatus::Ready);
    }
    let u = w.svc.usage(&id).await.unwrap();
    assert!(u.strong_in + u.strong_out + u.cheap_in + u.cheap_out < 100_000);
    assert!(u.strong_calls_this_hour <= 2);
}

#[tokio::test]
async fn a_simulated_ninety_minute_bout_runs_in_order_within_budget_and_locks_afterwards() {
    simulated_bout(false).await;
}

#[tokio::test]
async fn a_simulated_bout_with_every_provider_limited_still_studies_and_locks() {
    simulated_bout(true).await;
}
