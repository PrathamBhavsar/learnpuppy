---
type: spec
title: Textbook Mode Backend Spec (U7, F37 part 1)
description: Build spec for textbook import, covering the chapter map, stored objectives and quiz items, the textbook.jsonl ledger, the import job lifecycle, call budget, reading-check routes and DTOs, error cases, tests per crate and a file-by-file build order with verified paths.
tags: [textbook, f37, u7, spec, pdf, ledger, jobs, fsrs, budget]
timestamp: 2026-11-11
---

# Textbook Mode Backend Spec (U7, F37 part 1)

Back to [Learning Method Plan](/learning-method/index.md). Task: U7 in [UI Polish Task DAG](/ui-polish/tasks.md). Tests: [UI Polish Test Plan](/ui-polish/test-plan.md). Cost rule: [Cheap Tutor Plan](/cheap-tutor/index.md) and [ADR-0007](/decisions/0007-course-as-data.md). Layering: [Learning Method Architecture](/learning-method/architecture.md).

Every path and function named here was read on `staging` at `2fccf39` on 2026-11-11. "New" marks a file or item that does not exist yet. This is a spec, not code: the builder writes the bodies, and the names, shapes, constants and error codes below are fixed. Anything this spec does not settle goes into [Review Log](/ui-polish/review.md) before code, not into a guess.

## 1. What ships

A learner uploads a PDF book. In the background the server maps it into chapters, then makes **one cheap call per chapter** (or per chapter part) that returns 5 to 12 learning objectives with their quiz items and source pages. All of it is stored as data. After that, "I read chapter N" returns a quiz built from the stored items, graded by code, feeding FSRS cards. Studying and the reading check make **zero** model calls.

Out of scope for U7: screens (U8), deleting a book, OCR of scanned pages, editing objectives, a digest line (U8 reads chapter status from the routes below).

## 2. Decisions this spec fixes

| # | Decision | Why |
|---|---|---|
| D1 | The import runs as a `JobRecord` of a **new** `JobKind::Textbook` in the workspace's existing `jobs/` store, so it appears in `GET …/jobs`, emits `job.updated`, and stops with `POST …/jobs/{id}/abort`. It is **not** run by `Workbench::run`, which is chat-bound (`chat_id`, steer delivery, 180 s timeout). A new `TextbookService` runs it. | Task item 5 (same jobs route and SSE) without bending chat code. |
| D2 | A textbook job has `chat_id: ""` and `delivery: DeliveryStatus::Claimed` for its whole life. `Workbench::recover` skips `JobKind::Textbook` in both its loops; `TextbookService::recover` owns those jobs. | Otherwise `recover` flips them to `Pending`, and `ChatRuntime::job_updated` spawns `wake` on chat `""`. |
| D3 | Book id is content-addressed: `tb_` + the first 16 hex chars of SHA-256 of the PDF bytes (`sha2` is already a dependency of `lp-core`, `lp-service` and `lp-server`). | Importing the same PDF twice finds the same book: no duplicates by construction. |
| D4 | Objectives and items reuse `lp_core::course::Item` unchanged; grading reuses the course grading (`grade` in `crates/lp-service/src/course/mod.rs`, `rating_for` in `lp-core`). | No duplicated domain logic (CLAUDE.md rule). |
| D5 | Card ids are `t:{bookId}:{itemId}`; course cards are `c:{course}:{item}` (`card_id` in `course/mod.rs`). Cards are parked at `PARKED_DUE` until first answered, exactly like course cards. | Review queue never shows an unread chapter; prefixes never collide. |
| D6 | Chapter objectives use the existing cheap role `placer` (`COURSE_ROLES` in `crates/lp-service/src/settings.rs`, default `claude-haiku-5-5`). No new role, no settings change. | Smallest change; the learner already controls the cheap model there. |
| D7 | Budget: an **automatic** import makes at most `MAX_AUTO_CALLS = 40` cheap calls per book, counting the TOC call, every part and every retry. Chapters past the cap stay `queued`. A learner request (`POST …/import` with chapter indices) starts a new job with its own cap of 40 calls, recorded with `requested: true`. The usage route shows both totals. | Reconciles "capped at 40 per book" with "the rest are queued and imported on request". **Flag for the goldfish reviewer:** if the owner means a hard 40 for the book's lifetime, set `requested` calls to count against the same 40 and drop D7's second sentence; nothing else changes. |
| D8 | The upload route is `POST …/sources/textbook` in `crates/lp-server/src/routes/sources.rs`, reusing its multipart parser. Everything else is in the new `routes/textbook.rs`. | The task names both files. |
| D9 | The PDF input cap in the extractor rises from 40 MiB to 50 MiB (`MAX_PDF_INPUT_BYTES` in `crates/lp-agent/src/workbench/adapters.rs`), matching `MAX_PDF_BYTES = 50 * 1024 * 1024` in `sources.rs`. | Today a 45 MB upload is accepted and then fails in `pdf_path`. |
| D10 | Page text is not stored. A retry or a requested import reads the stored PDF again. | Keeps the workspace small; extraction is cheap compared with a model call. |

## 3. Pure types and functions (`crates/lp-core/src/textbook.rs`, new)

Register with `pub mod textbook;` in `crates/lp-core/src/lib.rs`. No IO, no clock, no RNG: `now` and seeds are arguments. Every struct derives `Clone, Debug, PartialEq, Serialize, Deserialize` (plus `Eq` where fields allow) and uses `#[serde(default)]` on new optional fields.

### 3.1 Constants

| Name | Value | Meaning |
|---|---|---|
| `MAX_AUTO_CALLS` | `40` | Cheap calls per automatic import (D7), and per requested import. |
| `PART_WORDS` | `8_000` | A chapter over this many words is split. |
| `MAX_PARTS` | `3` | Parts per chapter at most. |
| `MAX_PART_WORDS` | `10_000` | Hard cap on words sent in one call; the rest of the part is cut and the chapter marked `truncated`. |
| `MIN_CHAPTER_WORDS` | `300` | Below this a chapter is `skipped` (no call). |
| `MIN_OBJECTIVES` / `MAX_OBJECTIVES` | `5` / `12` | Asked for per chapter. |
| `MIN_KEPT_OBJECTIVES` | `3` | Fewer surviving the gate means the chapter failed. |
| `MAX_ITEMS_PER_OBJECTIVE` | `3` | Extra items are dropped. |
| `MAX_ATTEMPTS` | `2` | A chapter is tried at most twice per job (first try plus one retry). |
| `MAX_CHAPTERS` | `200` | A map with more entries is cut to the first 200 by page. |
| `TOC_PAGES` | `20` | Pages sent to the TOC call when there is no outline. |
| `TOC_CHARS` | `24_000` | Character cap on that call's text. |
| `CHUNK_PAGES` | `20` | Fallback chapter size when no map can be found. |
| `CHECK_ITEMS` | `8` | Items in one reading check at most, one per objective. |
| `MAX_PAGE_CHARS` | `20_000` | A single page's text is cut here after sanitising. |

