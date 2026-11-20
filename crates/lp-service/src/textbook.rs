//! Textbook mode service (U7): map an uploaded PDF into chapters, import a chapter as a background
//! job when the learner opens it, store objectives and quiz items as data, and run reading checks
//! with zero model calls.
//!
//! Spec: `brain/learning-method/textbook-spec.md` §6. Upload reads the outline and maps the book
//! (no model call with an outline, one TOC call without) and imports nothing. The only other model
//! calls are `chapter_objectives` per chapter part inside the job runner; every call is counted in
//! the ledger. A chapter gets one try plus one retry, and a book allows at most
//! [`MAX_MANUAL_IMPORTS_PER_HOUR`] chapter imports in a rolling hour, read from the ledger.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use lp_agent::course::textbook::{chapter_objectives, page_block, toc_chapters, ChapterBrief};
use lp_agent::course::CourseAgentError;
use lp_agent::workbench::{Pdf, MAX_PDF_PAGES_PER_READ};
use lp_cli_driver::engine::LlmEngine;
use lp_core::course::ItemKind;
use lp_core::fsrs::Rating;
use lp_core::ports::{
    DeliveryStatus, FlashCard, JobKind, JobRecord, JobStatus, JobStore, StoreError, TrackerStore,
};
use lp_core::textbook::*;
use parking_lot::Mutex;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio_util::sync::CancellationToken;

use crate::chat::runtime::MAX_TEXT_CHARS;
use crate::chat::Engines;
use crate::course::{card_for, grade, shuffled};
use crate::review;
use crate::stores::Stores;
use crate::workspace::BusyGuard;

/// One call is given this long before it counts as a timeout.
const CALL_TIMEOUT: Duration = Duration::from_secs(120);
/// The cheap role the objectives and TOC calls use (D6).
const ROLE: &str = "placer";
/// Pages read at upload to find a book's printed page offset and its table of contents.
const MAP_PAGES: usize = 60;
/// Most model calls one import job may send: the hourly chapter limit times the calls a chapter can
/// take (its parts, each tried twice). A backstop; the hourly limit normally stops a job first.
const MAX_JOB_CALLS: u32 = MAX_MANUAL_IMPORTS_PER_HOUR * (MAX_PARTS as u32) * MAX_ATTEMPTS;
const NO_TEXT: &str = "No text found. Scanned pages need OCR before import.";

/// Stable `Conflict` codes; the server maps them to `details.reason`.
pub mod conflict {
    pub const IMPORT_RUNNING: &str = "import_running";
    pub const NOTHING_TO_IMPORT: &str = "nothing_to_import";
    /// The per-book hourly chapter import limit is reached.
    pub const RATE_LIMITED: &str = "import_rate_limited";
    /// The chapter already used its one try plus one retry.
    pub const RETRY_USED: &str = "chapter_retry_used";
    pub const CHAPTER_NOT_READY: &str = "chapter_not_ready";
    pub const ALREADY_ANSWERED: &str = "already_answered";
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum TextbookError {
    #[error("not found: {0}")]
    NotFound(String),
    #[error("{0}")]
    Invalid(String),
    /// The payload is one of the stable [`conflict`] codes.
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("{0}")]
    Unavailable(String),
    #[error("storage failure: {0}")]
    Store(String),
}

impl From<StoreError> for TextbookError {
    fn from(e: StoreError) -> Self {
        match e {
            StoreError::NotFound(id) => Self::NotFound(id),
            other => Self::Store(other.to_string()),
        }
    }
}

impl From<std::io::Error> for TextbookError {
    fn from(e: std::io::Error) -> Self {
        Self::Store(e.to_string())
    }
}

pub type Res<T> = Result<T, TextbookError>;
type Clock = Arc<dyn Fn() -> i64 + Send + Sync>;
type Notify = Arc<dyn Fn(JobRecord) + Send + Sync>;

// ------------------------------------------------------------------ results

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Upload {
    pub book: String,
    pub existing: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BookPhase {
    Importing,
    Ready,
    Partial,
    Failed,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookSummary {
    pub id: String,
    pub title: String,
    pub total_pages: u32,
    pub chapters: usize,
    pub ready: usize,
    pub state: BookPhase,
    pub created_at: i64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChapterRowState {
    Pending,
    Importing,
    Ready,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChapterRow {
    pub index: u32,
    pub title: String,
    pub pages: PageRange,
    pub kind: ChapterKind,
    pub state: ChapterRowState,
    pub objectives: usize,
    pub status: ChapterStatus,
    pub error: Option<String>,
    pub truncated: bool,
    /// Opening it may start an import: not imported yet, or failed with its retry unused.
    pub can_import: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BookView {
    pub meta: BookMeta,
    pub source: Option<MapSource>,
    pub chapters: Vec<ChapterRow>,
    pub job: Option<String>,
    pub auto_calls: u32,
    pub requested_calls: u32,
    /// Chapter imports in the last hour.
    pub hour_used: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ObjectiveRow {
    pub id: String,
    pub text: String,
    pub pages: PageRange,
    pub items: usize,
}

/// Never carries items: no answers leave the server here.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChapterDetail {
    pub row: ChapterRow,
    pub objectives: Vec<ObjectiveRow>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckItem {
    pub id: String,
    pub objective: String,
    pub kind: ItemKind,
    pub q: String,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckView {
    pub check: String,
    pub chapter: u32,
    pub items: Vec<CheckItem>,
}

/// What rides along with an answer. A retry with the same `client_id` gets the stored result.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AnswerMeta {
    pub confidence: Option<u8>,
    pub ms: Option<u64>,
    pub client_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CheckFeedback {
    pub correct: bool,
    pub close: bool,
    pub answer: String,
    pub why: String,
    pub due: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextbookUsage {
    pub cheap_in: u64,
    pub cheap_out: u64,
    pub auto_calls: u32,
    pub requested_calls: u32,
    /// Chapter imports in the last hour, and the most the book allows in any hour.
    pub hour_used: u32,
    pub hour_cap: u32,
    /// Epoch ms when the oldest counted import leaves the window; set only at the cap.
    pub next_slot_at: Option<i64>,
}

// ------------------------------------------------------------------ service

struct Running {
    job: String,
    cancel: CancellationToken,
    current: Option<u32>,
    /// The final record is being written; the job can no longer be aborted.
    ending: bool,
}

/// Drops the `running` entry of one job, but never a newer job's entry for the same book.
struct RunningGuard<E> {
    me: Arc<Inner<E>>,
    book: String,
    job: String,
}

impl<E> Drop for RunningGuard<E> {
    fn drop(&mut self) {
        let mut running = self.me.running.lock();
        if running.get(&self.book).is_some_and(|r| r.job == self.job) {
            running.remove(&self.book);
        }
    }
}

struct Inner<E> {
    stores: Arc<Stores>,
    engines: Engines<E>,
    pdf: Arc<dyn Pdf>,
    device: String,
    notify: Notify,
    clock: Clock,
    /// Books with a job in flight.
    running: Mutex<HashMap<String, Running>>,
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    /// One whole upload (store, count, map) per book at a time, held across the mapping awaits.
    uploads: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

pub struct TextbookService<E> {
    inner: Arc<Inner<E>>,
}

impl<E> Clone for TextbookService<E> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
        }
    }
}

async fn blk<T: Send + 'static>(f: impl FnOnce() -> Res<T> + Send + 'static) -> Res<T> {
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| TextbookError::Store(e.to_string()))?
}

fn delta(before: (u64, u64), after: (u64, u64)) -> (u64, u64) {
    (
        after.0.saturating_sub(before.0),
        after.1.saturating_sub(before.1),
    )
}

/// `(imports in the rolling hour, when a slot frees up if the hour is full)`.
fn hour_window(events: &[TextbookEvent], now: i64) -> (u32, Option<i64>) {
    let times = manual_import_times(events, now.saturating_sub(IMPORT_WINDOW_MS));
    let used = times.len() as u32;
    let slot = (used >= MAX_MANUAL_IMPORTS_PER_HOUR)
        .then(|| times.first().map(|t| t + IMPORT_WINDOW_MS))
        .flatten();
    (used, slot)
}

/// One model call, counted in the ledger whether or not it succeeded, because it was sent. Bounded
/// by a timeout and the cancel token (`None` when cancelled).
async fn model_call<E, T, F, Fut>(
    me: &Arc<Inner<E>>,
    book: &str,
    job: &str,
    chapter: Option<u32>,
    requested: bool,
    cancel: &CancellationToken,
    f: F,
) -> Res<Option<Result<T, CallErr>>>
where
    E: LlmEngine + 'static,
    F: FnOnce(Arc<E>) -> Fut,
    Fut: Future<Output = Result<T, CourseAgentError>>,
{
    let engine = (me.engines)(ROLE);
    let before = engine.usage();
    let fut = f(engine.clone());
    let got = tokio::select! {
        _ = cancel.cancelled() => None,
        r = tokio::time::timeout(CALL_TIMEOUT, fut) => Some(match r {
            Ok(Ok(v)) => Ok(v),
            Ok(Err(CourseAgentError::Engine(_))) => Err(CallErr::Engine),
            Ok(Err(_)) => Err(CallErr::Parse),
            Err(_) => Err(CallErr::Timeout),
        }),
    };
    let used = delta(before, engine.usage());
    let ev = TextbookEvent::Usage {
        ts: (me.clock)(),
        job: job.to_string(),
        chapter,
        role: ROLE.into(),
        tokens_in: used.0,
        tokens_out: used.1,
        calls: 1,
        requested,
    };
    let (m, b) = (me.clone(), book.to_string());
    blk(move || Ok(m.stores.textbooks.append_event(&b, &ev)?)).await?;
    Ok(got)
}

/// Reads `first..=last` (1-based) in 10-page windows off the async threads. The inner `Err` is an
/// extractor message, `Ok(None)` a cancel.
async fn read_pages<E>(
    me: &Arc<Inner<E>>,
    path: &str,
    first: usize,
    last: usize,
    cancel: Option<&CancellationToken>,
) -> Res<Result<Option<Vec<String>>, String>> {
    let mut out = Vec::with_capacity((last + 1).saturating_sub(first));
    let mut at = first;
    while at <= last {
        if cancel.is_some_and(|c| c.is_cancelled()) {
            return Ok(Ok(None));
        }
        let to = (at + MAX_PDF_PAGES_PER_READ - 1).min(last);
        let (pdf, p) = (me.pdf.clone(), path.to_string());
        let got = tokio::task::spawn_blocking(move || pdf.page_texts(&p, at, to))
            .await
            .map_err(|e| TextbookError::Store(e.to_string()))?;
        match got {
            Ok(v) if v.len() == to - at + 1 => out.extend(v.iter().map(|t| sanitize_page(t))),
            Ok(_) => return Ok(Err("PDF extractor returned an unexpected page count".into())),
            Err(msg) => return Ok(Err(msg)),
        }
        at = to + 1;
    }
    Ok(Ok(Some(out)))
}

fn short(s: &str, max: usize) -> String {
    sanitize_page(s)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(max)
        .collect()
}

fn cards_for(book: &str, objectives: &[Objective]) -> Vec<FlashCard> {
    let others: Vec<&str> = objectives
        .iter()
        .flat_map(|o| &o.items)
        .map(|i| i.answer.as_str())
        .collect();
    objectives
        .iter()
        .flat_map(|o| &o.items)
        .filter_map(|i| card_for(&card_id(book, &i.id), i, &others))
        .collect()
}

impl<E: LlmEngine + 'static> TextbookService<E> {
    /// Does not recover: the service is built lazily, so the server calls [`Self::recover`] when
    /// a workspace opens. `_root` is the workspace root `stores` was opened on.
    pub fn new(
        _root: PathBuf,
        stores: Arc<Stores>,
        engines: Engines<E>,
        pdf: Arc<dyn Pdf>,
        device: String,
        notify: Arc<dyn Fn(JobRecord) + Send + Sync>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                stores,
                engines,
                pdf,
                device,
                notify,
                clock: Arc::new(crate::now_ms),
                running: Mutex::default(),
                locks: Mutex::default(),
                uploads: Mutex::default(),
            }),
        }
    }

    /// Replaces the wall clock (epoch ms), for tests.
    pub fn with_clock(mut self, clock: Clock) -> Self {
        if let Some(i) = Arc::get_mut(&mut self.inner) {
            i.clock = clock;
        }
        self
    }

    // -------------------------------------------------------------- upload and import

    /// Stores an uploaded PDF (the route already checked size, name and magic) and maps it into
    /// chapters: the outline costs no model call, a book without one costs one TOC call. Nothing
    /// else is imported; a chapter is imported when the learner opens it. The same bytes twice
    /// find the same book: nothing is written again.
    pub async fn upload(&self, file_name: &str, pdf: Bytes, busy: BusyGuard) -> Res<Upload> {
        let _busy = busy;
        let hex = hex::encode(Sha256::digest(&pdf));
        let book = book_id(&hex);
        let stem = std::path::Path::new(file_name)
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("Untitled");
        let title = match short(stem, 200) {
            t if t.is_empty() => "Untitled".to_string(),
            t => t,
        };
        let meta = BookMeta {
            id: book.clone(),
            title,
            file_name: short(file_name, 200),
            bytes: pdf.len() as u64,
            sha256: hex,
            total_pages: 0,
            page_offset: None,
        };
        // The whole upload of these bytes is serialized, mapping included: a second identical
        // upload waits, then finds a fully mapped book (or retries if the first one failed and
        // removed what it had created), never a half-made one the first may still delete.
        let upload_lock = self
            .inner
            .uploads
            .lock()
            .entry(book.clone())
            .or_default()
            .clone();
        let _one_at_a_time = upload_lock.lock().await;
        let me = self.inner.clone();
        let b = book.clone();
        let fresh = blk(move || {
            // One upload of these bytes at a time: the second caller finds the ledger.
            let lk = me.lock_for(&b);
            let _g = lk.lock();
            if me.stores.textbooks.exists(&b)? {
                return Ok(false);
            }
            me.stores.textbooks.save_source(&b, &pdf)?;
            let path = me.stores.textbooks.source_path(&b)?;
            let counted = me.pdf.page_count(&path.to_string_lossy());
            let total = match counted {
                Ok(n) if n > 0 && n <= u32::MAX as usize => n as u32,
                Ok(_) => {
                    me.stores.textbooks.remove(&b)?;
                    return Err(TextbookError::Invalid("This PDF has no pages".into()));
                }
                Err(msg) => {
                    me.stores.textbooks.remove(&b)?;
                    return Err(TextbookError::Invalid(msg));
                }
            };
            let ev = TextbookEvent::Imported {
                ts: (me.clock)(),
                book: BookMeta {
                    total_pages: total,
                    ..meta
                },
            };
            me.stores.textbooks.append_event(&b, &ev)?;
            Ok(true)
        })
        .await?;
        if !fresh {
            return Ok(Upload {
                book,
                existing: true,
            });
        }
        if let Err(e) = self.map_book(&book).await {
            // A book that cannot be mapped is not kept: the same bytes can be uploaded again.
            let (me, b) = (self.inner.clone(), book.clone());
            let _ = blk(move || Ok(me.stores.textbooks.remove(&b)?)).await;
            return Err(e);
        }
        Ok(Upload {
            book,
            existing: false,
        })
    }

    /// Maps the book: outline (0 calls), else one TOC call, else fixed chunks. Appends `Mapped`,
    /// then skips front and back matter. Reads at most `MAP_PAGES` pages, only without an outline.
    async fn map_book(&self, book: &str) -> Res<()> {
        let me = &self.inner;
        let (m, b) = (me.clone(), book.to_string());
        let (st, path) = blk(move || {
            let st = m.state(&b)?;
            let path = m.stores.textbooks.source_path(&b)?;
            Ok((st, path.to_string_lossy().into_owned()))
        })
        .await?;
        if st.source.is_some() {
            return Ok(());
        }
        let total = st
            .meta
            .as_ref()
            .ok_or_else(|| TextbookError::NotFound(book.into()))?
            .total_pages;
        let (pdf, p) = (me.pdf.clone(), path.clone());
        let outline = tokio::task::spawn_blocking(move || pdf.outline(&p))
            .await
            .map_err(|e| TextbookError::Store(e.to_string()))?
            .unwrap_or_else(|e| {
                tracing::warn!("outline unreadable: {e}");
                Vec::new()
            });
        let mut mapped = chapters_from_outline(&outline, total);
        let mut source = MapSource::Outline;
        if mapped.len() < 2 {
            let sample = (total as usize).min(MAP_PAGES);
            let pages = match read_pages(me, &path, 1, sample, None).await? {
                Ok(v) => v.unwrap_or_default(),
                Err(msg) => return Err(TextbookError::Invalid(msg)),
            };
            if pages.iter().all(|p| p.trim().is_empty()) {
                return Err(TextbookError::Invalid(NO_TEXT.into()));
            }
            let offset = printed_offset(&pages);
            mapped = Vec::new();
            let head: Vec<(u32, &str)> = pages
                .iter()
                .take(TOC_PAGES)
                .enumerate()
                .map(|(i, t)| (i as u32 + 1, t.as_str()))
                .collect();
            let text = page_block(&head, usize::MAX);
            let job = format!("map_{}", &book[3..]);
            let got = model_call(
                me,
                book,
                &job,
                None,
                false,
                &CancellationToken::new(),
                |e| async move { toc_chapters(&*e, &text).await },
            )
            .await?;
            if let Some(Ok(entries)) = got {
                mapped = chapters_from_toc(&entries, offset, &pages, total);
                source = MapSource::Toc;
            }
            if mapped.len() < 2 {
                mapped = chunk_chapters(total);
                source = MapSource::Chunks;
            }
        }
        let skipped: Vec<u32> = mapped
            .iter()
            .filter(|c| c.kind != ChapterKind::Chapter)
            .map(|c| c.index)
            .collect();
        let (m, b) = (me.clone(), book.to_string());
        blk(move || {
            let ts = (m.clock)();
            m.stores.textbooks.append_event(
                &b,
                &TextbookEvent::Mapped {
                    ts,
                    source,
                    chapters: mapped,
                },
            )?;
            for chapter in skipped {
                m.stores.textbooks.append_event(
                    &b,
                    &TextbookEvent::ChapterSkipped {
                        ts,
                        chapter,
                        reason: "front_back_matter".into(),
                    },
                )?;
            }
            Ok(())
        })
        .await
    }

    /// Starts an import of the chapters in `chapters`, or of every chapter still pending; returns
    /// the job id. Each chapter is one manual import against the hourly limit.
    pub async fn import(
        &self,
        book: &str,
        chapters: Option<Vec<u32>>,
        busy: BusyGuard,
    ) -> Res<String> {
        let me = self.inner.clone();
        if me.running.lock().contains_key(book) {
            return Err(TextbookError::Conflict(conflict::IMPORT_RUNNING.into()));
        }
        let b = book.to_string();
        let m2 = me.clone();
        let (state, events) = blk(move || m2.load(&b)).await?;
        let (used, _) = hour_window(&events, (me.clock)());
        if used >= MAX_MANUAL_IMPORTS_PER_HOUR {
            return Err(TextbookError::Conflict(conflict::RATE_LIMITED.into()));
        }
        if state.source.is_none() {
            // A book stored before it was mapped (a crash during upload): map it now.
            self.map_book(book).await?;
        }
        let (m3, b) = (me.clone(), book.to_string());
        let (state, n) = blk(move || {
            let st = m3.known(&b)?;
            let prefix = format!("tbj_{}_", &b[3..]);
            let (jobs, _) = m3.stores.jobs.all_report()?;
            let n = jobs
                .iter()
                .filter(|j| j.kind == JobKind::Textbook && j.id.starts_with(&prefix))
                .count();
            Ok((st, n))
        })
        .await?;
        // Each chapter once, in the order given: a repeated index must not repeat its calls.
        let chapters = chapters.map(|list| {
            let mut seen = HashSet::new();
            list.into_iter()
                .filter(|i| seen.insert(*i))
                .collect::<Vec<u32>>()
        });
        match &chapters {
            Some(list) => {
                if list.len() > MAX_CHAPTERS {
                    return Err(TextbookError::Invalid("too many chapters".into()));
                }
                if list.is_empty() {
                    return Err(TextbookError::Invalid("no chapters given".into()));
                }
                for i in list {
                    match state.chapters.get(*i as usize) {
                        None => return Err(TextbookError::Invalid(format!("no chapter {i}"))),
                        Some(v) if v.state == ChapterState::Ready => {
                            return Err(TextbookError::Invalid(format!(
                                "chapter {i} is already imported"
                            )))
                        }
                        Some(v) if v.state == ChapterState::Skipped => {
                            return Err(TextbookError::Invalid(format!(
                                "chapter {i} has nothing to import"
                            )))
                        }
                        Some(v) if v.state == ChapterState::Failed && !v.retryable => {
                            return Err(TextbookError::Conflict(conflict::RETRY_USED.into()))
                        }
                        Some(_) => {}
                    }
                }
            }
            None => {
                if pending_chapters(&state).is_empty() {
                    return Err(TextbookError::Conflict(conflict::NOTHING_TO_IMPORT.into()));
                }
            }
        }
        let meta = state
            .meta
            .clone()
            .ok_or_else(|| TextbookError::NotFound(book.into()))?;
        let id = format!("tbj_{}_{n}", &book[3..]);
        let rec = JobRecord {
            id: id.as_str().into(),
            chat_id: "".into(),
            kind: JobKind::Textbook,
            status: JobStatus::Running,
            delivery: DeliveryStatus::Claimed,
            created_at: (me.clock)(),
            result: Some(progress_json(&meta, &Progress::default())),
            usage: vec![],
        };
        let cancel = CancellationToken::new();
        {
            let mut running = me.running.lock();
            if running.contains_key(book) {
                return Err(TextbookError::Conflict(conflict::IMPORT_RUNNING.into()));
            }
            running.insert(
                book.to_string(),
                Running {
                    job: id.clone(),
                    cancel: cancel.clone(),
                    current: None,
                    ending: false,
                },
            );
        }
        // Removes the entry on every exit, a failed `create_job` and a panicking job included.
        let running_guard = RunningGuard {
            me: me.clone(),
            book: book.to_string(),
            job: id.clone(),
        };
        let (m3, r3) = (me.clone(), rec.clone());
        blk(move || Ok(m3.stores.jobs.create_job(&r3)?)).await?;
        (me.notify)(rec.clone());
        let job = Job {
            me: me.clone(),
            book: book.to_string(),
            rec,
            meta,
            progress: Progress::default(),
            cancel,
            only: chapters,
            calls: AtomicU32::new(0),
        };
        tokio::spawn(async move {
            let _busy = busy;
            let _running = running_guard;
            job.run().await;
        });
        Ok(id)
    }

    /// Cancels a job this service runs. False if it is not one of ours.
    pub fn abort(&self, job_id: &str) -> bool {
        match self
            .inner
            .running
            .lock()
            .values()
            .find(|r| r.job == job_id && !r.ending)
        {
            Some(r) => {
                r.cancel.cancel();
                true
            }
            None => false,
        }
    }

    /// Restart repair: textbook jobs left `Running` by a dead process end as `Done` with an error,
    /// and every `Ready` chapter gets any missing review card. The ledger is untouched.
    pub async fn recover(&self) -> Res<usize> {
        let me = self.inner.clone();
        blk(move || {
            let live: HashSet<String> = me.running.lock().values().map(|r| r.job.clone()).collect();
            let mut n = 0;
            for mut rec in me.stores.jobs.interrupted_jobs()? {
                if rec.kind != JobKind::Textbook || live.contains(rec.id.as_str()) {
                    continue;
                }
                rec.status = JobStatus::Done;
                let mut result = rec.result.take().unwrap_or_else(|| json!({}));
                result["error"] = json!("interrupted by a restart");
                rec.result = Some(result);
                me.stores.jobs.update_job(&rec)?;
                (me.notify)(rec);
                n += 1;
            }
            for book in me.stores.textbooks.list()? {
                let st = me.state(&book)?;
                me.reconcile(&book, &st, None)?;
            }
            Ok(n)
        })
        .await
    }

    // -------------------------------------------------------------- reads

    pub async fn list(&self) -> Res<Vec<BookSummary>> {
        let me = self.inner.clone();
        blk(move || {
            let mut out = Vec::new();
            for book in me.stores.textbooks.list()? {
                let st = me.state(&book)?;
                let Some(meta) = st.meta.clone() else {
                    continue;
                };
                let running = me.running.lock().contains_key(&book);
                let ready = st
                    .chapters
                    .iter()
                    .filter(|v| v.state == ChapterState::Ready)
                    .count();
                let targets = st
                    .chapters
                    .iter()
                    .filter(|v| v.chapter.kind == ChapterKind::Chapter)
                    .filter(|v| v.state != ChapterState::Skipped)
                    .count();
                let failed = st.chapters.iter().any(|v| v.state == ChapterState::Failed);
                let phase = if running {
                    BookPhase::Importing
                } else if st.source.is_none() || (ready == 0 && failed) {
                    BookPhase::Failed
                } else if ready > 0 && ready >= targets {
                    BookPhase::Ready
                } else {
                    BookPhase::Partial
                };
                out.push(BookSummary {
                    id: book,
                    title: meta.title,
                    total_pages: meta.total_pages,
                    chapters: st.chapters.len(),
                    ready,
                    state: phase,
                    created_at: st.created_at.unwrap_or(0),
                });
            }
            out.sort_by(|a, b| b.created_at.cmp(&a.created_at).then(a.id.cmp(&b.id)));
            Ok(out)
        })
        .await
    }

    pub async fn book(&self, book: &str) -> Res<BookView> {
        let (me, b) = (self.inner.clone(), book.to_string());
        blk(move || {
            let (st, events) = me.load(&b)?;
            let (hour_used, _) = hour_window(&events, (me.clock)());
            let due = me.due_cards(&b)?;
            let meta = st
                .meta
                .clone()
                .ok_or_else(|| TextbookError::NotFound(b.clone()))?;
            let (job, current) = {
                let running = me.running.lock();
                let r = running.get(&b);
                (r.map(|r| r.job.clone()), r.and_then(|r| r.current))
            };
            let chapters = st
                .chapters
                .iter()
                .map(|v| row_of(v, &b, &due, job.is_some(), current))
                .collect();
            Ok(BookView {
                meta,
                source: st.source,
                chapters,
                job,
                auto_calls: st.auto_calls,
                requested_calls: st.requested_calls,
                hour_used,
            })
        })
        .await
    }

    pub async fn chapter(&self, book: &str, chapter: u32) -> Res<ChapterDetail> {
        let (me, b) = (self.inner.clone(), book.to_string());
        blk(move || {
            let st = me.known(&b)?;
            let due = me.due_cards(&b)?;
            let v = st
                .chapters
                .get(chapter as usize)
                .ok_or_else(|| TextbookError::NotFound(format!("chapter {chapter}")))?;
            let (running, current) = {
                let r = me.running.lock();
                let r = r.get(&b);
                (r.is_some(), r.and_then(|r| r.current))
            };
            Ok(ChapterDetail {
                row: row_of(v, &b, &due, running, current),
                objectives: v
                    .objectives
                    .iter()
                    .map(|o| ObjectiveRow {
                        id: o.id.clone(),
                        text: o.text.clone(),
                        pages: o.pages,
                        items: o.items.len(),
                    })
                    .collect(),
            })
        })
        .await
    }

    pub async fn usage(&self, book: &str) -> Res<TextbookUsage> {
        let (me, b) = (self.inner.clone(), book.to_string());
        blk(move || {
            let (st, events) = me.load(&b)?;
            let (hour_used, next_slot_at) = hour_window(&events, (me.clock)());
            Ok(TextbookUsage {
                cheap_in: st.cheap_in,
                cheap_out: st.cheap_out,
                auto_calls: st.auto_calls,
                requested_calls: st.requested_calls,
                hour_used,
                hour_cap: MAX_MANUAL_IMPORTS_PER_HOUR,
                next_slot_at,
            })
        })
        .await
    }

    // -------------------------------------------------------------- reading check

    /// "I read chapter N": a quiz from stored items. No engine is touched.
    pub async fn start_check(&self, book: &str, chapter: u32) -> Res<CheckView> {
        let (me, b) = (self.inner.clone(), book.to_string());
        blk(move || {
            let lk = me.lock_for(&b);
            let _g = lk.lock();
            let st = me.known(&b)?;
            let v = st
                .chapters
                .get(chapter as usize)
                .ok_or_else(|| TextbookError::NotFound(format!("chapter {chapter}")))?;
            if v.state != ChapterState::Ready {
                return Err(TextbookError::Conflict(conflict::CHAPTER_NOT_READY.into()));
            }
            me.reconcile(&b, &st, Some(chapter))?;
            let prefix = format!("t:{b}:");
            let now = (me.clock)();
            let due: BTreeSet<String> = review::due_cards(&*me.stores.tracker, now)?
                .into_iter()
                .map(|c| c.id.to_string())
                .filter(|id| id.starts_with(&prefix))
                .collect();
            let reviewed: BTreeSet<String> = me
                .stores
                .tracker
                .reviews()?
                .into_iter()
                .map(|r| r.card_id.to_string())
                .filter(|id| id.starts_with(&prefix))
                .collect();
            let check = format!("rc_{}", uuid::Uuid::new_v4().simple());
            let ids = select_check(&st, chapter, &due, &reviewed, &check);
            let mut items = Vec::new();
            for id in &ids {
                for o in &v.objectives {
                    if let Some(it) = o.items.iter().find(|i| &i.id == id) {
                        items.push(CheckItem {
                            id: it.id.clone(),
                            objective: o.id.clone(),
                            kind: it.kind,
                            q: it.q.clone(),
                            options: match it.kind {
                                ItemKind::Mcq => shuffled(&it.options, &card_id(&b, &it.id)),
                                ItemKind::Recall => Vec::new(),
                            },
                        });
                    }
                }
            }
            me.stores.textbooks.append_event(
                &b,
                &TextbookEvent::CheckIssued {
                    ts: now,
                    check: check.clone(),
                    chapter,
                    items: ids,
                },
            )?;
            Ok(CheckView {
                check,
                chapter,
                items,
            })
        })
        .await
    }

    /// Grades one answer by code. A retry with the same `client_id` gets the stored result.
    pub async fn answer(
        &self,
        book: &str,
        check: &str,
        item: &str,
        text: &str,
        meta: AnswerMeta,
    ) -> Res<CheckFeedback> {
        let AnswerMeta {
            confidence,
            ms,
            client_id,
        } = meta;
        if text.chars().count() > MAX_TEXT_CHARS {
            return Err(TextbookError::Invalid(format!(
                "the answer must be at most {MAX_TEXT_CHARS} characters"
            )));
        }
        if confidence.is_some_and(|c| !(1..=5).contains(&c)) {
            return Err(TextbookError::Invalid("confidence is 1 to 5".into()));
        }
        let _ = ms; // the ledger line has no latency field; FSRS takes the rating only
        let (me, b) = (self.inner.clone(), book.to_string());
        let (check, item, text) = (check.to_string(), item.to_string(), text.to_string());
        blk(move || {
            let lk = me.lock_for(&b);
            let _g = lk.lock();
            let events = me.stores.textbooks.events(&b)?;
            let st = fold_book(&events);
            if st.meta.is_none() {
                return Err(TextbookError::NotFound(b));
            }
            let issued = st
                .checks
                .iter()
                .find(|(c, _, _)| *c == check)
                .filter(|(_, _, items)| items.contains(&item))
                .ok_or_else(|| TextbookError::NotFound(format!("check {check}")))?;
            let it = st
                .chapters
                .get(issued.1 as usize)
                .and_then(|v| {
                    v.objectives
                        .iter()
                        .flat_map(|o| &o.items)
                        .find(|i| i.id == item)
                })
                .ok_or_else(|| TextbookError::NotFound(format!("item {item}")))?;
            let card = card_id(&b, &item);
            let first = events.iter().find_map(|e| match e {
                TextbookEvent::Answered {
                    check: c,
                    item: i,
                    correct,
                    close,
                    client_id: cid,
                    ..
                } if *c == check && *i == item => Some((*correct, *close, cid.clone())),
                _ => None,
            });
            if let Some((correct, close, stored)) = first {
                if client_id.is_some() && client_id == stored {
                    return Ok(feedback(&me, &card, it, correct, close));
                }
                return Err(TextbookError::Conflict(conflict::ALREADY_ANSWERED.into()));
            }
            let (correct, close) = grade_answer(it, &text);
            // Only a close typed answer waits for the learner's own grade; a plain miss (or "I
            // don't know") is rated at once, or the card would stay parked and never come back.
            let rating = (it.kind == ItemKind::Mcq || correct || !close)
                .then(|| lp_core::course::rating_for(correct, confidence));
            me.stores.textbooks.append_event(
                &b,
                &TextbookEvent::Answered {
                    ts: (me.clock)(),
                    check: check.clone(),
                    item: item.clone(),
                    correct,
                    close,
                    rating,
                    client_id,
                },
            )?;
            if let Some(r) = rating {
                me.grade_card(&card, r)?;
            }
            Ok(feedback(&me, &card, it, correct, close))
        })
        .await
    }

    /// The learner's own grade for a typed answer that was close.
    pub async fn self_grade(&self, book: &str, check: &str, item: &str, grade: &str) -> Res<()> {
        let rating = lp_core::course::rating_from_grade(grade)
            .ok_or_else(|| TextbookError::Invalid("grade is again, hard, good or easy".into()))?;
        let (me, b) = (self.inner.clone(), book.to_string());
        let (check, item) = (check.to_string(), item.to_string());
        blk(move || {
            let lk = me.lock_for(&b);
            let _g = lk.lock();
            let events = me.stores.textbooks.events(&b)?;
            let st = fold_book(&events);
            let answered = events.iter().any(|e| {
                matches!(e, TextbookEvent::Answered { check: c, item: i, .. } if *c == check && *i == item)
            });
            if !answered {
                return Err(TextbookError::NotFound(format!("answer for {item}")));
            }
            let graded = events.iter().any(|e| {
                matches!(e, TextbookEvent::Answered { check: c, item: i, rating: Some(_), .. } if *c == check && *i == item)
            });
            let recall = st
                .checks
                .iter()
                .find(|(c, _, _)| *c == check)
                .and_then(|(_, ch, _)| st.chapters.get(*ch as usize))
                .and_then(|v| v.objectives.iter().flat_map(|o| &o.items).find(|i| i.id == item))
                .is_some_and(|i| i.kind == ItemKind::Recall);
            if graded || !recall {
                return Err(TextbookError::Conflict(conflict::ALREADY_ANSWERED.into()));
            }
            // A second line for the same pair: the fold keeps the first, this one only records the grade.
            me.stores.textbooks.append_event(
                &b,
                &TextbookEvent::Answered {
                    ts: (me.clock)(),
                    check,
                    item: item.clone(),
                    correct: false,
                    close: false,
                    rating: Some(rating),
                    client_id: None,
                },
            )?;
            me.grade_card(&card_id(&b, &item), rating)
        })
        .await
    }
}