### 3.2 Types

```rust
pub struct PageRange { pub start: u32, pub end: u32 }            // physical, 1-based, inclusive

pub struct OutlineEntry { pub title: String, pub page: u32, pub depth: u8 }

pub enum ChapterKind { Chapter, Front, Back }                     // serde snake_case

pub struct Chapter {
    pub index: u32,                                               // 0-based, dense, in page order
    pub title: String,                                            // sanitised, at most 200 chars
    pub pages: PageRange,
    pub kind: ChapterKind,
}

pub enum MapSource { Outline, Toc, Chunks }                       // serde snake_case

pub struct Objective {
    pub id: String,                                               // "ch{index:03}-o{n:02}", n from 1
    pub text: String,                                             // at most 300 chars
    pub pages: PageRange,                                         // clamped into the part's range
    pub items: Vec<lp_core::course::Item>,                        // ids "ch{index:03}-o{n:02}-i{k}"
}

pub struct BookMeta {
    pub id: String,                                               // "tb_" + 16 lowercase hex
    pub title: String,                                            // file stem, sanitised, at most 200 chars
    pub file_name: String,
    pub bytes: u64,
    pub sha256: String,                                           // full 64 hex
    pub total_pages: u32,
    pub page_offset: Option<i32>,                                 // physical minus printed; None if not found
}

#[serde(tag = "type", rename_all = "snake_case")]
pub enum TextbookEvent {
    Imported   { ts: i64, book: BookMeta },
    Mapped     { ts: i64, source: MapSource, chapters: Vec<Chapter> },
    ChapterDone   { ts: i64, chapter: u32, parts: u8, truncated: bool, objectives: Vec<Objective> },
    ChapterFailed { ts: i64, chapter: u32, reason: String, retryable: bool },
    ChapterSkipped{ ts: i64, chapter: u32, reason: String },     // too short, or front/back matter
    Usage      { ts: i64, job: String, chapter: Option<u32>, role: String,
                 tokens_in: u64, tokens_out: u64, calls: u32, requested: bool },
    CheckIssued{ ts: i64, check: String, chapter: u32, items: Vec<String> },
    Answered   { ts: i64, check: String, item: String, correct: bool, close: bool,
                 rating: Option<lp_core::fsrs::Rating>,
                 #[serde(default)] client_id: Option<String> },
}

pub enum ChapterState { Pending, Ready, Failed, Skipped }        // derived, never stored

pub struct ChapterView {                                          // fold output per chapter
    pub chapter: Chapter,
    pub state: ChapterState,
    pub objectives: Vec<Objective>,
    pub error: Option<String>,
    pub truncated: bool,
    pub checked: bool,                                            // at least one Answered for this chapter
}

pub struct BookState {
    pub meta: Option<BookMeta>,
    pub source: Option<MapSource>,
    pub chapters: Vec<ChapterView>,
    pub auto_calls: u32,
    pub requested_calls: u32,
    pub cheap_in: u64,
    pub cheap_out: u64,
    pub checks: Vec<(String, u32, Vec<String>)>,                  // check id, chapter, item ids
    pub answered: Vec<(String, String)>,                          // (check, item) already answered
}

pub enum ChapterStatus { NotRead, Checked, Due }                  // for U8; serde snake_case
```

### 3.3 Functions

| Signature | Rule |
|---|---|
| `pub fn sanitize_page(text: &str) -> String` | Drop every char that is `is_control()` except `\n` and `\t`; drop U+FEFF and U+200B..U+200F, U+202A..U+202E, U+2066..U+2069 (bidi overrides); collapse runs of more than 2 blank lines; cut at `MAX_PAGE_CHARS` on a char boundary. |
| `pub fn word_count(text: &str) -> usize` | `split_whitespace().count()`. |
| `pub fn book_id(sha256_hex: &str) -> String` | `format!("tb_{}", &hex[..16])`; the caller passes lowercase hex. |
| `pub fn valid_book_id(id: &str) -> bool` | Exactly `tb_` plus 16 of `[0-9a-f]`. |
| `pub fn printed_offset(pages: &[String]) -> Option<i32>` | For each physical page `p` (1-based), the printed number is the first or last non-blank line when that line is only ASCII digits (1 to 4 of them). Roman numerals are never matched, so front matter is passed over. Returns `p - n` for the first page whose offset is repeated by the next two pages that carry a number. `None` if none qualifies. |
| `pub fn chapters_from_outline(entries: &[OutlineEntry], total: u32) -> Vec<Chapter>` | Level: depth 0, unless depth 0 has fewer than 3 entries and depth 1 has at least 3, then depth 1. Keep entries of that level with `1 <= page <= total`, sort by page (stable). Entries that share a start page merge into one, titles joined with " / ". `end` is the next start minus 1; the last ends at `total`. Pages before the first start belong to no chapter. `kind` from `kind_of`. Cut to `MAX_CHAPTERS`. |
| `pub fn chapters_from_toc(entries: &[(String, u32)], offset: Option<i32>, pages: &[String], total: u32) -> Vec<Chapter>` | `entries` are (title, printed page) from the model. Mapped page is `printed + offset.unwrap_or(0)`; then the first page in `mapped-3 ..= mapped+3` whose text contains the title's first 30 chars (case-insensitive, after `sanitize_page`) wins; otherwise the mapped page. Out-of-range pages are dropped. Then the same sort, merge and end rules as the outline path. |
| `pub fn chunk_chapters(total: u32) -> Vec<Chapter>` | Fixed ranges of `CHUNK_PAGES`, titled `Pages {start}–{end}`, all `Chapter`. |
| `pub fn kind_of(title: &str) -> ChapterKind` | Case-insensitive on the trimmed title. `Front` for: contents, table of contents, copyright, preface, foreword, dedication, acknowledgments, acknowledgements, about this book, title page. `Back` for: index, colophon, about the author(s), bibliography, references, glossary. Otherwise `Chapter`. Appendices are `Chapter`. |
| `pub fn split_parts(page_words: &[(u32, usize)]) -> Vec<(PageRange, bool)>` | Input is one entry per page of the chapter. Total under `PART_WORDS` gives one part. Otherwise `k = min(MAX_PARTS, ceil(total / PART_WORDS))` contiguous page groups with word counts as even as page boundaries allow (greedy on the running sum against `total * i / k`). The bool is true when that group has more than `MAX_PART_WORDS` words, so its text is cut. |
| `pub fn objective_bounds(parts: usize) -> (usize, usize)` | `(ceil(MIN_OBJECTIVES / parts), MAX_OBJECTIVES / parts)`: 1 part gives (5, 12), 2 give (3, 6), 3 give (2, 4). |
| `pub fn accept_objectives(chapter: u32, first_n: usize, part: PageRange, raw: Vec<RawObjective>) -> Vec<Objective>` | `RawObjective { text: String, start_page: u32, end_page: u32, items: Vec<Item> }` is the parsed reply. Ids are assigned here, numbered from `first_n`. Page ranges are clamped into `part` (an inverted range becomes `part`). Items are cut to `MAX_ITEMS_PER_OBJECTIVE`, and any item where `gate_item` returns a `Level::Drop` flaw is removed. An MCQ must have exactly 4 options with the answer among them. An objective left with no item is dropped. Text is cut to 300 chars. |
| `pub fn fold_book(events: &[TextbookEvent]) -> BookState` | Replays the ledger in order. The first `Imported` and the first `Mapped` win; later ones are ignored, which is the idempotence guard. Per chapter, the first `ChapterDone` wins and is final. `ChapterFailed` and `ChapterSkipped` only count while the chapter is not `Ready`, so a later retry's `ChapterDone` overrides a failure. Usage sums into `auto_calls` or `requested_calls` by the `requested` flag. |
| `pub fn pending_chapters(state: &BookState) -> Vec<u32>` | Chapters in `Pending` or (`Failed` and retryable), in index order, `kind == Chapter` only. |
| `pub fn select_check(state: &BookState, chapter: u32, due: &BTreeSet<String>, reviewed: &BTreeSet<String>, seed: &str) -> Vec<String>` | Card ids come in; item ids go out. Pick one item per objective, at most `CHECK_ITEMS`. Order of objectives: those with a due card first, then never-reviewed ones, then the rest, each group in objective order. Within an objective, prefer a due item, then a never-reviewed one, then the first, with ties broken by a stable hash of `seed + item id`. Pure and deterministic. |
| `pub fn chapter_status(view: &ChapterView, due: &BTreeSet<String>, book: &str) -> ChapterStatus` | `Due` if any of its item cards (`t:{book}:{item}`) is in `due`. Otherwise `Checked` if `view.checked`. Otherwise `NotRead`. |
| `pub fn card_id(book: &str, item: &str) -> String` | `format!("t:{book}:{item}")`. |