/// `(correct, close)` for a textbook answer: a recall answer that names every `must_name` term is
/// correct (code, no model); otherwise the exact or close match on the stored answer.
fn grade_answer(it: &lp_core::course::Item, text: &str) -> (bool, bool) {
    use lp_core::method::recall::{rubric_check, RubricResult};
    if it.kind == ItemKind::Recall && rubric_check(it, text) == RubricResult::Match {
        return (true, false);
    }
    grade(it, text)
}

fn feedback<E>(
    me: &Inner<E>,
    card: &str,
    it: &lp_core::course::Item,
    correct: bool,
    close: bool,
) -> CheckFeedback {
    CheckFeedback {
        correct,
        close,
        answer: it.answer.clone(),
        why: it.why.clone(),
        due: me
            .stores
            .tracker
            .card(card)
            .ok()
            .flatten()
            .map(|c| c.card.due),
    }
}

fn row_of(
    v: &ChapterView,
    book: &str,
    due: &BTreeSet<String>,
    running: bool,
    current: Option<u32>,
) -> ChapterRow {
    let state = match v.state {
        ChapterState::Ready => ChapterRowState::Ready,
        ChapterState::Failed => ChapterRowState::Failed,
        ChapterState::Skipped => ChapterRowState::Skipped,
        ChapterState::Pending if running && current == Some(v.chapter.index) => {
            ChapterRowState::Importing
        }
        ChapterState::Pending => ChapterRowState::Pending,
    };
    let can_import = match v.state {
        ChapterState::Pending => v.chapter.kind == ChapterKind::Chapter,
        ChapterState::Failed => v.retryable,
        ChapterState::Ready | ChapterState::Skipped => false,
    };
    ChapterRow {
        index: v.chapter.index,
        title: v.chapter.title.clone(),
        pages: v.chapter.pages,
        kind: v.chapter.kind,
        state,
        objectives: v.objectives.len(),
        status: chapter_status(v, due, book),
        error: v.error.clone(),
        truncated: v.truncated,
        can_import,
    }
}

impl<E> Inner<E> {
    fn lock_for(&self, book: &str) -> Arc<Mutex<()>> {
        self.locks
            .lock()
            .entry(book.to_string())
            .or_default()
            .clone()
    }

    fn state(&self, book: &str) -> Res<BookState> {
        Ok(fold_book(&self.stores.textbooks.events(book)?))
    }

    /// The fold and the raw ledger of a book that exists, else `NotFound`.
    fn load(&self, book: &str) -> Res<(BookState, Vec<TextbookEvent>)> {
        if !valid_book_id(book) {
            return Err(TextbookError::NotFound(book.into()));
        }
        let events = self.stores.textbooks.events(book)?;
        let st = fold_book(&events);
        if st.meta.is_none() {
            return Err(TextbookError::NotFound(book.into()));
        }
        Ok((st, events))
    }

    /// The fold of a book that exists, else `NotFound` (also for a directory with no `Imported`).
    fn known(&self, book: &str) -> Res<BookState> {
        if !valid_book_id(book) {
            return Err(TextbookError::NotFound(book.into()));
        }
        let st = self.state(book)?;
        if st.meta.is_none() {
            return Err(TextbookError::NotFound(book.into()));
        }
        Ok(st)
    }

    fn due_cards(&self, book: &str) -> Res<BTreeSet<String>> {
        let prefix = format!("t:{book}:");
        Ok(review::due_cards(&*self.stores.tracker, (self.clock)())?
            .into_iter()
            .map(|c| c.id.to_string())
            .filter(|id| id.starts_with(&prefix))
            .collect())
    }

    /// Gives every item of the (one, or every) `Ready` chapter its card, keeping cards that exist.
    fn reconcile(&self, book: &str, st: &BookState, only: Option<u32>) -> Res<()> {
        let t = &self.stores.tracker;
        for v in &st.chapters {
            if v.state != ChapterState::Ready || only.is_some_and(|c| c != v.chapter.index) {
                continue;
            }
            for card in cards_for(book, &v.objectives) {
                if t.card(&card.id)?.is_none() {
                    t.upsert_card(&card)?;
                }
            }
        }
        Ok(())
    }

    fn grade_card(&self, card: &str, rating: Rating) -> Res<()> {
        match review::grade(
            &*self.stores.tracker,
            &self.device,
            card,
            rating,
            (self.clock)(),
        ) {
            Ok(()) | Err(StoreError::NotFound(_)) => Ok(()),
            Err(e) => Err(e.into()),
        }
    }
}

// ------------------------------------------------------------------ the job

#[derive(Clone, Default)]
struct Progress {
    phase: &'static str,
    pages_read: u32,
    total_pages: u32,
    chapters_done: u32,
    chapters_total: u32,
}

fn progress_json(meta: &BookMeta, p: &Progress) -> Value {
    json!({
        "title": format!("Import {}", meta.title),
        "bookId": meta.id,
        "progress": {
            "phase": if p.phase.is_empty() { "pages" } else { p.phase },
            "pagesRead": p.pages_read,
            "totalPages": if p.total_pages == 0 { meta.total_pages } else { p.total_pages },
            "chaptersDone": p.chapters_done,
            "chaptersTotal": p.chapters_total,
        }
    })
}

enum Outcome {
    Finished {
        stopped: Option<&'static str>,
    },
    /// The job could not run at all; the message never carries page text.
    Failed(String),
    Cancelled,
}

enum CallErr {
    Parse,
    Timeout,
    Engine,
}

enum ChapterResult {
    Done {
        objectives: Vec<Objective>,
        parts: u8,
        truncated: bool,
    },
    Failed {
        reason: &'static str,
    },
    /// The engine refused. `retryable` is false once the retry was spent.
    Engine {
        retryable: bool,
    },
    Cancelled,
}

struct Job<E> {
    me: Arc<Inner<E>>,
    book: String,
    rec: JobRecord,
    meta: BookMeta,
    progress: Progress,
    cancel: CancellationToken,
    only: Option<Vec<u32>>,
    /// Model calls this job has sent, against [`MAX_JOB_CALLS`].
    calls: AtomicU32,
}

impl<E: LlmEngine + 'static> Job<E> {
    async fn run(mut self) {
        let outcome = self.execute().await;
        self.finish(outcome).await;
    }

    async fn publish(&mut self) {
        self.rec.result = Some(progress_json(&self.meta, &self.progress));
        self.write().await;
    }

    async fn write(&self) {
        let (me, rec) = (self.me.clone(), self.rec.clone());
        let r2 = rec.clone();
        if let Err(e) = blk(move || Ok(me.stores.jobs.update_job(&r2)?)).await {
            tracing::warn!("textbook job {} not stored: {e}", rec.id);
        }
        (self.me.notify)(rec);
    }

    fn set_current(&self, chapter: Option<u32>) {
        if let Some(r) = self.me.running.lock().get_mut(&self.book) {
            r.current = chapter;
        }
    }

    async fn state(&self) -> Res<BookState> {
        let (me, b) = (self.me.clone(), self.book.clone());
        blk(move || me.state(&b)).await
    }

    async fn append(&self, ev: TextbookEvent) -> Res<()> {
        let (me, b) = (self.me.clone(), self.book.clone());
        blk(move || Ok(me.stores.textbooks.append_event(&b, &ev)?)).await
    }

    /// One chapter model call: a requested call, counted in the ledger.
    async fn call<T, F, Fut>(&self, chapter: Option<u32>, f: F) -> Res<Option<Result<T, CallErr>>>
    where
        F: FnOnce(Arc<E>) -> Fut,
        Fut: Future<Output = Result<T, CourseAgentError>>,
    {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let job = self.rec.id.to_string();
        model_call(&self.me, &self.book, &job, chapter, true, &self.cancel, f).await
    }

    async fn execute(&mut self) -> Outcome {
        match self.execute_inner().await {
            Ok(o) => o,
            Err(e) => Outcome::Failed(e.to_string()),
        }
    }

    async fn execute_inner(&mut self) -> Res<Outcome> {
        let st = self.state().await?;
        let list: Vec<u32> = match &self.only {
            Some(l) => l.clone(),
            None => pending_chapters(&st),
        };
        self.progress.phase = "objectives";
        self.progress.chapters_total = list.len() as u32;
        self.progress.total_pages = list
            .iter()
            .filter_map(|i| st.chapters.get(*i as usize))
            .map(|v| v.chapter.pages.end + 1 - v.chapter.pages.start)
            .sum();
        self.publish().await;
        let path = self
            .me
            .stores
            .textbooks
            .source_path(&self.book)?
            .to_string_lossy()
            .into_owned();

        let mut engine_errors = 0;
        let mut stopped = None;
        for idx in list {
            if self.cancel.is_cancelled() {
                return Ok(Outcome::Cancelled);
            }
            let (me, b) = (self.me.clone(), self.book.clone());
            let (st, events) = blk(move || me.load(&b)).await?;
            let Some(view) = st.chapters.get(idx as usize) else {
                continue;
            };
            let chapter = view.chapter.clone();
            // Re-checked right before the work: only a pending chapter, or a failed one whose
            // retry is unspent, may call a model. Anything else (a repeat, a skipped chapter, a
            // chapter another run finished) is passed over.
            let runnable = view.state == ChapterState::Pending
                || (view.state == ChapterState::Failed && view.retryable);
            if runnable {
                self.set_current(Some(idx));
                // Only this chapter's pages are read, 10 at a time.
                let (first, last) = (chapter.pages.start as usize, chapter.pages.end as usize);
                let pages =
                    match read_pages(&self.me, &path, first, last, Some(&self.cancel)).await? {
                        Ok(Some(v)) => v,
                        Ok(None) => return Ok(Outcome::Cancelled),
                        Err(msg) => return Ok(Outcome::Failed(msg)),
                    };
                self.progress.pages_read += pages.len() as u32;
                if pages.iter().all(|p| p.trim().is_empty()) {
                    // An image-only chapter (a cover, a part page) can never import: skip it so
                    // it stops offering Import, and still tell the user why.
                    self.append(TextbookEvent::ChapterSkipped {
                        ts: (self.me.clock)(),
                        chapter: idx,
                        reason: "no_text".into(),
                    })
                    .await?;
                    self.set_current(None);
                    return Ok(Outcome::Failed(NO_TEXT.into()));
                }
                let page_words: Vec<(u32, usize)> = pages
                    .iter()
                    .enumerate()
                    .map(|(i, t)| (chapter.pages.start + i as u32, word_count(t)))
                    .collect();
                let words: usize = page_words.iter().map(|(_, w)| *w).sum();
                if words < MIN_CHAPTER_WORDS {
                    self.append(TextbookEvent::ChapterSkipped {
                        ts: (self.me.clock)(),
                        chapter: idx,
                        reason: "too_short".into(),
                    })
                    .await?;
                } else {
                    // The hourly limit is re-read from the ledger before every chapter.
                    if hour_window(&events, (self.me.clock)()).0 >= MAX_MANUAL_IMPORTS_PER_HOUR {
                        stopped = Some("rate_limited");
                        self.set_current(None);
                        break;
                    }
                    let parts = split_parts(&page_words);
                    let worst = parts.len() as u32 * MAX_ATTEMPTS;
                    if self.calls.load(Ordering::SeqCst) + worst > MAX_JOB_CALLS {
                        stopped = Some("call_cap");
                        self.set_current(None);
                        break;
                    }
                    match self.do_chapter(&chapter, &pages, &parts).await? {
                        ChapterResult::Cancelled => return Ok(Outcome::Cancelled),
                        ChapterResult::Engine { retryable } => {
                            self.append(TextbookEvent::ChapterFailed {
                                ts: (self.me.clock)(),
                                chapter: idx,
                                reason: "engine_unavailable".into(),
                                retryable,
                            })
                            .await?;
                            engine_errors += 1;
                            if engine_errors >= 2 {
                                stopped = Some("engine_unavailable");
                            }
                        }
                        ChapterResult::Failed { reason } => {
                            engine_errors = 0;
                            // The try and its retry are both spent.
                            self.append(TextbookEvent::ChapterFailed {
                                ts: (self.me.clock)(),
                                chapter: idx,
                                reason: reason.into(),
                                retryable: false,
                            })
                            .await?;
                        }
                        ChapterResult::Done {
                            objectives,
                            parts,
                            truncated,
                        } => {
                            engine_errors = 0;
                            self.store_chapter(idx, objectives, parts, truncated)
                                .await?;
                        }
                    }
                }
            }
            self.set_current(None);
            self.progress.chapters_done += 1;
            self.publish().await;
            if stopped.is_some() {
                break;
            }
        }
        Ok(Outcome::Finished { stopped })
    }

    /// Writes `ChapterDone` and its cards, or `no_cards` when no item can make one.
    async fn store_chapter(
        &self,
        idx: u32,
        objectives: Vec<Objective>,
        parts: u8,
        truncated: bool,
    ) -> Res<()> {
        let cards = cards_for(&self.book, &objectives);
        if cards.is_empty() {
            return self
                .append(TextbookEvent::ChapterFailed {
                    ts: (self.me.clock)(),
                    chapter: idx,
                    reason: "no_cards".into(),
                    retryable: false,
                })
                .await;
        }
        self.append(TextbookEvent::ChapterDone {
            ts: (self.me.clock)(),
            chapter: idx,
            parts,
            truncated,
            objectives,
        })
        .await?;
        let me = self.me.clone();
        blk(move || {
            for card in &cards {
                if me.stores.tracker.card(&card.id)?.is_none() {
                    me.stores.tracker.upsert_card(card)?;
                }
            }
            Ok(())
        })
        .await
    }

    /// All parts of one chapter: one try plus one retry (`MAX_ATTEMPTS`). Nothing is stored here.
    /// `pages[0]` is the chapter's first page.
    async fn do_chapter(
        &self,
        chapter: &Chapter,
        pages: &[String],
        parts: &[(PageRange, bool)],
    ) -> Res<ChapterResult> {
        let (min, max) = objective_bounds(parts.len());
        let mut last = "parse";
        for attempt in 0..MAX_ATTEMPTS {
            let mut objectives: Vec<Objective> = Vec::new();
            let mut ok = true;
            for (i, (range, _)) in parts.iter().enumerate() {
                let slice: Vec<(u32, &str)> = (range.start..=range.end)
                    .filter_map(|p| {
                        pages
                            .get((p - chapter.pages.start) as usize)
                            .map(|t| (p, t.as_str()))
                    })
                    .collect();
                let text = page_block(&slice, MAX_PART_WORDS);
                let (book, title) = (self.meta.title.clone(), chapter.title.clone());
                let (part, nparts, pr) = (i + 1, parts.len(), *range);
                let got = self
                    .call(Some(chapter.index), |e| async move {
                        let brief = ChapterBrief {
                            book: &book,
                            chapter: &title,
                            part,
                            parts: nparts,
                            pages: pr,
                            text: &text,
                            min,
                            max,
                        };
                        chapter_objectives(&*e, &brief).await
                    })
                    .await?;
                match got {
                    None => return Ok(ChapterResult::Cancelled),
                    Some(Ok(raw)) => {
                        let n = objectives.len() + 1;
                        objectives.extend(accept_objectives(chapter.index, n, *range, raw));
                    }
                    Some(Err(CallErr::Engine)) => {
                        return Ok(ChapterResult::Engine {
                            retryable: attempt == 0,
                        })
                    }
                    Some(Err(CallErr::Timeout)) => {
                        last = "timeout";
                        ok = false;
                        break;
                    }
                    Some(Err(CallErr::Parse)) => {
                        last = "parse";
                        ok = false;
                        break;
                    }
                }
            }
            if ok && objectives.len() >= MIN_KEPT_OBJECTIVES {
                return Ok(ChapterResult::Done {
                    objectives,
                    parts: parts.len() as u8,
                    truncated: parts.iter().any(|(_, cut)| *cut),
                });
            }
            if ok {
                last = "too_few_objectives";
            }
        }
        Ok(ChapterResult::Failed { reason: last })
    }

    async fn finish(&mut self, outcome: Outcome) {
        let st = self.state().await.unwrap_or_default();
        let count = |s: ChapterState| st.chapters.iter().filter(|v| v.state == s).count();
        let pending = st
            .chapters
            .iter()
            .filter(|v| v.state == ChapterState::Pending && v.chapter.kind == ChapterKind::Chapter)
            .count();
        let mut result = progress_json(&self.meta, &self.progress);
        result["summary"] = json!({
            "ready": count(ChapterState::Ready),
            "failed": count(ChapterState::Failed),
            "skipped": count(ChapterState::Skipped),
            "pending": pending,
            "calls": st.auto_calls + st.requested_calls,
        });
        self.rec.status = JobStatus::Done;
        match outcome {
            Outcome::Finished { stopped } => {
                if let Some(s) = stopped {
                    result["stopped"] = json!(s);
                }
            }
            Outcome::Failed(msg) => result["error"] = json!(msg),
            Outcome::Cancelled => {
                self.rec.status = JobStatus::Failed;
                result["error"] = json!("cancelled");
            }
        }
        self.rec.result = Some(result);
        // Ending: an abort that arrives now finds a finished job (409 upstream), not a live one.
        if let Some(r) = self.me.running.lock().get_mut(&self.book) {
            r.ending = true;
        }
        self.write().await;
        self.me.running.lock().remove(&self.book);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::workspace::{NewWorkspace, NoopSink, WorkspaceRegistry};
    use async_trait::async_trait;
    use lp_cli_driver::engine::{
        DriverError, StreamDelta, StructuredOutput, StructuredRequest, TurnRequest, TurnResponse,
    };
    use schemars::JsonSchema;
    use serde::de::DeserializeOwned;
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
    use tokio::sync::mpsc;

    // ---------------------------------------------------------------- fakes

    struct FakePdf {
        pages: Vec<String>,
        outline: Vec<OutlineEntry>,
        /// Page texts fail from this 1-based page on.
        fail_from: Option<usize>,
        /// Every `page_texts` call, as `(first, last)`: one entry is one extractor spawn.
        reads: Mutex<Vec<(usize, usize)>>,
    }

    impl Pdf for FakePdf {
        fn pages(&self, _path: &str) -> Result<Vec<String>, String> {
            Ok(self.pages.clone())
        }
        fn page_count(&self, _path: &str) -> Result<usize, String> {
            Ok(self.pages.len())
        }
        fn page_texts(&self, _p: &str, first: usize, last: usize) -> Result<Vec<String>, String> {
            self.reads.lock().push((first, last));
            if self.fail_from.is_some_and(|f| last >= f) {
                return Err("PDF extraction timed out; try a smaller page range".into());
            }
            Ok(self.pages[first - 1..last].to_vec())
        }
        fn outline(&self, _p: &str) -> Result<Vec<OutlineEntry>, String> {
            Ok(self.outline.clone())
        }
    }

    type Reply = Box<dyn Fn(usize, &str) -> Result<String, DriverError> + Send + Sync>;

    struct Mock {
        calls: AtomicUsize,
        hold_from: AtomicUsize,
        reply: Reply,
        prompts: Mutex<Vec<String>>,
    }

    impl Mock {
        fn n(&self) -> usize {
            self.calls.load(Ordering::SeqCst)
        }
        fn toc_calls(&self) -> usize {
            self.prompts
                .lock()
                .iter()
                .filter(|p| p.contains("printed_page"))
                .count()
        }
    }

    #[async_trait]
    impl LlmEngine for Mock {
        async fn stream_turn(
            &self,
            req: TurnRequest,
            _tx: mpsc::Sender<StreamDelta>,
            _c: CancellationToken,
        ) -> Result<TurnResponse, DriverError> {
            let n = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
            self.prompts.lock().push(req.prompt.clone());
            let hold = self.hold_from.load(Ordering::SeqCst);
            if hold > 0 && n >= hold {
                tokio::time::sleep(Duration::from_secs(30)).await;
            }
            Ok(TurnResponse {
                text: (self.reply)(n, &req.prompt)?,
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
            let n = self.n() as u64;
            (n * 100, n * 10)
        }
    }

    fn range_of(prompt: &str) -> (u32, u32) {
        let at = prompt.find("covering pages ").map_or(0, |i| i + 15);
        let mut it = prompt[at..].split_whitespace();
        let a = it.next().and_then(|s| s.parse().ok()).unwrap_or(1);
        let b = it
            .nth(1)
            .and_then(|s| s.trim_end_matches('.').parse().ok())
            .unwrap_or(a);
        (a, b)
    }

    fn good(prompt: &str) -> String {
        let (a, b) = range_of(prompt);
        let objs: Vec<Value> = (1..=6)
            .map(|i| {
                json!({"text": format!("Explain concept {i} of this part"),
                "start_page": a, "end_page": b,
                "items": [
                    {"type":"mcq","q":format!("Which statement {i} about page {a} holds true here?"),
                     "options":[format!("alpha {i}"),format!("bravo {i}"),format!("charlie {i}"),format!("delta {i}")],
                     "answer":format!("alpha {i}"),"why":format!("because {i}")},
                    {"type":"recall","q":format!("Name concept {i} of page {a}"),
                     "answer":format!("term {i} of {a}"),"accept":[],"why":"because","must_name":[]}]})
            })
            .collect();
        json!({ "objectives": objs }).to_string()
    }

    fn words(n: usize) -> String {
        "ab ".repeat(n)
    }

    /// `n` one-page chapters of `w` words with an outline.
    fn book(n: usize, w: usize) -> FakePdf {
        FakePdf {
            pages: (1..=n).map(|i| format!("{}\n{i}", words(w))).collect(),
            outline: (1..=n)
                .map(|i| OutlineEntry {
                    title: format!("Chapter {i}"),
                    page: i as u32,
                    depth: 0,
                })
                .collect(),
            fail_from: None,
            reads: Mutex::default(),
        }
    }

    struct Rig {
        svc: TextbookService<Mock>,
        stores: Arc<Stores>,
        engine: Arc<Mock>,
        notes: Arc<Mutex<Vec<JobRecord>>>,
        handle: Arc<crate::workspace::WorkspaceHandle>,
        /// Milliseconds the test clock is ahead of its start.
        skew: Arc<std::sync::atomic::AtomicI64>,
        _dir: tempfile::TempDir,
        n: AtomicUsize,
    }

    fn rig_with(pdf: Arc<dyn Pdf>, reply: Reply) -> Rig {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("ws");
        std::fs::create_dir_all(&root).unwrap();
        let stores = Arc::new(Stores::open(&root).unwrap());
        let engine = Arc::new(Mock {
            calls: AtomicUsize::new(0),
            hold_from: AtomicUsize::new(0),
            reply,
            prompts: Mutex::default(),
        });
        let notes: Arc<Mutex<Vec<JobRecord>>> = Arc::default();
        let (e2, n2) = (engine.clone(), notes.clone());
        let engines: Engines<Mock> = Arc::new(move |_role: &str| e2.clone());
        let skew = Arc::new(std::sync::atomic::AtomicI64::new(0));
        let (start, k2) = (crate::now_ms(), skew.clone());
        let svc = TextbookService::new(
            root,
            stores.clone(),
            engines,
            pdf,
            "dev".into(),
            Arc::new(move |r| n2.lock().push(r)),
        )
        .with_clock(Arc::new(move || start + k2.load(Ordering::SeqCst)));
        let reg = WorkspaceRegistry::new(dir.path().join("reg"), Arc::new(NoopSink)).unwrap();
        let handle = reg
            .create(NewWorkspace {
                name: "t".into(),
                ..NewWorkspace::default()
            })
            .unwrap();
        Rig {
            svc,
            stores,
            engine,
            notes,
            handle,
            skew,
            _dir: dir,
            n: AtomicUsize::new(0),
        }
    }

    fn rig(pdf: FakePdf) -> Rig {
        rig_with(Arc::new(pdf), Box::new(|_, p| Ok(good(p))))
    }

    impl Rig {
        fn guard(&self) -> BusyGuard {
            self.handle.busy()
        }
        fn bytes(&self) -> Bytes {
            let k = self.n.load(Ordering::SeqCst);
            Bytes::from(format!("%PDF-fake-{k}"))
        }
        async fn upload(&self) -> Upload {
            self.svc
                .upload("My Book.pdf", self.bytes(), self.guard())
                .await
                .unwrap()
        }
        async fn wait(&self, job: &str) -> JobRecord {
            for _ in 0..1500 {
                let (jobs, _) = self.stores.jobs.all_report().unwrap();
                let done = jobs
                    .iter()
                    .find(|j| j.id == job)
                    .is_some_and(|j| j.status != JobStatus::Running);
                if done && self.svc.inner.running.lock().is_empty() {
                    return jobs.into_iter().find(|j| j.id == job).unwrap();
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
            panic!("job {job} did not finish");
        }
        /// Upload, then open every chapter (one import of all that are pending) and wait.
        async fn imported(&self) -> (String, JobRecord) {
            let up = self.upload().await;
            let job = self.import_all(&up.book).await;
            (up.book, job)
        }
        async fn import_all(&self, book: &str) -> JobRecord {
            let job = self.svc.import(book, None, self.guard()).await.unwrap();
            self.wait(&job).await
        }
        /// Open these chapters and wait for the job.
        async fn open(&self, book: &str, chapters: Vec<u32>) -> JobRecord {
            let job = self
                .svc
                .import(book, Some(chapters), self.guard())
                .await
                .unwrap();
            self.wait(&job).await
        }
        fn advance(&self, ms: i64) {
            self.skew.fetch_add(ms, Ordering::SeqCst);
        }
        fn state(&self, book: &str) -> BookState {
            fold_book(&self.stores.textbooks.events(book).unwrap())
        }
        fn ready(&self, book: &str) -> usize {
            self.state(book)
                .chapters
                .iter()
                .filter(|v| v.state == ChapterState::Ready)
                .count()
        }
        fn card_count(&self) -> usize {
            self.stores.tracker.cards().unwrap().len()
        }
    }

    fn garbage() -> Result<String, DriverError> {
        Ok("not json".into())
    }

    // ---------------------------------------------------------------- import

    #[tokio::test]
    async fn upload_makes_zero_chapter_calls_and_starts_no_job() {
        let r = rig(book(3, 400));
        let up = r.upload().await;
        assert!(!up.existing);
        let st = r.state(&up.book);
        assert_eq!(st.source, Some(MapSource::Outline));
        assert_eq!(st.chapters.len(), 3);
        assert!(st.chapters.iter().all(|v| v.state == ChapterState::Pending));
        assert_eq!(r.engine.n(), 0, "an outline needs no model call at all");
        assert_eq!((st.auto_calls, st.requested_calls), (0, 0));
        assert!(r.stores.jobs.all_report().unwrap().0.is_empty(), "no job");
        assert!(r.notes.lock().is_empty());
        assert_eq!(r.card_count(), 0);
        assert!(r.svc.inner.running.lock().is_empty());
        assert_eq!(
            r.stores.textbooks.events(&up.book).unwrap().len(),
            2,
            "the ledger holds only Imported and Mapped"
        );
        let v = r.svc.book(&up.book).await.unwrap();
        assert!(v.job.is_none());
        assert!(v
            .chapters
            .iter()
            .all(|c| c.state == ChapterRowState::Pending && c.can_import));
        assert_eq!(v.meta.title, "My Book");
        let u = r.svc.usage(&up.book).await.unwrap();
        assert_eq!((u.hour_used, u.hour_cap, u.next_slot_at), (0, 8, None));
    }

    #[tokio::test]
    async fn upload_never_imports_a_60_chapter_book() {
        let pdf = book(60, 400);
        let r = rig(pdf);
        let up = r.upload().await;
        assert_eq!(r.engine.n(), 0);
        assert_eq!(r.state(&up.book).chapters.len(), 60);
        assert_eq!(r.ready(&up.book), 0);
        let list = r.svc.list().await.unwrap();
        assert_eq!(
            list[0].state,
            BookPhase::Partial,
            "mapped, nothing imported"
        );
        assert_eq!(list[0].ready, 0);
    }

    #[tokio::test]
    async fn opening_one_chapter_imports_only_that_chapter() {
        let r = rig(book(3, 400));
        let up = r.upload().await;
        let job = r.open(&up.book, vec![1]).await;
        assert_eq!(r.engine.n(), 1, "one call, for the chapter that was opened");
        let st = r.state(&up.book);
        assert_eq!(st.chapters[0].state, ChapterState::Pending);
        assert_eq!(st.chapters[1].state, ChapterState::Ready);
        assert_eq!(st.chapters[2].state, ChapterState::Pending);
        assert_eq!((st.auto_calls, st.requested_calls), (0, 1));
        assert_eq!((st.cheap_in, st.cheap_out), (100, 10));
        assert_eq!(job.status, JobStatus::Done);
        assert_eq!(job.chat_id.as_str(), "");
        assert_eq!(job.delivery, DeliveryStatus::Claimed);
        let result = job.result.unwrap();
        assert_eq!(result["summary"]["ready"], 1);
        assert_eq!(result["summary"]["pending"], 2);
        assert_eq!(result["bookId"], json!(up.book));
        assert!(result["error"].is_null());
        assert!(!r.notes.lock().is_empty());
        assert!(r.card_count() > 0);
        let v = r.svc.book(&up.book).await.unwrap();
        assert_eq!(v.chapters[1].state, ChapterRowState::Ready);
        assert!(!v.chapters[1].can_import);
        assert!(v.chapters[2].can_import);
        assert_eq!(v.hour_used, 1);
    }

    #[tokio::test]
    async fn import_reads_only_the_chapter_in_10_page_windows() {
        // chapter 1 is 25 pages, chapter 2 is 5; nothing is read at upload
        let mut pdf = book(30, 20);
        pdf.outline = vec![("One", 1), ("Two", 26)]
            .into_iter()
            .map(|(t, p)| OutlineEntry {
                title: t.into(),
                page: p,
                depth: 0,
            })
            .collect();
        let pdf = Arc::new(pdf);
        let r = rig_with(pdf.clone(), Box::new(|_, p| Ok(good(p))));
        let up = r.upload().await;
        assert!(pdf.reads.lock().is_empty(), "upload reads no page text");
        r.open(&up.book, vec![0]).await;
        assert_eq!(
            *pdf.reads.lock(),
            vec![(1, 10), (11, 20), (21, 25)],
            "3 extractor spawns for 25 pages, never one per page; chapter 2 is not read"
        );
        assert_eq!(r.ready(&up.book), 1);
    }

    #[tokio::test]
    async fn no_outline_makes_exactly_one_toc_call_at_upload() {
        let mut pdf = book(3, 400);
        pdf.outline.clear();
        for (i, p) in pdf.pages.iter_mut().enumerate() {
            *p = format!("Chapter {} heading\n{}", i + 1, words(400));
        }
        let reply: Reply = Box::new(|_, p| {
            if p.contains("printed_page") {
                Ok(json!({"chapters":[
                    {"title":"Chapter 1 heading","printed_page":1},
                    {"title":"Chapter 2 heading","printed_page":2},
                    {"title":"Chapter 3 heading","printed_page":3}]})
                .to_string())
            } else {
                Ok(good(p))
            }
        });
        let r = rig_with(Arc::new(pdf), reply);
        let up = r.upload().await;
        let st = r.state(&up.book);
        assert_eq!(st.source, Some(MapSource::Toc));
        assert_eq!(r.engine.toc_calls(), 1);
        assert_eq!(r.engine.n(), 1, "the TOC call, and no chapter call");
        assert_eq!((st.auto_calls, st.requested_calls), (1, 0));
        assert_eq!(st.chapters.len(), 3);
        assert_eq!(r.ready(&up.book), 0);
        let usage = r.svc.usage(&up.book).await.unwrap();
        assert_eq!(usage.hour_used, 0, "the TOC call is not a chapter import");
        r.import_all(&up.book).await;
        assert_eq!(r.engine.n(), 4);
        assert_eq!(r.ready(&up.book), 3);
    }

    #[tokio::test]
    async fn one_call_per_chapter_and_split_parts_counted() {
        // chapter 2 is two pages of 4500 words: 9000 words, two parts
        let mut pdf = book(4, 400);
        pdf.pages[1] = words(4500);
        pdf.pages[2] = words(4500);
        pdf.outline = vec![("One", 1), ("Two", 2), ("Three", 4)]
            .into_iter()
            .map(|(t, p)| OutlineEntry {
                title: t.into(),
                page: p,
                depth: 0,
            })
            .collect();
        let r = rig(pdf);
        let (book, _) = r.imported().await;
        let st = r.state(&book);
        assert_eq!(st.chapters.len(), 3);
        assert_eq!(r.engine.n(), 4, "3 chapters plus 1 extra part");
        assert_eq!(st.requested_calls, 4);
        let two = &st.chapters[1];
        assert_eq!(two.state, ChapterState::Ready);
        // objectives from two parts are numbered across the parts
        let ids: Vec<&str> = two.objectives.iter().map(|o| o.id.as_str()).collect();
        assert_eq!(ids.first(), Some(&"ch001-o01"));
        assert_eq!(ids.last(), Some(&"ch001-o12"));
        // two parts are still one chapter import
        assert_eq!(r.svc.usage(&book).await.unwrap().hour_used, 3);
    }

    #[tokio::test]
    async fn hour_limit_is_8_chapter_imports_per_rolling_hour() {
        let r = rig(book(20, 400));
        let up = r.upload().await;
        let t0 = r.svc.inner.clock.as_ref()();
        let job = r.import_all(&up.book).await;
        assert_eq!(r.engine.n(), 8, "8 chapters, then the limit");
        assert_eq!(r.ready(&up.book), 8);
        assert_eq!(job.result.unwrap()["stopped"], "rate_limited");
        let u = r.svc.usage(&up.book).await.unwrap();
        assert_eq!((u.hour_used, u.hour_cap), (8, 8));
        assert_eq!(u.next_slot_at, Some(t0 + IMPORT_WINDOW_MS));
        // 59 minutes later nothing has changed
        r.advance(59 * 60_000);
        let e = r.svc.import(&up.book, Some(vec![8]), r.guard()).await;
        assert_eq!(
            e.unwrap_err(),
            TextbookError::Conflict(conflict::RATE_LIMITED.into())
        );
        let e = r.svc.import(&up.book, None, r.guard()).await;
        assert_eq!(
            e.unwrap_err(),
            TextbookError::Conflict(conflict::RATE_LIMITED.into())
        );
        assert_eq!(r.engine.n(), 8, "a refused import makes no call");
        // past the hour every slot is free again
        r.advance(2 * 60_000);
        assert_eq!(r.svc.usage(&up.book).await.unwrap().hour_used, 0);
        r.open(&up.book, vec![8]).await;
        assert_eq!(r.ready(&up.book), 9);
        assert_eq!(r.engine.n(), 9);
        assert_eq!(r.svc.usage(&up.book).await.unwrap().hour_used, 1);
    }

    #[tokio::test]
    async fn the_hour_limit_is_checked_between_chapters_of_one_job() {
        // 7 imports used, then a job over 3 chapters gets only one more
        let r = rig(book(12, 400));
        let up = r.upload().await;
        r.open(&up.book, (0..7).collect()).await;
        assert_eq!(r.engine.n(), 7);
        let job = r.open(&up.book, vec![7, 8, 9]).await;
        assert_eq!(r.engine.n(), 8);
        assert_eq!(job.result.unwrap()["stopped"], "rate_limited");
        assert_eq!(r.state(&up.book).chapters[8].state, ChapterState::Pending);
    }

    #[tokio::test]
    async fn retry_is_part_of_the_one_import_and_does_not_use_a_slot() {
        // call 1 is garbage, the retry (call 2) works: one chapter import, two calls
        let reply: Reply = Box::new(|n, p| if n == 1 { garbage() } else { Ok(good(p)) });
        let r = rig_with(Arc::new(book(3, 400)), reply);
        let up = r.upload().await;
        r.open(&up.book, vec![0]).await;
        assert_eq!(r.engine.n(), 2);
        assert_eq!(r.ready(&up.book), 1);
        assert_eq!(r.svc.usage(&up.book).await.unwrap().hour_used, 1);
    }

    #[tokio::test]
    async fn chapter_gets_one_try_plus_one_retry_then_it_is_final() {
        let reply: Reply = Box::new(|n, p| if n <= 2 { garbage() } else { Ok(good(p)) });
        let r = rig_with(Arc::new(book(3, 400)), reply);
        let up = r.upload().await;
        r.open(&up.book, vec![0]).await;
        assert_eq!(r.engine.n(), 2, "the try and one retry, no third call");
        let st = r.state(&up.book);
        assert_eq!(st.chapters[0].state, ChapterState::Failed);
        assert_eq!(st.chapters[0].error.as_deref(), Some("parse"));
        assert!(!st.chapters[0].retryable);
        assert_eq!(pending_chapters(&st), vec![1, 2], "it is not offered again");
        let v = r.svc.book(&up.book).await.unwrap();
        assert!(!v.chapters[0].can_import);
        assert!(v.chapters[1].can_import);
        // asking for it by number is refused and makes no call
        let e = r.svc.import(&up.book, Some(vec![0]), r.guard()).await;
        assert_eq!(
            e.unwrap_err(),
            TextbookError::Conflict(conflict::RETRY_USED.into())
        );
        assert_eq!(r.engine.n(), 2);
        // the other chapters are unaffected
        r.open(&up.book, vec![1]).await;
        assert_eq!(r.ready(&up.book), 1);
    }

    #[tokio::test]
    async fn two_engine_errors_stop_job_early_and_stay_retryable() {
        let reply: Reply = Box::new(|_, _| Err(DriverError::Failed("rate limit".into())));
        let r = rig_with(Arc::new(book(5, 400)), reply);
        let (book, job) = r.imported().await;
        assert_eq!(r.engine.n(), 2);
        let st = r.state(&book);
        assert_eq!(st.chapters[0].error.as_deref(), Some("engine_unavailable"));
        assert_eq!(st.chapters[1].error.as_deref(), Some("engine_unavailable"));
        assert!(st.chapters[0].retryable, "no content attempt was spent");
        assert_eq!(st.chapters[2].state, ChapterState::Pending);
        assert_eq!(job.result.unwrap()["stopped"], "engine_unavailable");
    }

    #[tokio::test]
    async fn same_pdf_twice_no_duplicate_objectives_or_cards() {
        let r = rig(book(3, 400));
        let (book, _) = r.imported().await;
        let (cards, calls) = (r.card_count(), r.engine.n());
        assert!(cards > 0);
        let again = r.upload().await;
        assert!(again.existing);
        assert_eq!(again.book, book);
        assert_eq!(r.card_count(), cards);
        assert_eq!(r.engine.n(), calls);
        let imported = r
            .stores
            .textbooks
            .events(&book)
            .unwrap()
            .iter()
            .filter(|e| matches!(e, TextbookEvent::Imported { .. }))
            .count();
        assert_eq!(imported, 1);
        assert_eq!(r.svc.list().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn concurrent_identical_uploads_one_book() {
        let r = rig(book(3, 400));
        let bytes = r.bytes();
        let (a, b) = tokio::join!(
            r.svc.upload("a.pdf", bytes.clone(), r.guard()),
            r.svc.upload("a.pdf", bytes.clone(), r.guard())
        );
        let (a, b) = (a.unwrap(), b.unwrap());
        assert_eq!(a.book, b.book);
        assert_ne!(
            a.existing, b.existing,
            "exactly one of them stored the book"
        );
        assert_eq!(r.stores.textbooks.list().unwrap().len(), 1);
        assert_eq!(r.engine.n(), 0, "neither upload called a model");
        assert_eq!(r.state(&a.book).chapters.len(), 3);
    }

    /// A PDF whose outline read is slow, so a second upload can arrive while the first maps.
    struct SlowOutline(FakePdf);

    impl Pdf for SlowOutline {
        fn pages(&self, p: &str) -> Result<Vec<String>, String> {
            self.0.pages(p)
        }
        fn page_count(&self, p: &str) -> Result<usize, String> {
            self.0.page_count(p)
        }
        fn page_texts(&self, p: &str, first: usize, last: usize) -> Result<Vec<String>, String> {
            self.0.page_texts(p, first, last)
        }
        fn outline(&self, p: &str) -> Result<Vec<OutlineEntry>, String> {
            std::thread::sleep(Duration::from_millis(150));
            self.0.outline(p)
        }
    }

    #[tokio::test]
    async fn concurrent_uploads_of_an_unmappable_book_both_fail_cleanly() {
        // No outline and no text: mapping fails after the book is already on disk.
        let pdf = FakePdf {
            pages: vec![String::new(); 4],
            outline: vec![],
            fail_from: None,
            reads: Mutex::default(),
        };
        let r = rig_with(Arc::new(SlowOutline(pdf)), Box::new(|_, p| Ok(good(p))));
        let bytes = r.bytes();
        let (a, b) = tokio::join!(
            r.svc.upload("s.pdf", bytes.clone(), r.guard()),
            r.svc.upload("s.pdf", bytes.clone(), r.guard())
        );
        // The second upload must not report a half-made book as existing: it waits for the
        // first, then fails the same way.
        assert_eq!(a.unwrap_err(), TextbookError::Invalid(NO_TEXT.into()));
        assert_eq!(b.unwrap_err(), TextbookError::Invalid(NO_TEXT.into()));
        assert!(r.svc.list().await.unwrap().is_empty(), "nothing is kept");
        assert_eq!(r.engine.n(), 0);
    }

    #[tokio::test]
    async fn a_panicking_import_does_not_block_the_next_one() {
        let boom = Arc::new(AtomicBool::new(true));
        let b2 = boom.clone();
        let r = rig_with(
            Arc::new(book(2, 400)),
            Box::new(move |_, p| {
                if b2.swap(false, Ordering::SeqCst) {
                    panic!("engine blew up");
                }
                Ok(good(p))
            }),
        );
        let up = r.upload().await;
        r.svc.import(&up.book, None, r.guard()).await.unwrap();
        for _ in 0..500 {
            if r.engine.n() >= 1 && r.svc.inner.running.lock().is_empty() {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert!(
            r.svc.inner.running.lock().is_empty(),
            "a panicked job leaves no running entry"
        );
        let job = r.svc.import(&up.book, None, r.guard()).await.unwrap();
        r.wait(&job).await;
        assert_eq!(r.ready(&up.book), 2);
    }

    #[tokio::test]
    async fn cancel_marks_failed_and_keeps_ready_chapters() {
        let r = rig(book(4, 400));
        r.engine.hold_from.store(3, Ordering::SeqCst);
        let up = r.upload().await;
        let job = r.svc.import(&up.book, None, r.guard()).await.unwrap();
        for _ in 0..500 {
            if r.engine.n() >= 3 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        assert_eq!(r.engine.n(), 3);
        assert!(r.svc.abort(&job));
        assert!(!r.svc.abort("tbj_other"));
        let rec = r.wait(&job).await;
        assert_eq!(rec.status, JobStatus::Failed);
        assert_eq!(rec.result.unwrap()["error"], "cancelled");
        assert_eq!(r.ready(&up.book), 2, "finished chapters stay");
        assert_eq!(r.engine.n(), 3, "no call after the abort");
    }

    #[tokio::test]
    async fn page_texts_failure_mid_import_ends_job_with_error() {
        let mut pdf = book(25, 400);
        pdf.fail_from = Some(15);
        let r = rig(pdf);
        let up = r.upload().await;
        let job = r.open(&up.book, vec![14]).await;
        assert_eq!(job.status, JobStatus::Done);
        let result = job.result.unwrap();
        assert!(result["error"].as_str().unwrap().contains("timed out"));
        assert_eq!(r.engine.n(), 0);
        assert_eq!(r.ready(&up.book), 0);
        assert_eq!(r.state(&up.book).chapters[14].state, ChapterState::Pending);
    }

    #[tokio::test]
    async fn toc_under_two_chapters_falls_back_to_chunks() {
        let mut pdf = book(45, 100);
        pdf.outline.clear();
        let reply: Reply = Box::new(|_, p| {
            if p.contains("printed_page") {
                Ok(r#"{"chapters":[{"title":"Only","printed_page":1}]}"#.into())
            } else {
                Ok(good(p))
            }
        });
        let r = rig_with(Arc::new(pdf), reply);
        let up = r.upload().await;
        let st = r.state(&up.book);
        assert_eq!(st.source, Some(MapSource::Chunks));
        assert_eq!(st.chapters.len(), 3);
        assert_eq!(r.engine.toc_calls(), 1);
        assert_eq!(st.auto_calls, 1, "the TOC call stays counted");
        assert_eq!(r.engine.n(), 1);
    }

    #[tokio::test]
    async fn scanned_book_without_outline_is_refused_at_upload_with_no_calls() {
        let pdf = FakePdf {
            pages: vec![String::new(); 4],
            outline: vec![],
            fail_from: None,
            reads: Mutex::default(),
        };
        let r = rig(pdf);
        let e = r
            .svc
            .upload("s.pdf", r.bytes(), r.guard())
            .await
            .unwrap_err();
        assert_eq!(e, TextbookError::Invalid(NO_TEXT.into()));
        assert_eq!(r.engine.n(), 0);
        assert!(r.svc.list().await.unwrap().is_empty(), "nothing is kept");
    }

    #[tokio::test]
    async fn scanned_book_with_outline_ends_chapter_import_with_ocr_error_no_calls() {
        let mut pdf = book(3, 400);
        pdf.pages = vec![String::new(); 3];
        let r = rig(pdf);
        let up = r.upload().await;
        let job = r.open(&up.book, vec![0]).await;
        assert_eq!(r.engine.n(), 0);
        assert_eq!(job.result.unwrap()["error"], NO_TEXT);
        assert_eq!(r.ready(&up.book), 0);
        assert_eq!(r.state(&up.book).chapters[0].state, ChapterState::Skipped);
        let v = r.svc.book(&up.book).await.unwrap();
        assert!(
            !v.chapters[0].can_import,
            "an empty chapter stops offering Import"
        );
    }

    /// A job over `only`, built directly so its chapter list is not cleaned by `import`.
    async fn raw_job(r: &Rig, book: &str, only: Vec<u32>, calls: u32) -> JobRecord {
        let st = r.state(book);
        let rec = JobRecord {
            id: "tbj_raw_0".into(),
            chat_id: "".into(),
            kind: JobKind::Textbook,
            status: JobStatus::Running,
            delivery: DeliveryStatus::Claimed,
            created_at: 1,
            result: None,
            usage: vec![],
        };
        r.stores.jobs.create_job(&rec).unwrap();
        let job = Job {
            me: r.svc.inner.clone(),
            book: book.to_string(),
            rec,
            meta: st.meta.unwrap(),
            progress: Progress::default(),
            cancel: CancellationToken::new(),
            only: Some(only),
            calls: AtomicU32::new(calls),
        };
        job.run().await;
        let (jobs, _) = r.stores.jobs.all_report().unwrap();
        jobs.into_iter().find(|j| j.id == "tbj_raw_0").unwrap()
    }

    #[tokio::test]
    async fn duplicate_chapter_indices_call_the_model_once() {
        let r = rig(book(4, 400));
        let up = r.upload().await;
        r.open(&up.book, vec![2, 2, 2]).await;
        assert_eq!(r.engine.n(), 1, "chapter 2 was imported once");
        assert_eq!(r.ready(&up.book), 1);
        assert_eq!(r.svc.usage(&up.book).await.unwrap().hour_used, 1);
        // a failing chapter listed three times: one try plus one retry, not three
        let r = rig_with(Arc::new(book(4, 400)), Box::new(|_, _| garbage()));
        let up = r.upload().await;
        r.open(&up.book, vec![2, 2, 2]).await;
        assert_eq!(r.engine.n(), 2);
        assert!(!r.state(&up.book).chapters[2].retryable);
        // the job loop itself repeats nothing even when handed duplicates
        let r = rig_with(Arc::new(book(4, 400)), Box::new(|_, _| garbage()));
        let up = r.upload().await;
        raw_job(&r, &up.book, vec![2, 2, 2], 0).await;
        assert_eq!(r.engine.n(), 2, "the repeats see a spent chapter and pass");
    }

    #[tokio::test]
    async fn skipped_chapters_are_rejected_and_never_called() {
        let r = rig(book(3, 400));
        let up = r.upload().await;
        r.stores
            .textbooks
            .append_event(
                &up.book,
                &TextbookEvent::ChapterSkipped {
                    ts: 1,
                    chapter: 1,
                    reason: "too_short".into(),
                },
            )
            .unwrap();
        let e = r.svc.import(&up.book, Some(vec![1]), r.guard()).await;
        assert!(matches!(e, Err(TextbookError::Invalid(_))), "{e:?}");
        raw_job(&r, &up.book, vec![1, 1], 0).await;
        assert_eq!(r.engine.n(), 0, "the job loop passes a skipped chapter");
        // and a book-wide import does not pick it up
        let job = r.import_all(&up.book).await;
        assert_eq!(r.engine.n(), 2, "chapters 0 and 2 only");
        assert!(job.result.unwrap()["error"].is_null());
    }

    #[tokio::test]
    async fn one_job_cannot_exceed_its_call_cap() {
        let r = rig(book(3, 400));
        let up = r.upload().await;
        let job = raw_job(&r, &up.book, vec![0], MAX_JOB_CALLS - 1).await;
        assert_eq!(
            r.engine.n(),
            0,
            "a chapter that might not fit is not started"
        );
        assert_eq!(job.result.unwrap()["stopped"], "call_cap");
        assert_eq!(r.ready(&up.book), 0);
    }

    #[tokio::test]
    async fn unmapped_book_is_mapped_by_its_first_import() {
        // a crash after `Imported` and before `Mapped` leaves a stored book with no chapters
        let r = rig(book(3, 400));
        let sha = hex::encode(Sha256::digest(b"unmapped"));
        let book = book_id(&sha);
        r.stores.textbooks.save_source(&book, b"%PDF-x").unwrap();
        r.stores
            .textbooks
            .append_event(
                &book,
                &TextbookEvent::Imported {
                    ts: 1,
                    book: BookMeta {
                        id: book.clone(),
                        title: "Half".into(),
                        file_name: "h.pdf".into(),
                        bytes: 6,
                        sha256: sha,
                        total_pages: 3,
                        page_offset: None,
                    },
                },
            )
            .unwrap();
        assert!(r.state(&book).source.is_none());
        r.open(&book, vec![1]).await;
        let st = r.state(&book);
        assert_eq!(st.source, Some(MapSource::Outline));
        assert_eq!(st.chapters[1].state, ChapterState::Ready);
        assert_eq!(r.engine.n(), 1);
    }

    // ---------------------------------------------------------------- recovery

    /// A mapped book with chapter 0 `Ready` in the ledger only: no cards, no job.
    fn seed_ready_book(r: &Rig, tag: &str) -> String {
        let sha = hex::encode(Sha256::digest(tag.as_bytes()));
        let book = book_id(&sha);
        let range = PageRange { start: 1, end: 3 };
        let raws: Vec<RawObjective> = (1..=4)
            .map(|i| RawObjective {
                text: format!("Objective {i}"),
                start_page: 1,
                end_page: 2,
                items: vec![
                    lp_core::course::Item {
                        kind: ItemKind::Mcq,
                        q: format!("Which one is item {i} here?"),
                        options: (0..4).map(|k| format!("opt {i} {k}")).collect(),
                        answer: format!("opt {i} 0"),
                        why: "w".into(),
                        ..Default::default()
                    },
                    lp_core::course::Item {
                        kind: ItemKind::Recall,
                        q: format!("Name {i}"),
                        answer: format!("answer {i}"),
                        why: "w".into(),
                        ..Default::default()
                    },
                ],
            })
            .collect();
        let objectives = accept_objectives(0, 1, range, raws);
        let ev = |e| r.stores.textbooks.append_event(&book, &e).unwrap();
        ev(TextbookEvent::Imported {
            ts: 1,
            book: BookMeta {
                id: book.clone(),
                title: "Seed".into(),
                file_name: "s.pdf".into(),
                bytes: 1,
                sha256: sha,
                total_pages: 3,
                page_offset: None,
            },
        });
        ev(TextbookEvent::Mapped {
            ts: 2,
            source: MapSource::Outline,
            chapters: vec![
                Chapter {
                    index: 0,
                    title: "One".into(),
                    pages: range,
                    kind: ChapterKind::Chapter,
                },
                Chapter {
                    index: 1,
                    title: "Two".into(),
                    pages: PageRange { start: 4, end: 5 },
                    kind: ChapterKind::Chapter,
                },
            ],
        });
        ev(TextbookEvent::ChapterDone {
            ts: 3,
            chapter: 0,
            parts: 1,
            truncated: false,
            objectives,
        });
        book
    }

    #[tokio::test]
    async fn crash_between_chapter_done_and_cards_is_reconciled() {
        let r = rig(book(1, 400));
        let a = seed_ready_book(&r, "a");
        let b = seed_ready_book(&r, "b");
        assert_eq!(
            r.card_count(),
            0,
            "the crash left a Ready chapter with no cards"
        );
        // start_check repairs the chapter it is asked about
        let check = r.svc.start_check(&a, 0).await.unwrap();
        assert!(!check.items.is_empty());
        let after_a = r.card_count();
        assert_eq!(after_a, 8, "4 objectives x (mcq + recall)");
        // recover repairs every Ready chapter
        r.svc.recover().await.unwrap();
        assert_eq!(r.card_count(), 16);
        assert!(r
            .stores
            .tracker
            .card(&card_id(&b, "ch000-o01-i1"))
            .unwrap()
            .is_some());
        assert_eq!(r.engine.n(), 0);
    }

    #[tokio::test]
    async fn restart_recovers_running_job_and_resume_imports_rest() {
        let fail = Arc::new(AtomicBool::new(true));
        let f2 = fail.clone();
        let reply: Reply = Box::new(move |n, p| {
            if n > 1 && f2.load(Ordering::SeqCst) {
                Err(DriverError::Failed("limited".into()))
            } else {
                Ok(good(p))
            }
        });
        let r = rig_with(Arc::new(book(4, 400)), reply);
        let (book, _) = r.imported().await;
        assert_eq!(r.ready(&book), 1);
        // a job a dead process left running
        let stale = JobRecord {
            id: "tbj_dead".into(),
            chat_id: "".into(),
            kind: JobKind::Textbook,
            status: JobStatus::Running,
            delivery: DeliveryStatus::Claimed,
            created_at: 1,
            result: Some(json!({"title":"Import x","bookId":book})),
            usage: vec![],
        };
        r.stores.jobs.create_job(&stale).unwrap();
        let before = r.stores.textbooks.events(&book).unwrap();
        assert_eq!(r.svc.recover().await.unwrap(), 1);
        let (jobs, _) = r.stores.jobs.all_report().unwrap();
        let rec = jobs.iter().find(|j| j.id == "tbj_dead").unwrap();
        assert_eq!(rec.status, JobStatus::Done);
        assert_eq!(
            rec.result.as_ref().unwrap()["error"],
            "interrupted by a restart"
        );
        assert_eq!(
            r.stores.textbooks.events(&book).unwrap(),
            before,
            "ledger untouched"
        );
        // resume
        fail.store(false, Ordering::SeqCst);
        let calls = r.engine.n();
        let job = r.svc.import(&book, None, r.guard()).await.unwrap();
        r.wait(&job).await;
        assert_eq!(r.ready(&book), 4);
        assert_eq!(r.engine.n() - calls, 3, "only the missing chapters");
        let st = r.state(&book);
        assert_eq!(st.requested_calls, 6, "3 in the first job, 3 on resume");
        assert_eq!(st.auto_calls, 0);
        assert!(job.starts_with("tbj_"));
        let e = r.svc.import(&book, None, r.guard()).await.unwrap_err();
        assert_eq!(
            e,
            TextbookError::Conflict(conflict::NOTHING_TO_IMPORT.into())
        );
    }

    #[tokio::test]
    async fn import_running_conflicts_and_bad_requests_are_rejected() {
        let r = rig(book(3, 400));
        r.engine.hold_from.store(2, Ordering::SeqCst);
        let up = r.upload().await;
        let job = r.svc.import(&up.book, None, r.guard()).await.unwrap();
        for _ in 0..500 {
            if r.engine.n() >= 2 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        let e = r.svc.import(&up.book, None, r.guard()).await.unwrap_err();
        assert_eq!(e, TextbookError::Conflict(conflict::IMPORT_RUNNING.into()));
        let again = r.upload().await;
        assert!(again.existing);
        r.svc.abort(&job);
        r.wait(&job).await;
        r.engine.hold_from.store(0, Ordering::SeqCst);
        for bad in [vec![9], vec![0], vec![]] {
            let e = r
                .svc
                .import(&up.book, Some(bad), r.guard())
                .await
                .unwrap_err();
            assert!(matches!(e, TextbookError::Invalid(_)), "{e:?}");
        }
        let e = r
            .svc
            .import("tb_0000000000000000", None, r.guard())
            .await
            .unwrap_err();
        assert!(matches!(e, TextbookError::NotFound(_)));
        assert!(matches!(
            r.svc.book("../x").await,
            Err(TextbookError::NotFound(_))
        ));
    }

    // ---------------------------------------------------------------- studying

    #[tokio::test]
    async fn reading_check_and_answers_make_zero_calls() {
        let r = rig(book(3, 400));
        let (book, _) = r.imported().await;
        let calls = r.engine.n();
        r.svc.list().await.unwrap();
        r.svc.book(&book).await.unwrap();
        r.svc.chapter(&book, 0).await.unwrap();
        r.svc.usage(&book).await.unwrap();
        let check = r.svc.start_check(&book, 0).await.unwrap();
        assert!(!check.items.is_empty() && check.items.len() <= CHECK_ITEMS);
        for it in &check.items {
            let fb = r
                .svc
                .answer(&book, &check.check, &it.id, "x", AnswerMeta::default())
                .await
                .unwrap();
            if it.kind == ItemKind::Recall && fb.close {
                r.svc
                    .self_grade(&book, &check.check, &it.id, "good")
                    .await
                    .unwrap();
            }
            assert!(!fb.answer.is_empty());
        }
        assert_eq!(r.engine.n(), calls, "studying never calls a model");
        let e = r.svc.start_check(&book, 99).await.unwrap_err();
        assert!(matches!(e, TextbookError::NotFound(_)));
    }

    fn stored_item(r: &Rig, book: &str, id: &str) -> lp_core::course::Item {
        r.state(book)
            .chapters
            .iter()
            .flat_map(|v| &v.objectives)
            .flat_map(|o| &o.items)
            .find(|i| i.id == id)
            .unwrap()
            .clone()
    }

    #[tokio::test]
    async fn answer_grades_by_code_and_updates_fsrs() {
        let r = rig(book(2, 400));
        let (book, _) = r.imported().await;
        // The pick between an objective's mcq and recall item is seeded by the check id: ask
        // again until a check carries an mcq.
        let mut check = r.svc.start_check(&book, 0).await.unwrap();
        for _ in 0..40 {
            if check.items.iter().any(|i| i.kind == ItemKind::Mcq) {
                break;
            }
            check = r.svc.start_check(&book, 0).await.unwrap();
        }
        let item = check
            .items
            .iter()
            .find(|i| i.kind == ItemKind::Mcq)
            .unwrap();
        // options are shuffled for display and never carry the answer key
        assert_eq!(item.options.len(), 4);
        let stored = stored_item(&r, &book, &item.id);
        let card = card_id(&book, &item.id);
        assert_eq!(
            r.stores.tracker.card(&card).unwrap().unwrap().card.due,
            crate::course::PARKED_DUE
        );
        let fb = r
            .svc
            .answer(
                &book,
                &check.check,
                &item.id,
                &stored.answer,
                AnswerMeta::default(),
            )
            .await
            .unwrap();
        assert!(fb.correct && !fb.close);
        assert_eq!(fb.answer, stored.answer);
        let due = r.stores.tracker.card(&card).unwrap().unwrap().card.due;
        assert!(due < crate::course::PARKED_DUE);
        assert_eq!(fb.due, Some(due));
        let history = review::history(&*r.stores.tracker, &card).unwrap();
        assert_eq!(history.len(), 1);
        let e = r
            .svc
            .answer(
                &book,
                &check.check,
                &item.id,
                &stored.answer,
                AnswerMeta::default(),
            )
            .await
            .unwrap_err();
        assert_eq!(
            e,
            TextbookError::Conflict(conflict::ALREADY_ANSWERED.into())
        );
        assert_eq!(review::history(&*r.stores.tracker, &card).unwrap().len(), 1);
        // the same client id replays the stored feedback and grades nothing again
        let item2 = check.items.iter().find(|i| i.id != item.id).unwrap();
        let s2 = stored_item(&r, &book, &item2.id);
        let meta = || AnswerMeta {
            client_id: Some("c1".into()),
            ..AnswerMeta::default()
        };
        let a = r
            .svc
            .answer(&book, &check.check, &item2.id, &s2.answer, meta())
            .await
            .unwrap();
        let b = r
            .svc
            .answer(&book, &check.check, &item2.id, &s2.answer, meta())
            .await
            .unwrap();
        assert_eq!(a.correct, b.correct);
        assert_eq!(
            review::history(&*r.stores.tracker, &card_id(&book, &item2.id))
                .unwrap()
                .len(),
            1
        );
        // unknown check, item and bad input
        for (c, i) in [("rc_x", item.id.as_str()), (check.check.as_str(), "nope")] {
            let e = r
                .svc
                .answer(&book, c, i, "x", AnswerMeta::default())
                .await
                .unwrap_err();
            assert!(matches!(e, TextbookError::NotFound(_)), "{c} {i}");
        }
        let bad = AnswerMeta {
            confidence: Some(9),
            ..AnswerMeta::default()
        };
        assert!(matches!(
            r.svc.answer(&book, &check.check, &item.id, "x", bad).await,
            Err(TextbookError::Invalid(_))
        ));
    }

    #[tokio::test]
    async fn wrong_recall_is_rated_again_and_comes_back() {
        let r = rig(book(2, 400));
        let (book, _) = r.imported().await;
        for gap in [false, true] {
            let check = r.svc.start_check(&book, 0).await.unwrap();
            let Some(item) = check.items.iter().find(|i| i.kind == ItemKind::Recall) else {
                continue;
            };
            let card = card_id(&book, &item.id);
            // a plain miss, or "I don't know" (the UI sends an empty answer)
            let text = if gap { "" } else { "zzz nonsense" };
            let fb = r
                .svc
                .answer(&book, &check.check, &item.id, text, AnswerMeta::default())
                .await
                .unwrap();
            assert!(!fb.correct && !fb.close);
            let hist = review::history(&*r.stores.tracker, &card).unwrap();
            assert_eq!(hist.len(), 1, "a wrong answer is rated at once");
            assert_ne!(
                r.stores.tracker.card(&card).unwrap().unwrap().card.due,
                crate::course::PARKED_DUE,
                "the card is scheduled, so it returns"
            );
            assert!(fb.due.is_some_and(|d| d != crate::course::PARKED_DUE));
            // nothing left to grade by hand
            assert!(matches!(
                r.svc
                    .self_grade(&book, &check.check, &item.id, "good")
                    .await,
                Err(TextbookError::Conflict(_))
            ));
            return;
        }
        panic!("no recall item in any check");
    }

    #[tokio::test]
    async fn close_recall_needs_self_grade() {
        let r = rig(book(2, 400));
        let (book, _) = r.imported().await;
        let mut seen_recall = false;
        for _ in 0..8 {
            let check = r.svc.start_check(&book, 0).await.unwrap();
            let Some(item) = check.items.iter().find(|i| i.kind == ItemKind::Recall) else {
                continue;
            };
            seen_recall = true;
            let card = card_id(&book, &item.id);
            // One typo away from the stored answer: close, not exact.
            let stored = r.state(&book).chapters[0]
                .objectives
                .iter()
                .flat_map(|o| &o.items)
                .find(|i| i.id == item.id)
                .unwrap()
                .answer
                .clone();
            let typo = stored.replacen('m', "n", 1);
            let fb = r
                .svc
                .answer(&book, &check.check, &item.id, &typo, AnswerMeta::default())
                .await
                .unwrap();
            assert!(
                !fb.correct && fb.close,
                "a typo is close: {typo:?} vs {stored:?}"
            );
            assert!(
                review::history(&*r.stores.tracker, &card)
                    .unwrap()
                    .is_empty(),
                "no review before the learner grades"
            );
            assert_eq!(
                r.stores.tracker.card(&card).unwrap().unwrap().card.due,
                crate::course::PARKED_DUE
            );
            assert!(matches!(
                r.svc.self_grade(&book, &check.check, &item.id, "meh").await,
                Err(TextbookError::Invalid(_))
            ));
            r.svc
                .self_grade(&book, &check.check, &item.id, "hard")
                .await
                .unwrap();
            assert_eq!(review::history(&*r.stores.tracker, &card).unwrap().len(), 1);
            let again = r
                .svc
                .self_grade(&book, &check.check, &item.id, "good")
                .await
                .unwrap_err();
            assert!(matches!(again, TextbookError::Conflict(_)));
            // an unanswered item cannot be graded
            let other = check.items.iter().find(|i| i.id != item.id).unwrap();
            assert!(matches!(
                r.svc
                    .self_grade(&book, &check.check, &other.id, "good")
                    .await,
                Err(TextbookError::NotFound(_))
            ));
            break;
        }
        assert!(seen_recall);
    }

    #[tokio::test]
    async fn check_status_goes_not_read_checked_due_and_unready_chapter_is_409() {
        let r = rig(book(3, 400));
        let (book, _) = r.imported().await;
        let v = r.svc.book(&book).await.unwrap();
        assert!(v
            .chapters
            .iter()
            .all(|c| c.status == ChapterStatus::NotRead));
        let check = r.svc.start_check(&book, 1).await.unwrap();
        let item = &check.items[0];
        let stored = stored_item(&r, &book, &item.id);
        let answer = if item.kind == ItemKind::Mcq {
            stored.answer.clone()
        } else {
            "x".into()
        };
        r.svc
            .answer(
                &book,
                &check.check,
                &item.id,
                &answer,
                AnswerMeta::default(),
            )
            .await
            .unwrap();
        let v = r.svc.book(&book).await.unwrap();
        assert_eq!(v.chapters[1].status, ChapterStatus::Checked);
        assert_eq!(v.chapters[0].status, ChapterStatus::NotRead);
        // an unready chapter
        let seed = seed_ready_book(&r, "unready");
        let e = r.svc.start_check(&seed, 1).await.unwrap_err();
        assert_eq!(
            e,
            TextbookError::Conflict(conflict::CHAPTER_NOT_READY.into())
        );
        let detail = r.svc.chapter(&seed, 1).await.unwrap();
        assert_eq!(detail.row.state, ChapterRowState::Pending);
        assert!(detail.row.can_import);
        assert!(detail.objectives.is_empty());
    }

    #[tokio::test]
    async fn card_for_keeps_the_course_card_shape() {
        let item = lp_core::course::Item {
            id: "i1".into(),
            kind: ItemKind::Mcq,
            q: "Which?".into(),
            options: vec!["a".into(), "b".into(), "c".into(), "d".into()],
            answer: "c".into(),
            ..Default::default()
        };
        let c = card_for("c:x:i1", &item, &[]).unwrap();
        assert_eq!(c.id.as_str(), "c:x:i1");
        assert_eq!(c.options[c.correct_index as usize], "c");
        assert_eq!(c.card.due, crate::course::PARKED_DUE);
        assert_eq!(
            c,
            card_for("c:x:i1", &item, &["zz"]).unwrap(),
            "stable for the same id"
        );
        let recall = lp_core::course::Item {
            kind: ItemKind::Recall,
            answer: "x".into(),
            q: "q".into(),
            ..Default::default()
        };
        // A recall item is a typed card even with too few distractors, and carries its rubric.
        let rubric = lp_core::course::Item {
            must_name: vec!["alpha".into()],
            ..recall.clone()
        };
        let c = card_for("c:x:r", &rubric, &[]).unwrap();
        assert_eq!(c.kind, lp_core::ports::CardKind::Recall);
        assert_eq!(c.must_name, vec!["alpha".to_string()]);
        assert_eq!(
            card_for("c:x:i1", &item, &[]).unwrap().kind,
            lp_core::ports::CardKind::Mcq
        );
    }

    #[test]
    fn recall_is_graded_by_the_must_name_rubric() {
        let it = lp_core::course::Item {
            kind: ItemKind::Recall,
            answer: "the powerhouse of the cell".into(),
            must_name: vec!["mitochondria".into()],
            ..Default::default()
        };
        assert_eq!(grade_answer(&it, "It is the Mitochondria!"), (true, false));
        assert_eq!(grade_answer(&it, "the nucleus"), (false, false));
        // No rubric: only the exact answer passes; a rubric miss never turns right.
        let bare = lp_core::course::Item {
            must_name: vec![],
            ..it.clone()
        };
        assert_eq!(grade_answer(&bare, "mitochondria"), (false, false));
        assert_eq!(
            grade_answer(&bare, "the powerhouse of the cell"),
            (true, false)
        );
    }

    #[tokio::test]
    async fn fixture_pdf_imports_with_real_poppler() {
        use lp_agent::workbench::PdftotextPdf;
        if std::process::Command::new("pdftotext")
            .arg("-v")
            .output()
            .is_err()
        {
            eprintln!("skipped: Poppler is missing");
            return;
        }
        let path = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/textbook.pdf");
        let bytes = std::fs::read(path).unwrap();
        assert!(bytes.len() < 64 * 1024);
        // the extractor itself: reading order, an image-only page, roman front matter
        let all: Vec<String> = (1..=9)
            .map(|p| PdftotextPdf.page_texts(path, p, p).unwrap().remove(0))
            .collect();
        let two = &all[3];
        let (l, r) = (
            two.find("left column line one").unwrap(),
            two.find("right column line one").unwrap(),
        );
        assert!(l < r, "two-column text is in reading order");
        assert!(all[4].trim().is_empty(), "the image-only page has no text");
        let clean: Vec<String> = all.iter().map(|t| sanitize_page(t)).collect();
        assert_eq!(
            printed_offset(&clean),
            Some(2),
            "roman front matter is passed over"
        );

        let r = rig_with(Arc::new(PdftotextPdf), Box::new(|_, p| Ok(good(p))));
        let up = r
            .svc
            .upload("Streams Book.pdf", Bytes::from(bytes), r.guard())
            .await
            .unwrap();
        assert_eq!(r.engine.n(), 0, "upload called no model");
        let job = r.import_all(&up.book).await;
        assert!(job.result.unwrap()["error"].is_null());
        let st = r.state(&up.book);
        assert_eq!(st.meta.as_ref().unwrap().total_pages, 9);
        assert_eq!(st.source, Some(MapSource::Outline));
        let got: Vec<(String, u32, u32)> = st
            .chapters
            .iter()
            .map(|c| {
                (
                    c.chapter.title.clone(),
                    c.chapter.pages.start,
                    c.chapter.pages.end,
                )
            })
            .collect();
        assert_eq!(
            got,
            vec![
                ("Chapter 1 Streams".to_string(), 3, 5),
                ("Chapter 2 Pipes".to_string(), 6, 7),
                ("Chapter 3 Sockets".to_string(), 8, 9),
            ]
        );
        assert_eq!(
            r.ready(&up.book),
            3,
            "the image page did not fail chapter 1"
        );
        assert_eq!(r.engine.n(), 3);
        // the injection sentence reaches the engine only inside the fence
        let needle = "Ignore previous instructions";
        let prompts = r.engine.prompts.lock().clone();
        let hits: Vec<&String> = prompts.iter().filter(|p| p.contains(needle)).collect();
        assert_eq!(hits.len(), 1);
        let p = hits[0];
        let (open, close) = (p.find("<<data-").unwrap(), p.rfind("<<data-").unwrap());
        let at = p.find(needle).unwrap();
        assert!(open < at && at < close);
        assert!(p[..open].contains("not instructions"));
    }
}