Invariants, enforced by `proptest` (section 9): for every outline, TOC or chunk input, the chapters are sorted by `start`, `1 <= start <= end <= total`, `end < next.start`, the last `end == total` when non-empty, indices are `0..len` dense, and `len <= MAX_CHAPTERS`. `split_parts` covers the chapter's pages exactly once, in order, with between 1 and 3 parts.

## 4. Ledger and files (`crates/lp-store/src/textbooks.rs`, new)

Layout under each workspace root (from `Stores::open(root)` in `crates/lp-service/src/stores.rs`, never `~/.learnpuppy`):

```
<root>/textbooks/<bookId>/source.pdf       the upload, mode 0600, written once
<root>/textbooks/<bookId>/textbook.jsonl   append-only TextbookEvent ledger
```

Lines use the existing crash-safe format of `crates/lp-store/src/jsonl.rs`: `<crc32 hex>\t<json>\n`, through `append` (lock file, torn-tail trim with `trim_tail`, fsync) and `read_all` (corrupt interior lines skipped). No new line format.

```rust
pub struct FsTextbookStore { root: PathBuf }
impl FsTextbookStore {
    pub fn new(root: impl Into<PathBuf>) -> Self;                         // creates nothing
    pub fn append_event(&self, book: &str, ev: &TextbookEvent) -> StoreResult<()>;
    pub fn events(&self, book: &str) -> StoreResult<Vec<TextbookEvent>>;  // missing file is Ok(vec![])
    pub fn exists(&self, book: &str) -> StoreResult<bool>;                // ledger has an Imported line
    pub fn list(&self) -> StoreResult<Vec<String>>;                       // valid book ids, sorted
    pub fn save_source(&self, book: &str, pdf: &[u8]) -> StoreResult<()>; // write_atomic, under lock
    pub fn source_path(&self, book: &str) -> StoreResult<PathBuf>;
}
```

Every method first checks the id with `lp_core::textbook::valid_book_id` and returns `StoreError::Backend("invalid book id …")` otherwise, so an id can never leave `textbooks/`. Directories are made with `create_dir` (owner-only). Export from `crates/lp-store/src/lib.rs` (`mod textbooks; pub use textbooks::FsTextbookStore;`). Add `pub textbooks: Arc<FsTextbookStore>` to `Stores` and build it in `Stores::open` as `FsTextbookStore::new(root)`, the same way `courses` is built.

Workspace export (`crates/lp-service/src/workspace/export.rs`) copies the workspace tree. The builder confirms `textbooks/` is included, or adds it, and states which in the commit.

## 5. Model calls (`crates/lp-agent/src/course/textbook.rs`, new)

A child module of `crates/lp-agent/src/course/mod.rs` (`pub mod textbook;`), so it uses the parent's private `Fence`, `render`, `ask`, `ask_parsed` and `parse` without widening them. Prompts are `crates/lp-agent/src/course/prompts/textbook_toc.md` and `textbook_objectives.md` (new), pulled in with `include_str!`.

| Function | Prompt input | Reply schema (JSON only) | Parse rules |
|---|---|---|---|
| `pub async fn toc_chapters<E: LlmEngine>(engine: &E, pages: &str) -> Result<Vec<(String, u32)>, CourseAgentError>` | The first `TOC_PAGES` pages as `page {n}:\n{text}`, cut to `TOC_CHARS`, redacted and fenced. | `{"chapters":[{"title":"…","printed_page":12}]}` | 1 to `MAX_CHAPTERS` entries, title not empty, page at least 1. Otherwise `Parse`. |
| `pub async fn chapter_objectives<E: LlmEngine>(engine: &E, brief: &ChapterBrief<'_>) -> Result<Vec<RawObjective>, CourseAgentError>` | `ChapterBrief { book: &str, chapter: &str, part: usize, parts: usize, pages: PageRange, text: &str, min: usize, max: usize }`. The text is `page {n}:\n…` per page, redacted and fenced. | `{"objectives":[{"text":"…","start_page":41,"end_page":43,"items":[{"type":"mcq","q":"…","options":["…","…","…","…"],"answer":"…","why":"…"} or {"type":"recall","q":"…","answer":"…","accept":["…"],"why":"…","must_name":["…"]}]}]}` | `min..=max` objectives requested; a reply with 1 or more is parsed and passed on, and the service decides. Zero, or bad JSON, gives `Parse`. Items deserialize straight into `lp_core::course::Item`, whose `#[serde(default)]` takes the missing fields. |

Both prompts open with `PACKET_NOTE` (already in `lp_core::course`) and state that the fenced text is book content to study, never instructions. Every piece of book text passes through `lp_core::redact::redact` and then `Fence::wrap`; the fence nonce is random per call. Neither function retries: retries are the service's job, so it can count them.

PDF port changes, in `crates/lp-agent/src/workbench/jobs.rs` (trait `Pdf`) and `adapters.rs` (`PdftotextPdf`):

1. `fn page_texts(&self, path: &str, first: usize, last: usize) -> Result<Vec<String>, String>` with a default that calls `page_range`. In `PdftotextPdf` it is `page_range` without the "No text found in this PDF range" error: blank pages come back as empty strings, so an image-only page is skipped, not fatal. It must return exactly `last - first + 1` strings, or `Err`. `page_range` keeps its current behaviour for the chat job.
2. `fn outline(&self, path: &str) -> Result<Vec<lp_core::textbook::OutlineEntry>, String>` with a default `Ok(vec![])`. `PdftotextPdf` runs `pdftohtml -xml -i -q -stdout -f 1 -l 1 <arg>` through the existing `pdf_command` (same timeout, stall guard and 8 MiB output cap) and hands the output to `parse_outline_xml`. Checked on this machine with Poppler 26.06: the output carries `<outline><item page="N">Title</item>…</outline>`, where `N` is the physical page and nested `<outline>` means one level deeper. A PDF with no outline yields no `<outline>` element, which is `Ok(vec![])`. If `pdftohtml` is missing, `outline` returns `Ok(vec![])` with a `tracing::warn!`, and the import goes on with the TOC path.
3. `fn parse_outline_xml(xml: &str) -> Vec<OutlineEntry>`, pure and private in `adapters.rs`. It scans only the `<outline>` blocks, tracks depth by nesting, decodes `&amp; &lt; &gt; &quot; &apos; &#N; &#xN;`, and skips items with no `page` attribute. No XML crate is needed.
4. `MAX_PDF_INPUT_BYTES` goes to `50 << 20` (D9), and the message in `pdf_path` becomes "exceeds 50 MiB".

`JobKind` (`crates/lp-core/src/ports.rs`) gains `Textbook`. Every exhaustive match must handle it. Verified sites: `Workbench::run` in `workbench/mod.rs` (it is unreachable there: return the error "textbook jobs run in TextbookService"), `kind_name`, `title_of` and `tool_kind` in `crates/lp-service/src/chat/jobs.rs` (`kind_name` gives `"textbook"`, `title_of` uses key `"title"`, `tool_kind` has no textbook tool), and `crates/lp-server/src/seed.rs` (it only builds records, so check it compiles). `workbench/tools.rs` uses `JobKind` without an exhaustive match and needs no change. The builder greps `JobKind::` again before committing.

## 6. Service (`crates/lp-service/src/textbook.rs`, new)

Register with `pub mod textbook;` in `crates/lp-service/src/lib.rs`. Shape copied from `CourseService` in `crates/lp-service/src/course/mod.rs`: an `Arc<Inner<E>>`, blocking work through a local `blk` (`tokio::task::spawn_blocking`), the clock injectable.

```rust
pub enum TextbookError { NotFound(String), Invalid(String), Conflict(String),
                         Unavailable(String), Store(String) }       // From<StoreError>, From<io::Error>

pub struct TextbookService<E> { inner: Arc<Inner<E>> }

impl<E: LlmEngine + 'static> TextbookService<E> {
    pub fn new(root: PathBuf, stores: Arc<Stores>, engines: Engines<E>, pdf: Arc<dyn Pdf>,
               device: String, notify: Arc<dyn Fn(JobRecord) + Send + Sync>) -> Self;  // runs recover()
    pub fn with_clock(self, clock: Clock) -> Self;
    pub async fn upload(&self, file_name: &str, pdf: Bytes, busy: BusyGuard) -> Res<Upload>;
    pub async fn import(&self, book: &str, chapters: Option<Vec<u32>>, busy: BusyGuard) -> Res<String>; // job id
    pub fn abort(&self, job_id: &str) -> bool;                       // true if this service owned it
    pub async fn list(&self) -> Res<Vec<BookSummary>>;
    pub async fn book(&self, book: &str) -> Res<BookView>;
    pub async fn chapter(&self, book: &str, chapter: u32) -> Res<ChapterDetail>;
    pub async fn start_check(&self, book: &str, chapter: u32) -> Res<CheckView>;
    pub async fn answer(&self, book: &str, check: &str, item: &str, text: &str,
                        confidence: Option<u8>, ms: Option<u64>) -> Res<CheckFeedback>;
    pub async fn self_grade(&self, book: &str, check: &str, item: &str, grade: &str) -> Res<()>;
    pub async fn usage(&self, book: &str) -> Res<TextbookUsage>;
}
```

`Engines<E>` is `crate::chat::Engines` (runtime.rs). `Pdf` is `lp_agent::workbench::Pdf`; production passes `Arc::new(PdftotextPdf)`. `BusyGuard` is `crate::workspace::registry::BusyGuard`, the same guard `ChatRuntime::job_updated` holds through `handle.busy()`. The job keeps it until it ends, so a workspace cannot be archived mid-import.

Plain result structs (no serde, no HTTP):

- `Upload { book: String, job: Option<String>, existing: bool }`
- `BookSummary { id, title, total_pages, chapters: usize, ready: usize, state: BookPhase, created_at }`, where `BookPhase` is `Importing | Ready | Partial | Failed`, derived from the fold plus whether a job is running.
- `BookView { meta: BookMeta, source: Option<MapSource>, chapters: Vec<ChapterRow>, job: Option<String>, auto_calls: u32, requested_calls: u32 }`
- `ChapterRow { index, title, pages: PageRange, kind: ChapterKind, state: ChapterRowState, objectives: usize, status: ChapterStatus, error: Option<String>, truncated: bool }`, where `ChapterRowState` is `Pending | Importing | Ready | Failed | Skipped | Queued`. `Importing` is the chapter the running job is on. `Queued` means pending with no running job, after the cap was hit.
- `ChapterDetail { row: ChapterRow, objectives: Vec<ObjectiveRow> }`, with `ObjectiveRow { id, text, pages: PageRange, items: usize }`. It never carries items.
- `CheckView { check: String, chapter: u32, items: Vec<CheckItem> }`, with `CheckItem { id, objective: String, kind: ItemKind, q: String, options: Vec<String> }`. The options are shuffled with the course's `shuffled` helper, seeded by `t:{book}:{item}`. There is never an `answer`, `accept`, `why` or `must_name`.
- `CheckFeedback { correct: bool, close: bool, answer: String, why: String, due: Option<i64> }`
- `TextbookUsage { cheap_in, cheap_out, auto_calls, requested_calls, cap: u32 }`

### 6.1 Upload

1. The route has already checked the 50 MB cap, the `.pdf` name and the `%PDF-` magic (section 7). Compute SHA-256 → `book_id`.
2. If `stores.textbooks.exists(book)`, return `Upload { existing: true, job: <the running job for this book, or None> }`. Nothing is written and nothing starts.
3. Otherwise `save_source`, read `page_count` through the `Pdf` port in `spawn_blocking` (a failure deletes the book dir and returns `Invalid("…")` with the extractor's message), append `Imported`, and start the job (6.2) with `chapters: None`.
4. A second identical upload while the first is still saving: a per-book `Mutex` in `Inner` (as in `CourseService::lock_for`) serialises steps 2 and 3, so the second caller sees `existing: true`.

### 6.2 Job lifecycle

```mermaid
graph TD
  start["import(): JobRecord kind Textbook, Running, delivery Claimed"] --> pages["read pages 10 at a time: page_texts in spawn_blocking"]
  pages --> mapped{"Mapped in ledger?"}
  mapped -->|"yes"| work
  mapped -->|"no"| outline["outline(): 0 calls"]
  outline -->|"entries"| save["append Mapped"]
  outline -->|"none"| toc["toc_chapters: 1 call, counted"]
  toc -->|"ok"| save
  toc -->|"fails"| chunks["chunk_chapters"] --> save
  save --> work["for each chapter in pending_chapters or the requested list"]
  work --> cap{"calls this job < 40?"}
  cap -->|"no"| finish
  cap -->|"yes"| call["chapter_objectives per part"]
  call -->|"ok"| done["append ChapterDone, upsert parked cards"]
  call -->|"parse error, attempt 1"| call
  call -->|"engine error twice running"| finish
  done --> work
  work -->|"none left"| finish["status Done, result summary"]
  start -.->|"abort"| cancelled["status Failed, error cancelled"]
```

- **Start.** `import` returns `Conflict("an import is already running for this book")` if `Inner.running` already has the book. Otherwise it creates `JobRecord { id: "tbj_{hex16}_{n}", chat_id: "", kind: Textbook, status: Running, delivery: Claimed, created_at: now, result: Some({title, bookId, progress}), usage: vec![] }`, where `n` is the count of this book's earlier jobs in `stores.jobs`. It writes the record with `stores.jobs.create_job`, calls `notify`, stores a `CancellationToken` in `Inner.running`, `tokio::spawn`s the runner, and returns the id. `chapters: Some(list)` is only valid for indices that exist and are not `Ready`; anything else is `Invalid`.
- **Progress.** `result` is `{"title":"Import <book title>","bookId":…,"progress":{"phase":"pages"|"map"|"objectives","pagesRead":u32,"totalPages":u32,"chaptersDone":u32,"chaptersTotal":u32}}`. It is written with `update_job` and then `notify` at most once per 10 pages and once per chapter. `pagesRead` and `totalPages` use the same keys `progress_field` in `chat/runtime.rs` already reads, so `JobItemDto.pages_read` and `total_pages` fill with no change.
- **Pages.** Pages are read with `page_texts` in 10-page windows (`MAX_PDF_PAGES_PER_READ`), each in `spawn_blocking`, checking the cancel token between windows. Each page goes through `sanitize_page`. The pages are held in memory for the job only (400 pages × 20,000 chars is at most 8 MB). `printed_offset` runs once; if `BookMeta.page_offset` differs, the ledger is not rewritten: the value is used for this job only.
- **Map.** Done once per book, guarded by the first `Mapped`. Use the outline if it gives at least 2 chapters, else the TOC call, else chunks.
- **Chapters.** Chapters run one at a time. `Front` and `Back` chapters, and chapters under `MIN_CHAPTER_WORDS`, get `ChapterSkipped` with no call. For the rest, `split_parts` runs, then one `chapter_objectives` call per part on `engines("placer")`, a fresh engine per call so `usage()` deltas are exact, read before and after as `CourseService::ask` does. Each call is bounded by `tokio::time::timeout(120 s)` and `select!` on the cancel token. After every call, `Usage { calls: 1, … }` is appended **whether the call succeeded or not**, because a call counts once it was sent. The parts' results are merged through `accept_objectives`, numbered across parts. Fewer than `MIN_KEPT_OBJECTIVES` counts as a parse failure. On success, `ChapterDone` is appended, then cards are upserted (6.4).
- **Retry.** A `CourseAgentError::Parse` (or a timeout) retries the whole chapter once (`MAX_ATTEMPTS`). The second failure appends `ChapterFailed { retryable: true }` and the job moves on. A `CourseAgentError::Engine(_)` appends `ChapterFailed { retryable: true, reason: "engine_unavailable" }`; two engine errors in a row end the job early, with `result.stopped = "engine_unavailable"`, and the remaining chapters stay pending.
- **Cap.** Before each call, the job checks `calls_this_job < MAX_AUTO_CALLS`. At the cap it stops starting calls; the rest stay pending and show as `Queued`. A split chapter that would cross the cap is not started (no half chapters).
- **End.** `status: Done`, with `result.summary = {ready, failed, skipped, queued, calls}` and `result.error` set only when the job could not run at all (PDF unreadable, store failure). The record is written, `notify` called, `Inner.running` cleared and the `BusyGuard` dropped.
- **Cancel.** `abort(job_id)` cancels the token and returns true. The runner notices at the next window or call, appends nothing more, and sets `status: Failed`, `result.error = "cancelled"`. Chapters already `Ready` stay. The partial call that was in flight still has its `Usage` appended if a reply came back.
- **Recover.** `TextbookService::new` runs `recover()`. Every `JobKind::Textbook` record still `Running` in `stores.jobs.all_report()` becomes `Done` with `result.error = "interrupted by a restart"`, and `notify` is called. The ledger is untouched; `POST …/import` resumes from `pending_chapters`.

### 6.3 Reading check

1. `start_check(book, chapter)`: the chapter must be `Ready`, else `Conflict("chapter_not_ready")`. Read the due cards with `review::due_cards(tracker, now)` and the reviewed ones with `review::history` (both in `crates/lp-service/src/review.rs`), filtered to the `t:{book}:` prefix. Call `select_check` with `seed = check id`, where the check id is `rc_` + `uuid::Uuid::new_v4().simple()`. Append `CheckIssued`. **No engine is touched.**
2. `answer(book, check, item, …)`: the check must exist in the fold and contain `item` (else `NotFound`). An `(check, item)` pair already answered gives `Conflict("already_answered")`. Grade with the course's `grade(item, text)`. Rating as in `CourseService::answer_sync`: `rating_for(correct, confidence)` for an MCQ or a correct recall, and none for a close or wrong recall until `self_grade`. Then `review::grade(tracker, device, card_id, rating, now)`; `StoreError::NotFound` (an item with no card) is ignored, as `grade_card` does. Append `Answered`. Return `CheckFeedback`, with `due` the card's new due date. Text limits are the course ones (`MAX_TEXT_CHARS`, confidence 1 to 5).
3. `self_grade`: only for an answered recall item with no rating yet. Uses `rating_from_grade`, then `review::grade`.

### 6.4 Cards

After each `ChapterDone`, every item gets a `FlashCard` with id `t:{book}:{item}`, `card: Card::new(PARKED_DUE)`, inserted only if `tracker.card(id)` is `None`, as `save_course` does. To avoid a second copy of the card builder, refactor `flash_card` in `crates/lp-service/src/course/mod.rs` into `pub(crate) fn card_for(id: &str, item: &Item, other_answers: &[&str]) -> Option<FlashCard>` (MCQ keeps its options; recall takes its answer plus 3 distinct others, or gets no card). `flash_card` becomes a thin caller, `textbook.rs` calls `card_for` with the chapter's other answers, and `PARKED_DUE` and `shuffled` become `pub(crate)`. Course behaviour must not change: the existing course tests stay green, untouched.

### 6.5 Errors and cost guarantees

| Case | Service result |
|---|---|
| Unknown or invalid book id, chapter index, check or item | `NotFound` |
| Upload is not a readable PDF (`page_count` fails, encrypted, 0 pages) | `Invalid(extractor message)`, book dir removed |
| `import` while one runs | `Conflict` |
| `import` with no pending chapters | `Conflict("nothing to import")` |
| Reading check on a chapter that is not `Ready` | `Conflict("chapter_not_ready")` |
| Answer twice | `Conflict("already_answered")` |
| Store failure | `Store` |
| Every page blank (scanned book) | the job ends `Done`, `error = "No text found. Scanned pages need OCR before import."`, no model call |

Cost: `list`, `book`, `chapter`, `start_check`, `answer`, `self_grade` and `usage` never call `engines`. The only call sites are the TOC call and `chapter_objectives` inside the runner. Textbook usage lives in `textbook.jsonl`, and `CourseService::strong_stamps` only reads `courses/*/events.jsonl`, so the import never touches the G4 strong cap. No cheap hourly cap exists in code today, so there is nothing else to exempt.

## 7. Server routes and DTOs

### 7.1 Wiring

- `crates/lp-server/src/app.rs`: add `textbooks: Arc<parking_lot::Mutex<HashMap<String, TextbookService<RoleEngine>>>>` and `pub fn textbook_service(&self, ws: &str, stores: &Stores, root: &Path) -> TextbookService<RoleEngine>`. Build engines exactly as `course_service` does: lift the engines closure into a private `fn role_engines(&self) -> Engines<RoleEngine>` that both call. `notify` publishes `Event::JobUpdated { job: job_info(&rec) }` with `self.hub.publish(Scope::Workspace(ws.to_owned()), …)`. `job_info` in `crates/lp-service/src/chat/runtime.rs` becomes `pub` for that. `pdf` is `Arc::new(PdftotextPdf)`. In faux mode, tests can inject a fake `Pdf` through a `#[cfg(test)]` or test-hook setter. The builder picks one and notes it.
- `crates/lp-server/src/routes/mod.rs`: `.merge(textbook::routes())`. `crates/lp-server/src/openapi.rs`: every new path and schema.
- `crates/lp-server/src/routes/jobs.rs` `abort`: first `st.textbook_service(…).abort(&id)`; if true, return the job's current `JobItemDto` (read through `st.chats.jobs`); else fall through to `st.chats.abort_job` as today. A finished textbook job is still `409` through the existing path.
- `JobView` and `JobItemDto` gain `book_id: Option<String>` (from `result.bookId`), `chapters_done: Option<u64>` and `chapters_total: Option<u64>` (from `result.progress`, via `progress_field`). `chat_id` is `""` for textbook jobs. `kind` doc string adds `textbook`.

### 7.2 Routes

Ids: `{book}` must match `valid_book_id`, else `404`. `{check}` is `rc_[0-9a-f]{32}`, `{n}` is a `u32`. Errors map as the courses route maps `CourseError`, plus `Conflict` → `409 conflict` with `details.reason` set to the stable code (`chapter_not_ready`, `already_answered`, `import_running`, `nothing_to_import`). All handlers use `open_writable` for writes and `open_ws` for reads (`routes/common.rs`), with no `unwrap()`.

| Method and path | Body | Success | Errors |
|---|---|---|---|
| `POST /workspaces/{ws}/sources/textbook` (in `sources.rs`) | multipart, one field `file` (`.pdf`, magic `%PDF-`, at most `MAX_PDF_BYTES`); no `question` field | `202 TextbookUploadDto` for a new book; `200 TextbookUploadDto` with `existing: true` | `400 validation` (bad form, name, magic, unreadable PDF); `413` with `details.maxBytes` (same path as `pdf`); `404` workspace; `409` archived workspace |
| `GET /workspaces/{ws}/textbooks` | – | `200 TextbookListDto` | `404` |
| `GET /workspaces/{ws}/textbooks/{book}` | – | `200 TextbookDto` | `404` |
| `GET /workspaces/{ws}/textbooks/{book}/chapters/{n}` | – | `200 ChapterDetailDto` | `404` |
| `POST /workspaces/{ws}/textbooks/{book}/import` | `ImportRequest { chapters?: u32[] }` (at most 200) | `202 ImportStartedDto` | `400`, `404`, `409 import_running`, `409 nothing_to_import` |
| `POST /workspaces/{ws}/textbooks/{book}/chapters/{n}/check` | – | `200 ReadingCheckDto` | `404`, `409 chapter_not_ready` |
| `POST /workspaces/{ws}/textbooks/{book}/checks/{check}/answer` | `CheckAnswerRequest { item, text, confidence?, ms?, clientId? }` | `200 CheckFeedbackDto` | `400`, `404`, `409 already_answered` |
| `POST /workspaces/{ws}/textbooks/{book}/checks/{check}/self-grade` | `SelfGradeRequest` (reuse the course DTO) | `204` | `400`, `404`, `409` |
| `GET /workspaces/{ws}/textbooks/{book}/usage` | – | `200 TextbookUsageDto` | `404` |
| (existing) `GET /workspaces/{ws}/jobs`, `POST …/jobs/{id}/abort`, SSE `job.updated` | – | textbook jobs appear here | as today |

A retry with the same `clientId` on `answer` returns the stored feedback, not `409`: the service keeps the client id in `Answered.client_id` and replays the match.

### 7.3 DTOs (`crates/lp-server/src/dto/textbook.rs`, new)

All `#[derive(Serialize, ToSchema)]` (requests `Deserialize`), `#[serde(rename_all = "camelCase")]`, with `From<service struct>` impls and a round-trip unit test each, as in `docs/web/03-server-api.md` §DTO rule.

```
TextbookUploadDto   { bookId, jobId: string|null, existing: bool, bytes: u64 }
TextbookListDto     { items: [TextbookSummaryDto] }
TextbookSummaryDto  { id, title, totalPages, chapters, ready, state: "importing"|"ready"|"partial"|"failed", createdAt }
TextbookDto         { id, title, fileName, totalPages, pageOffset: i32|null, mapSource: "outline"|"toc"|"chunks"|null,
                      jobId: string|null, autoCalls, requestedCalls, callsCap: 40,
                      chapters: [ChapterRowDto] }
ChapterRowDto       { index, title, startPage, endPage, kind: "chapter"|"front"|"back",
                      state: "pending"|"importing"|"ready"|"failed"|"skipped"|"queued",
                      status: "not_read"|"checked"|"due", objectives: u32, error: string|null, truncated: bool }
ChapterDetailDto    { chapter: ChapterRowDto, objectives: [{ id, text, startPage, endPage, items: u32 }] }
ImportRequest       { chapters?: [u32] }
ImportStartedDto    { jobId }
ReadingCheckDto     { checkId, chapter, items: [{ id, objectiveId, kind: "mcq"|"recall", q, options: [string] }] }
CheckAnswerRequest  { item, text, confidence?: 1..5, ms?: u64, clientId?: string }
CheckFeedbackDto    { correct, close, answer, why, due: i64|null }
TextbookUsageDto    { cheapIn, cheapOut, autoCalls, requestedCalls, callsCap }
```

**Leak rule:** `ReadingCheckDto` and `ChapterDetailDto` never contain `answer`, `accept`, `why`, `mustName`, `misconceptions` or `correct`. A test serializes both to JSON and asserts none of those keys appear at any depth.

Regenerate `web/openapi/openapi.json` with `learnpuppy openapi`, and its client with `web/scripts/gen-api.ts`. Never hand-merge them (U7 lands after U3, U4 and U5: rebase first).

## 8. Safety checklist

- Upload: the 50 MB cap, the `.pdf` name and the `%PDF-` magic are all enforced by the reused `form` in `sources.rs` (make `form`, `boundary`, `too_large` and `bad` `pub(super)`; `form` takes a mode enum, `Pdf | Note | Textbook`, in place of `note: bool`, and `Textbook` rejects a `question` field).
- Path safety: book ids are hex only; `source_path` is built by the store; `pdf_path` already prefixes `./` to a leading `-`.
- Extracted text: `sanitize_page`, then the per-page and per-part caps, then `redact`, then the fence. No book text ever reaches a log line (`tracing` fields carry ids and counts only).
- Subprocesses: only `pdfinfo`, `pdftotext` and `pdftohtml`, through `pdf_command` (null stdin, stderr dropped, timeout, stall guard, output cap, kill on exit).
- Server: the loopback bind and auth middleware are unchanged; no route returns file paths (the job `result` stores the `bookId`, not the PDF path).

## 9. Tests per crate

Every test must fail without its change (test plan rule 2).

**lp-core (`textbook.rs` `#[cfg(test)]`)**
1. `sanitize_strips_controls_and_bidi_keeps_newlines`
2. `printed_offset_skips_roman_front_matter`: pages `i ii iii iv 1 2 3` give offset 4.
3. `printed_offset_none_without_numbers`
4. `outline_level_falls_to_depth_one_for_parts`
5. `outline_same_start_pages_merge`
6. `toc_maps_printed_pages_with_offset_and_title_search`
7. `kind_of_front_back_and_appendix`
8. `split_parts_8k_boundary_and_three_part_cap`, including a 40k-word chapter marked truncated.
9. `objective_bounds_per_parts`
10. `accept_objectives_clamps_pages_drops_bad_items_and_numbers_ids`
11. `fold_first_mapped_and_first_done_win`: a doubled `Mapped` or `ChapterDone` yields no duplicates.
12. `fold_retry_done_overrides_failed`
13. `select_check_due_first_one_per_objective_deterministic`
14. `chapter_status_due_checked_not_read`
15. `proptest! chapter_ranges_invariants` for outline, TOC and chunks (section 3 invariants).
16. `proptest! split_parts_covers_exactly`

**lp-store (`crates/lp-store/src/tests.rs` or `textbooks.rs` tests)**
1. `textbook_ledger_round_trip`
2. `textbook_torn_tail_is_trimmed_on_next_append`: write half a line, append, read back every valid event.
3. `textbook_concurrent_appends_all_land`: 2 threads × 100 appends, 200 valid lines, all CRCs good.
4. `textbook_two_workspaces_isolated`: two roots, an append in one leaves the other empty.
5. `textbook_rejects_bad_book_ids`: `../x`, `tb_XYZ`, empty.

**lp-agent**
1. `parse_outline_xml_nested_and_entities` (adapters.rs)
2. `pdftohtml_outline_real_fixture` (skips if Poppler is missing, as `real_pdf_reads_by_range` does)
3. `page_texts_keeps_blank_pages`: an image-only page is an empty string, not an error.
4. `pdf_input_cap_is_50_mib`
5. `chapter_objectives_fences_and_redacts`: the prompt seen by a fake engine has the injection sentence and a fake `api_key=…` inside the fence, and the key is masked.
6. `chapter_objectives_rejects_empty_and_bad_json`
7. `toc_chapters_parses_and_validates`

**lp-service (`crates/lp-service/src/textbook.rs` tests, with a counting fake engine like `Mock` in `course/tests.rs` and a fake `Pdf`)**
1. `upload_maps_outline_with_zero_map_calls`
2. `no_outline_makes_exactly_one_toc_call`
3. `one_call_per_chapter_and_split_parts_counted`: total calls equal chapters plus extra parts.
4. `auto_import_never_exceeds_40_calls`: a 60-chapter fake book gives 40 calls and the rest `Queued`.
5. `requested_import_finishes_queued_chapters`
6. `failed_chapter_retries_once_then_failed_retryable`
7. `two_engine_errors_stop_job_early`
8. `same_pdf_twice_no_duplicate_objectives_or_cards`: card count unchanged, one `Imported` line.
9. `concurrent_identical_uploads_one_book`
10. `cancel_marks_failed_and_keeps_ready_chapters`
11. `restart_recovers_running_job_and_resume_imports_rest`
12. `workbench_recover_skips_textbook_jobs`: run `Workbench::recover` over a `Running` textbook job and assert it is untouched and no wake happens.
13. `reading_check_and_answers_make_zero_calls`: counter at 0 for `list`, `book`, `chapter`, `start_check`, `answer` and `self_grade`.
14. `answer_grades_by_code_and_updates_fsrs`: card leaves `PARKED_DUE`, history length 1.
15. `close_recall_needs_self_grade`
16. `scanned_book_ends_with_ocr_error_no_calls`
17. `course_cards_unchanged_after_card_for_refactor`: existing course tests stay green.
18. Fixture test on `crates/lp-service/tests/fixtures/textbook.pdf` (new folder): 3 chapters, an outline, a two-column page, roman-numeral front matter, an image-only page and an "ignore previous instructions" sentence. Assert the chapter list and page ranges, that the two-column text is present in reading order, that the image page is skipped without error, and that the injection text reaches the engine only inside the fence.

**lp-server (`crates/lp-server/tests/api.rs`, oneshot, faux engine)**
1. `textbook_upload_202_then_200_existing`
2. `textbook_upload_rejects_non_pdf_magic_name_and_413`
3. `textbook_routes_404_on_bad_ids`
4. `textbook_job_listed_and_job_updated_event_emitted`
5. `textbook_abort_through_jobs_route`
6. `reading_check_dto_has_no_answer_fields` (leak test over serialized JSON)
7. `check_answer_idempotent_client_id`
8. `chapter_not_ready_is_409_with_reason`
9. `health_under_200ms_during_400_page_import`: a synthetic 400-page PDF, generated like `multi_page_pdf` in `adapters.rs`, with `GET /health` polled while the job runs. Marked `#[ignore]` only if Poppler is absent.
10. Round-trip tests for every DTO in `dto/textbook.rs`, plus the OpenAPI drift check (`make api-check`).

**Fixture generation.** `crates/lp-service/tests/fixtures/make_textbook_pdf.py` (new; Python standard library only, our own text, so license-clean) writes `textbook.pdf`, which is committed, under 64 kB. The two-column page uses two text objects at different x positions. The image page holds a tiny inline image and no text.

**Manual.** `scripts/real_smoke.py` gains a `--textbook <pdf>` mode: upload, poll `GET …/jobs` until done, print the chapters and two objectives per chapter, and assert `autoCalls <= 40`. It is not in CI.

## 10. Build order (file by file)

Each step compiles and passes `make gate` on its own. Commit per step, or squash at the end.

1. `crates/lp-core/src/textbook.rs` (new) and `lib.rs`: types, constants, functions, lp-core tests and proptests.
2. `crates/lp-core/src/ports.rs`: `JobKind::Textbook`. Fix every match: `crates/lp-agent/src/workbench/mod.rs` (`run`, and `recover` skips Textbook in both loops), `crates/lp-service/src/chat/jobs.rs` (`kind_name`, `title_of`, `tool_kind`), `crates/lp-server/src/seed.rs`. Add the `workbench_recover_skips_textbook_jobs` test in `crates/lp-agent/src/workbench/tests.rs`.
3. `crates/lp-store/src/textbooks.rs` (new), `lib.rs`, store tests. Then `crates/lp-service/src/stores.rs`: the `textbooks` field.
4. `crates/lp-agent/src/workbench/jobs.rs` (trait `Pdf`: `page_texts`, `outline`) and `adapters.rs` (`PdftotextPdf` impls, `parse_outline_xml`, the 50 MiB cap). Then `crates/lp-agent/src/course/textbook.rs` (new), `course/mod.rs` (`pub mod textbook;`), and `prompts/textbook_toc.md` and `prompts/textbook_objectives.md` (new). Re-export `Pdf` and `PdftotextPdf`: both are already `pub use`d from `workbench/mod.rs`.
5. `crates/lp-service/src/course/mod.rs`: the `card_for` refactor, and `PARKED_DUE`, `shuffled` and `grade` made `pub(crate)`. Course tests must stay unchanged and green.
6. `crates/lp-service/src/textbook.rs` (new) and `lib.rs`. `crates/lp-service/src/chat/runtime.rs`: `job_info` made `pub`, and `JobView` gains `book_id`, `chapters_done` and `chapters_total`. Service tests and the fixture (`crates/lp-service/tests/fixtures/`).
7. `crates/lp-server/src/dto/textbook.rs` (new) and `dto/mod.rs`; `dto/chat.rs` `JobItemDto` gets the three new fields.
8. `crates/lp-server/src/app.rs` (`textbook_service`, `role_engines`), `routes/sources.rs` (form mode enum, `POST …/sources/textbook`), `routes/textbook.rs` (new), `routes/mod.rs`, `routes/jobs.rs` (abort routing), `openapi.rs`. Route tests in `crates/lp-server/tests/api.rs`.
9. Regenerate `web/openapi/openapi.json` and the TypeScript client, then run `make gate`.
10. `scripts/real_smoke.py` `--textbook` mode, and a [Review Log](/ui-polish/review.md) entry with the D7 answer and anything the builder changed from this spec.

## 11. Acceptance mapping

| U7 acceptance | Where proven |
|---|---|
| A 400+ page PDF imports without blocking | server test 9, `spawn_blocking` windows, job runner |
| Chapter list correct incl. two-column, image-only, roman front matter | service test 18, lp-core 2 and 15 |
| Same PDF twice, no duplicates | D3, lp-core 11, service tests 8 and 9 |
| Each objective has stored items | `accept_objectives` drops item-less objectives, lp-core 10 |
| Reading check makes zero model calls | service test 13 |
| Torn tail and concurrent append | store tests 2 and 3 |
| Upload rejection and DTO leak | server tests 2 and 6 |
| At most 40 calls per book at import | service test 4, usage route |
