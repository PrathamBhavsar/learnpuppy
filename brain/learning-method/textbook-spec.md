---
type: spec
title: Textbook Mode Backend Spec (U7, F37 part 1)
description: Build spec for textbook import, covering the chapter map, stored objectives and quiz items, the textbook.jsonl ledger, the lazy per-chapter import job, the per-chapter retry and per-hour import limit, reading-check routes and DTOs, error cases, tests per crate and a file-by-file build order with verified paths.
tags: [textbook, f37, u7, spec, pdf, ledger, jobs, fsrs, rate-limit, lazy-import]
timestamp: 2026-11-12
---

# Textbook Mode Backend Spec (U7, F37 part 1)

Back to [Learning Method Plan](/learning-method/index.md). Task: U7 in [UI Polish Task DAG](/ui-polish/tasks.md). Tests: [UI Polish Test Plan](/ui-polish/test-plan.md). Cost rule: [Cheap Tutor Plan](/cheap-tutor/index.md) and [ADR-0007](/decisions/0007-course-as-data.md). Layering: [Learning Method Architecture](/learning-method/architecture.md).

Every path and function named here was read on `staging` at `2fccf39` on 2026-11-11. "New" marks a file or item that does not exist yet. This is a spec, not code: the builder writes the bodies, and the names, shapes, constants and error codes below are fixed. Anything this spec does not settle goes into [Review Log](/ui-polish/review.md) before code, not into a guess.

## 1. What ships

A learner uploads a PDF book. Upload maps it into chapters (the PDF outline, 0 model calls; one cheap TOC call without an outline) and imports **nothing**. When the learner opens a chapter, the server makes **one cheap call per part of that chapter** (one part up to 8,000 words) that returns 5 to 12 learning objectives with their quiz items and source pages. All of it is stored as data. After that, "I read chapter N" returns a quiz built from the stored items, graded by code, feeding FSRS cards. Studying and the reading check make **zero** model calls.

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
| D7 | **Revised by R4 (lazy import).** There is no lifetime cap and no automatic import. Upload maps the book and starts no job. A chapter is imported only when the learner opens it (`POST …/import` with that chapter). Each chapter gets **one try plus one retry** (`MAX_ATTEMPTS = 2`): both happen inside its one import, and a chapter whose try and retry both fail is final (`ChapterFailed { retryable: false }`); an engine outage that spent no content attempt stays retryable. Per book, at most `MAX_MANUAL_IMPORTS_PER_HOUR = 8` chapter imports in a rolling hour, on the server clock, counted from the ledger: the distinct `(job, chapter)` pairs with a requested `Usage` line in the last `IMPORT_WINDOW_MS` (`manual_import_times` in `lp-core`). Parts and retries inside one import do not use extra slots; the TOC call at upload is not an import. One job also stops after `MAX_JOB_CALLS` (8 × 3 parts × 2 attempts = 48) model calls, as a backstop. | Upload used to start a job over every pending chapter, up to the lifetime 40 (about 350k to 450k tokens a book) for chapters nobody read. Spend now follows reading, bounded by chapter, by hour and by job. |
| D8 | The upload route is `POST …/sources/textbook` in `crates/lp-server/src/routes/sources.rs`, reusing its multipart parser. Everything else is in the new `routes/textbook.rs`. | The task names both files. |
| D9 | The PDF input cap in the extractor rises from 40 MiB to 50 MiB (`MAX_PDF_INPUT_BYTES` in `crates/lp-agent/src/workbench/adapters.rs`), matching `MAX_PDF_BYTES = 50 * 1024 * 1024` in `sources.rs`. | Today a 45 MB upload is accepted and then fails in `pdf_path`. |
| D10 | Page text is not stored. A retry or a requested import reads the stored PDF again. | Keeps the workspace small; extraction is cheap compared with a model call. |

## 3. Pure types and functions (`crates/lp-core/src/textbook.rs`, new)

Register with `pub mod textbook;` in `crates/lp-core/src/lib.rs`. No IO, no clock, no RNG: `now` and seeds are arguments. Every struct derives `Clone, Debug, PartialEq, Serialize, Deserialize` (plus `Eq` where fields allow) and uses `#[serde(default)]` on new optional fields.

### 3.1 Constants

| Name | Value | Meaning |
|---|---|---|
| `MAX_MANUAL_IMPORTS_PER_HOUR` | `8` | Chapter imports per book in any rolling `IMPORT_WINDOW_MS` (D7). |
| `IMPORT_WINDOW_MS` | `3_600_000` | The rolling window, in epoch ms, read from the server clock. |
| `MAX_JOB_CALLS` | `48` | Backstop on model calls in one import job (service constant). |
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

pub struct RawObjective {                                         // parsed model reply, before the gate
    pub text: String,
    pub start_page: u32,
    pub end_page: u32,
    pub items: Vec<lp_core::course::Item>,
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
    pub created_at: Option<i64>,                                  // ts of the first Imported line
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
| `pub fn fold_book(events: &[TextbookEvent]) -> BookState` | Replays the ledger in order. The first `Imported` and the first `Mapped` win; `created_at` is the first `Imported.ts`, and `BookSummary.created_at` and `TextbookSummaryDto.createdAt` read it; later ones are ignored, which is the idempotence guard. Per chapter, the first `ChapterDone` wins and is final. `ChapterFailed` and `ChapterSkipped` only count while the chapter is not `Ready`, so a later retry's `ChapterDone` overrides a failure. A `ChapterDone` for a chapter index that is not in the map is ignored. Usage sums into `auto_calls` or `requested_calls` by the `requested` flag. |
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
    pub fn list(&self) -> StoreResult<Vec<String>>;                       // valid book ids with an Imported line, sorted
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

1. `fn page_texts(&self, path: &str, first: usize, last: usize) -> Result<Vec<String>, String>` with a default that calls `page_range`. In `PdftotextPdf` it is `page_range` without the "No text found in this PDF range" error: blank pages come back as empty strings, so an image-only page is skipped, not fatal. It must return exactly `last - first + 1` strings, or `Err`: `page_range` pops a trailing blank page (`adapters.rs:391`), so `page_texts` pads the result with empty strings up to that count (an image-only last page must not shift the numbering) and returns `Err` if the extractor gave more. `page_range` keeps its current behaviour for the chat job.
2. `fn outline(&self, path: &str) -> Result<Vec<lp_core::textbook::OutlineEntry>, String>` with a default `Ok(vec![])`. `PdftotextPdf` runs `pdftohtml -xml -i -q -stdout -f 1 -l 1 <arg>` through the existing `pdf_command` (same timeout, stall guard and 8 MiB output cap) and hands the output to `parse_outline_xml`. Checked on this machine with Poppler 26.06: the output carries `<outline><item page="N">Title</item>…</outline>`, where `N` is the physical page and nested `<outline>` means one level deeper. A PDF with no outline yields no `<outline>` element, which is `Ok(vec![])`. If `pdftohtml` is missing, `outline` returns `Ok(vec![])` with a `tracing::warn!`, and the import goes on with the TOC path.
3. `fn parse_outline_xml(xml: &str) -> Vec<OutlineEntry>`, pure and private in `adapters.rs`. It scans only the `<outline>` blocks, tracks depth by nesting, decodes `&amp; &lt; &gt; &quot; &apos; &#N; &#xN;`, and skips items with no `page` attribute. No XML crate is needed.
4. `MAX_PDF_INPUT_BYTES` goes to `50 << 20` (D9), and the message in `pdf_path` becomes "exceeds 50 MiB".

`JobKind` (`crates/lp-core/src/ports.rs`) gains `Textbook`. Every exhaustive match must handle it. Verified sites by grep: `Workbench::run` in `workbench/mod.rs` has **two** matches on `job.kind`: the `let name = match job.kind` at `workbench/mod.rs:128` (add `JobKind::Textbook => "textbook"`) and the dispatch match at about line 139 (unreachable there: return the error "textbook jobs run in TextbookService"); `Workbench::recover` skips Textbook (lines 192-207, both loops); `kind_name`, `title_of` and `tool_kind` in `crates/lp-service/src/chat/jobs.rs` (`kind_name` gives `"textbook"`, `title_of` uses key `"title"`, `tool_kind` has no textbook tool; also the string-to-kind match near `chat/jobs.rs:86` needs no textbook arm, as there is no tool), and `crates/lp-server/src/seed.rs` (it only builds records, so check it compiles). `workbench/tools.rs` uses `JobKind` without an exhaustive match and needs no change. The builder greps `JobKind::` again before committing.

## 6. Service (`crates/lp-service/src/textbook.rs`, new)

Register with `pub mod textbook;` in `crates/lp-service/src/lib.rs`. Shape copied from `CourseService` in `crates/lp-service/src/course/mod.rs`: an `Arc<Inner<E>>`, blocking work through a local `blk` (`tokio::task::spawn_blocking`), the clock injectable.

```rust
pub enum TextbookError { NotFound(String), Invalid(String), Conflict(String),
                         Unavailable(String), Store(String) }       // From<StoreError>, From<io::Error>

pub struct TextbookService<E> { inner: Arc<Inner<E>> }

impl<E: LlmEngine + 'static> TextbookService<E> {
    pub fn new(root: PathBuf, stores: Arc<Stores>, engines: Engines<E>, pdf: Arc<dyn Pdf>,
               device: String, notify: Arc<dyn Fn(JobRecord) + Send + Sync>) -> Self;  // does not recover; see Recover
    pub fn with_clock(self, clock: Clock) -> Self;
    pub async fn upload(&self, file_name: &str, pdf: bytes::Bytes, busy: BusyGuard) -> Res<Upload>;
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

`Engines<E>` is `crate::chat::Engines` (runtime.rs). `Pdf` is `lp_agent::workbench::Pdf`; production passes `Arc::new(PdftotextPdf)`. New dependencies for `crates/lp-service/Cargo.toml` (step 6): `uuid.workspace = true` (already in the workspace table) and `bytes = "1"` (add to the root `Cargo.toml` `[workspace.dependencies]` and reference it with `bytes.workspace = true`). `tokio-util` (`CancellationToken`) and `sha2` are already there. `BusyGuard` is `crate::workspace::registry::BusyGuard`, the same guard `ChatRuntime::job_updated` holds through `handle.busy()`. The job keeps it until it ends, so a workspace cannot be archived mid-import.

Plain result structs (no serde, no HTTP):

- `Upload { book: String, existing: bool }` (no job: upload starts none)
- `BookSummary { id, title, total_pages, chapters: usize, ready: usize, state: BookPhase, created_at }`, where `BookPhase` is `Importing | Ready | Partial | Failed`, derived from the fold plus whether a job is running.
- `BookView { meta: BookMeta, source: Option<MapSource>, chapters: Vec<ChapterRow>, job: Option<String>, auto_calls: u32, requested_calls: u32, hour_used: u32 }`
- `ChapterRow { index, title, pages: PageRange, kind: ChapterKind, state: ChapterRowState, objectives: usize, status: ChapterStatus, error: Option<String>, truncated: bool, can_import: bool }`, where `ChapterRowState` is `Pending | Importing | Ready | Failed | Skipped`. `Importing` is the chapter the running job is on; `Pending` is a mapped chapter nobody has opened. `can_import` is true for a `Pending` chapter of kind `Chapter` and for a `Failed` chapter whose retry is unspent.
- `ChapterDetail { row: ChapterRow, objectives: Vec<ObjectiveRow> }`, with `ObjectiveRow { id, text, pages: PageRange, items: usize }`. It never carries items.
- `CheckView { check: String, chapter: u32, items: Vec<CheckItem> }`, with `CheckItem { id, objective: String, kind: ItemKind, q: String, options: Vec<String> }`. The options are shuffled with the course's `shuffled` helper, seeded by `t:{book}:{item}`. There is never an `answer`, `accept`, `why` or `must_name`.
- `CheckFeedback { correct: bool, close: bool, answer: String, why: String, due: Option<i64> }`
- `TextbookUsage { cheap_in, cheap_out, auto_calls, requested_calls, hour_used: u32, hour_cap: u32, next_slot_at: Option<i64> }`; `next_slot_at` is set only at the cap and is when the oldest counted import leaves the hour. `auto_calls` is now only the TOC call; chapter calls are `requested_calls`.

### 6.1 Upload

1. The route has already checked the 50 MB cap, the `.pdf` name and the `%PDF-` magic (section 7). Compute SHA-256 → `book_id`.
2. If `stores.textbooks.exists(book)`, return `Upload { existing: true }`. Nothing is written and nothing starts.
3. Otherwise `save_source`, read `page_count` through the `Pdf` port in `spawn_blocking` (a failure deletes the book dir and returns `Invalid("…")` with the extractor's message), append `Imported`, then **map** (below). No job is created and no chapter is imported.
4. A second identical upload while the first is still saving: a per-book `Mutex` in `Inner` (as in `CourseService::lock_for`) serialises steps 2 and 3, so the second caller sees `existing: true`.

**Map.** `outline()` reads no page text. With at least 2 chapters from `chapters_from_outline`, that is the whole map: 0 model calls. Otherwise the first `MAP_PAGES = 60` pages are read in 10-page windows (to find the printed page offset and the TOC), one TOC call is made (counted as a non-requested `Usage` with job `map_{id}`), and the result is `chapters_from_toc` or, under 2 chapters, `chunk_chapters`. Every page of that sample blank (a scanned book with no outline) is `Invalid("No text found. Scanned pages need OCR before import.")` and the book is removed. `Mapped` is appended, then `ChapterSkipped` for front and back matter. A book stored but not mapped (a crash between `Imported` and `Mapped`) is mapped by its first `import`.

`chapters_from_outline` picks the leaf chapters: with fewer than 3 depth-0 entries and at least 3 at depth 1 it uses depth 1; otherwise depth-0 entries named like `Part I`, `Part 2: Basics` or `Book Three` that are followed by depth-1 entries are containers, dropped in favour of their depth-1 children, while other depth-0 entries (Preface, Index) stay.

### 6.2 Job lifecycle

```mermaid
graph TD
  open["learner opens a chapter: POST import with that chapter"] --> check{"hour limit, retry unspent, not Ready or Skipped?"}
  check -->|"no"| refuse["409 import_rate_limited or chapter_retry_used, 400 for Ready or Skipped"]
  check -->|"yes"| start["import(): JobRecord kind Textbook, Running, delivery Claimed"]
  start --> work["for each listed chapter, once, in order"]
  work --> state{"Pending, or Failed and retryable?"}
  state -->|"no"| work
  state -->|"yes"| pages["read only this chapter's pages, 10 at a time"]
  pages --> limit{"hour used < 8 and job calls fit?"}
  limit -->|"no"| finish
  limit -->|"yes"| call["chapter_objectives per part"]
  call -->|"ok"| done["append ChapterDone, upsert parked cards"]
  call -->|"parse error or timeout, attempt 1"| call
  call -->|"second failure"| failed["ChapterFailed retryable false"]
  call -->|"engine error twice running"| finish
  done --> work
  failed --> work
  work -->|"none left"| finish["status Done, result summary"]
  start -.->|"abort"| cancelled["status Failed, error cancelled"]
```

- **Start.** `import` returns `Conflict("an import is already running for this book")` if `Inner.running` already has the book. Otherwise it creates `JobRecord { id: "tbj_{hex16}_{n}", chat_id: "", kind: Textbook, status: Running, delivery: Claimed, created_at: now, result: Some({title, bookId, progress}), usage: vec![] }`, where `n` is the count of this book's earlier jobs in `stores.jobs`. It writes the record with `stores.jobs.create_job`, calls `notify`, stores a `CancellationToken` in `Inner.running`, `tokio::spawn`s the runner, and returns the id. `chapters: Some(list)` is deduplicated (each index once, in order) and is only valid for indices that exist and are neither `Ready` nor `Skipped` (`Invalid`); a `Failed` chapter whose retry is spent is `Conflict("chapter_retry_used")`. With `None`, every pending chapter is listed. Before the job is created, `import` refuses with `Conflict("import_rate_limited")` when the hour is already full.
- **Progress.** `result` is `{"title":"Import <book title>","bookId":…,"progress":{"phase":"pages"|"map"|"objectives","pagesRead":u32,"totalPages":u32,"chaptersDone":u32,"chaptersTotal":u32}}`. It is written with `update_job` and then `notify` at most once per 10 pages and once per chapter. `pagesRead` and `totalPages` use the same keys `progress_field` in `chat/runtime.rs` already reads, so `JobItemDto.pages_read` and `total_pages` fill with no change.
- **Pages.** Only the pages of the chapter being imported are read, with `page_texts` in 10-page windows (`MAX_PDF_PAGES_PER_READ`), each in `spawn_blocking`, checking the cancel token between windows: a 25-page chapter is 3 extractor spawns, never one per page. Each page goes through `sanitize_page`. A chapter whose pages are all blank ends the job `Done` with `result.error = "No text found. Scanned pages need OCR before import."`, no call.
- **Map.** Done at upload (6.1), never in the job.
- **Chapters.** Chapters run one at a time, and each chapter's state is re-read from the ledger right before its work: only `Pending`, or `Failed` with `retryable`, runs; a repeat, a `Skipped` chapter or one another run finished is passed over. Chapters under `MIN_CHAPTER_WORDS` get `ChapterSkipped` with no call. For the rest, `split_parts` runs, then one `chapter_objectives` call per part on `engines("placer")`, a fresh engine per call so `usage()` deltas are exact, read before and after as `CourseService::ask` does. Each call is bounded by `tokio::time::timeout(120 s)` and `select!` on the cancel token. After every call, `Usage { calls: 1, … }` is appended **whether the call succeeded or not**, because a call counts once it was sent. The parts' results are merged through `accept_objectives`, numbered across parts. Fewer than `MIN_KEPT_OBJECTIVES` counts as a parse failure. On success, `ChapterDone` is appended, then cards are upserted (6.4). A crash between the two is repaired by 6.4's reconcile, not by re-calling the model.
- **Retry.** A `CourseAgentError::Parse` (or a timeout) retries the whole chapter once (`MAX_ATTEMPTS`). The second failure appends `ChapterFailed { retryable: false }`: the one try and one retry are spent and the chapter is not offered again. A `CourseAgentError::Engine(_)` appends `ChapterFailed { reason: "engine_unavailable", retryable }` with `retryable` true when it hit the first attempt and false when it hit the retry; two engine errors in a row end the job early, with `result.stopped = "engine_unavailable"`, and the remaining chapters stay pending.
- **Cap.** Before each chapter that would call a model, the job re-reads the ledger and counts the book's imports in the last hour (`hour_window`, server clock). At 8 the job ends with `result.stopped = "rate_limited"` and the remaining chapters stay pending, never half-written (`ChapterDone` is only appended with all parts). The job also tracks its own calls: if the chapter's worst case (parts × `MAX_ATTEMPTS`) would pass `MAX_JOB_CALLS`, it ends with `result.stopped = "call_cap"`. The usage route shows `hourUsed`, `hourCap: 8` and `nextSlotAt`.
- **End.** `status: Done`, with `result.summary = {ready, failed, skipped, pending, calls}` and `result.error` set only when the job could not run at all (PDF unreadable, store failure). The record is written, `notify` called, `Inner.running` cleared and the `BusyGuard` dropped.
- **Cancel.** `abort(job_id)` cancels the token and returns true. The runner notices at the next window or call, appends nothing more, and sets `status: Failed`, `result.error = "cancelled"`. Chapters already `Ready` stay. The partial call that was in flight still has its `Usage` appended if a reply came back.
- **Recover.** `recover()` is a public method, not part of `new`, because the service is built lazily on the first textbook route and a restart would otherwise leave a stale `Running` job in `GET …/jobs`. The server calls `textbook_service(…).recover()` when a workspace is opened (the same place the workbench recovers), and `routes/jobs.rs` list builds the service too. Every `JobKind::Textbook` record still `Running` in `stores.jobs.all_report()` becomes `Done` with `result.error = "interrupted by a restart"`, and `notify` is called. The ledger is untouched; `POST …/import` resumes from `pending_chapters`.

### 6.3 Reading check

1. `start_check(book, chapter)`: the chapter must be `Ready`, else `Conflict("chapter_not_ready")`. Read the due cards with `review::due_cards(tracker, now)` filtered to the `t:{book}:` prefix. `review::history(tracker, card_id)` (`review.rs:227`) takes one card id and no `now`, so do not call it per card: take the reviewed set from `tracker.reviews()` once, collecting the `card_id`s that start with `t:{book}:`. Call `select_check` with `seed = check id`, where the check id is `rc_` + `uuid::Uuid::new_v4().simple()`. Append `CheckIssued`. **No engine is touched.**
2. `answer(book, check, item, …)`: the check must exist in the fold and contain `item` (else `NotFound`). An `(check, item)` pair already answered gives `Conflict("already_answered")`. Grade with the course's `grade(item, text)`. Rating as in `CourseService::answer_sync`: `rating_for(correct, confidence)` for an MCQ or a correct recall, and none for a close or wrong recall until `self_grade`. Then `review::grade(tracker, device, card_id, rating, now)`; `StoreError::NotFound` (an item with no card) is ignored, as `grade_card` does. Append `Answered`. Return `CheckFeedback`, with `due` the card's new due date. Text limits are the course ones (`MAX_TEXT_CHARS`, confidence 1 to 5).
3. `self_grade`: only for an answered recall item with no rating yet. Uses `rating_from_grade`, then `review::grade`.

### 6.4 Cards

Card creation is idempotent and reconciled in three places: after each `ChapterDone`, in `start_check` for the chapter being checked, and in `recover()` for every `Ready` chapter. A crash after `ChapterDone` but before the upsert therefore never leaves a Ready chapter without cards, so `grade_card` never meets `NotFound` for a textbook item. Every item gets a `FlashCard` with id `t:{book}:{item}`, `card: Card::new(PARKED_DUE)`, inserted only if `tracker.card(id)` is `None`, as `save_course` does. To avoid a second copy of the card builder, refactor `flash_card` in `crates/lp-service/src/course/mod.rs` into `pub(crate) fn card_for(id: &str, item: &Item, other_answers: &[&str]) -> Option<FlashCard>` (MCQ keeps its options; recall takes its answer plus 3 distinct others, or gets no card). `flash_card` becomes a thin caller, `textbook.rs` calls `card_for` with the chapter's other answers, and `PARKED_DUE` and `shuffled` become `pub(crate)`. Course behaviour must not change: the existing course tests stay green, untouched.

### 6.5 Errors and cost guarantees

| Case | Service result |
|---|---|
| Unknown or invalid book id, chapter index, check or item | `NotFound` |
| Upload is not a readable PDF (`page_count` fails, encrypted, 0 pages) | `Invalid(extractor message)`, book dir removed |
| `import` while one runs | `Conflict` |
| `import` with no pending chapters | `Conflict("nothing to import")` |
| `import` when 8 chapters were imported in the last hour | `Conflict("import_rate_limited")`, no call |
| `import` of a `Failed` chapter whose try and retry are spent | `Conflict("chapter_retry_used")`, no call |
| `import` of a `Skipped` chapter, or a chapter index listed twice | `Invalid` for `Skipped`; duplicates are collapsed to one |
| Reading check on a chapter that is not `Ready` | `Conflict("chapter_not_ready")` |
| Answer twice | `Conflict("already_answered")` |
| Store failure | `Store` |
| `page_texts` fails mid-import (decompress failure, extractor timeout, output cap) | The job ends `Done` with `result.error` = the extractor message (never page text); already `Ready` chapters stay; no further calls; the pages already read are dropped. The chapter in flight is not marked failed. |
| A chapter's items yield no cards (a recall item without enough distractors for `card_for`) | The item stays in the objective and the check, but has no card. If every item of a `Ready` chapter has no card, it is `ChapterFailed { retryable: false, reason: "no_cards" }` instead of `ChapterDone`, so `chapter_status` is never stuck. A chapter with at least one card can reach `Due`. |
| TOC yields fewer than 2 chapters | Fall back to `chunk_chapters` (6.2 Map). |
| `book()` of a directory with `source.pdf` but no `Imported` line (crash during upload) | `NotFound`. `list()` omits it, and the next upload of the same bytes finds no ledger, overwrites the source and proceeds. |
| Import of an already `Ready` chapter | Rejected as `Invalid`. Re-importing or refreshing a `Ready` chapter is out of scope for U7. |
| Every page of a chapter blank (scanned book with an outline) | the job ends `Done`, `error = "No text found. Scanned pages need OCR before import."`, no model call; with no outline the upload itself is refused (6.1) |

Cost: `list`, `book`, `chapter`, `start_check`, `answer`, `self_grade` and `usage` never call `engines`. The only call sites are the TOC call inside upload (0 with an outline, 1 without) and `chapter_objectives` inside the runner, which only runs for a chapter the learner opened. Upload makes 0 chapter calls (`upload_makes_zero_chapter_calls_and_starts_no_job`). Textbook usage lives in `textbook.jsonl`, and `CourseService::strong_stamps` only reads `courses/*/events.jsonl`, so the import never touches the G4 strong cap.

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
| `POST /workspaces/{ws}/sources/textbook` (in `sources.rs`) | multipart, one field `file` (`.pdf`, magic `%PDF-`, at most `MAX_PDF_BYTES`); no `question` field | `201 TextbookUploadDto` for a new, mapped book (no job started); `200 TextbookUploadDto` with `existing: true` | `400 validation` (bad form, name, magic, unreadable PDF); `413` with `details.maxBytes` (same path as `pdf`); `404` workspace; `409` archived workspace |
| `GET /workspaces/{ws}/textbooks` | – | `200 TextbookListDto` | `404` |
| `GET /workspaces/{ws}/textbooks/{book}` | – | `200 TextbookDto` | `404` |
| `GET /workspaces/{ws}/textbooks/{book}/chapters/{n}` | – | `200 ChapterDetailDto` | `404` |
| `POST /workspaces/{ws}/textbooks/{book}/import` | `ImportRequest { chapters?: u32[] }` (at most 200) | `202 ImportStartedDto` | `400`, `404`, `409 import_running`, `409 nothing_to_import`, `409 import_rate_limited`, `409 chapter_retry_used` |
| `POST /workspaces/{ws}/textbooks/{book}/chapters/{n}/check` | – | `200 ReadingCheckDto` | `404`, `409 chapter_not_ready` |
| `POST /workspaces/{ws}/textbooks/{book}/checks/{check}/answer` | `CheckAnswerRequest { item, text, confidence?, ms?, clientId? }` | `200 CheckFeedbackDto` | `400`, `404`, `409 already_answered` |
| `POST /workspaces/{ws}/textbooks/{book}/checks/{check}/self-grade` | `SelfGradeRequest` (reuse the course DTO) | `204` | `400`, `404`, `409` |
| `GET /workspaces/{ws}/textbooks/{book}/usage` | – | `200 TextbookUsageDto` | `404` |
| (existing) `GET /workspaces/{ws}/jobs`, `POST …/jobs/{id}/abort`, SSE `job.updated` | – | textbook jobs appear here | as today |

A retry with the same `clientId` on `answer` returns the stored feedback, not `409`: the service keeps the client id in `Answered.client_id` and replays the match.

### 7.3 DTOs (`crates/lp-server/src/dto/textbook.rs`, new)

All `#[derive(Serialize, ToSchema)]` (requests `Deserialize`), `#[serde(rename_all = "camelCase")]`, with `From<service struct>` impls and a round-trip unit test each, as in `docs/web/03-server-api.md` §DTO rule.

```
TextbookUploadDto   { bookId, existing: bool, bytes: u64 }
TextbookListDto     { items: [TextbookSummaryDto] }
TextbookSummaryDto  { id, title, totalPages, chapters, ready, state: "importing"|"ready"|"partial"|"failed", createdAt }
TextbookDto         { id, title, fileName, totalPages, pageOffset: i32|null, mapSource: "outline"|"toc"|"chunks"|null,
                      jobId: string|null, autoCalls, requestedCalls, hourUsed, hourCap: 8,
                      chapters: [ChapterRowDto] }
ChapterRowDto       { index, title, startPage, endPage, kind: "chapter"|"front"|"back",
                      state: "pending"|"importing"|"ready"|"failed"|"skipped",
                      status: "not_read"|"checked"|"due", objectives: u32, error: string|null, truncated: bool,
                      canImport: bool }
ChapterDetailDto    { chapter: ChapterRowDto, objectives: [{ id, text, startPage, endPage, items: u32 }] }
ImportRequest       { chapters?: [u32] }
ImportStartedDto    { jobId }
ReadingCheckDto     { checkId, chapter, items: [{ id, objectiveId, kind: "mcq"|"recall", q, options: [string] }] }
CheckAnswerRequest  { item, text, confidence?: 1..5, ms?: u64, clientId?: string }
CheckFeedbackDto    { correct, close, answer, why, due: i64|null }
TextbookUsageDto    { cheapIn, cheapOut, autoCalls, requestedCalls, hourUsed, hourCap, nextSlotAt: i64|null }
```

**Plain-text rule:** `title`, `error`, `q`, `options`, `answer`, `why` and objective `text` are untrusted. Clients render them as plain text only (section 8).

**Leak rule:** `ReadingCheckDto` and `ChapterDetailDto` never contain `answer`, `accept`, `why`, `mustName`, `misconceptions` or `correct`. A test serializes both to JSON and asserts none of those keys appear at any depth.

Regenerate `web/openapi/openapi.json` with `learnpuppy openapi`, and its client with `web/scripts/gen-api.ts`. Never hand-merge them (U7 lands after U3, U4 and U5: rebase first).

## 8. Safety checklist

- Upload: the 50 MB cap, the `.pdf` name and the `%PDF-` magic are all enforced by the reused `form` in `sources.rs` (make `form`, `boundary`, `too_large` and `bad` `pub(super)`; `form` takes a mode enum, `Pdf | Note | Textbook`, in place of `note: bool`, and `Textbook` rejects a `question` field).
- Path safety: book ids are hex only; `source_path` is built by the store; `pdf_path` already prefixes `./` to a leading `-`.
- Extracted text: `sanitize_page`, then the per-page and per-part caps, then `redact`, then the fence. No book text ever reaches a log line (`tracing` fields carry ids and counts only).
- Subprocesses: only `pdfinfo`, `pdftotext` and `pdftohtml`, through `pdf_command` (null stdin, stderr dropped, timeout, stall guard, output cap, kill on exit).
- Untrusted output: chapter titles (TOC call, outline) and `error` strings come from the PDF or the model, and so do `q`, `options`, `answer`, `why` and objective text. The backend serves them as JSON data, never as HTML, and adds no markup. U8 must render every one of these as plain text (React text children, no `dangerouslySetInnerHTML`, no markdown or HTML rendering of these fields); this is an acceptance item in U8. `error` strings carry extractor or engine messages only, never page text. Titles and objective text are passed through `sanitize_page` at accept time and cut to their length caps.
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
10b. `fold_created_at_is_first_imported_ts`
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
6. `textbook_list_omits_dir_without_imported_line`

**lp-agent**
1. `parse_outline_xml_nested_and_entities` (adapters.rs)
2. `pdftohtml_outline_real_fixture` (skips if Poppler is missing, as `real_pdf_reads_by_range` does)
3. `page_texts_keeps_blank_pages`: an image-only page is an empty string, not an error.
3b. `page_texts_pads_trailing_blank_page`: an image-only last page still gives `last-first+1` strings and later pages keep their numbers.
4. `pdf_input_cap_is_50_mib`
5. `chapter_objectives_fences_and_redacts`: the prompt seen by a fake engine has the injection sentence and a fake `api_key=…` inside the fence, and the key is masked.
6. `chapter_objectives_rejects_empty_and_bad_json`
7. `toc_chapters_parses_and_validates`

**lp-service (`crates/lp-service/src/textbook.rs` tests, with a counting fake engine like `Mock` in `course/tests.rs` and a fake `Pdf`)**
1. `upload_makes_zero_chapter_calls_and_starts_no_job`: call counter 0 with an outline, ledger holds only `Imported` and `Mapped`, no job.
1b. `upload_never_imports_a_60_chapter_book`
1c. `opening_one_chapter_imports_only_that_chapter`
1d. `import_reads_only_the_chapter_in_10_page_windows`: a fake `Pdf` records `page_texts` spawns, 25 pages are 3 reads and nothing at upload.
2. `no_outline_makes_exactly_one_toc_call_at_upload`
3. `one_call_per_chapter_and_split_parts_counted`: two parts are one import slot.
4. `hour_limit_is_8_chapter_imports_per_rolling_hour`: 8 imports, the job stops with `rate_limited`, a refused import makes 0 calls, 59 minutes later it is still refused, past the hour it works; `the_hour_limit_is_checked_between_chapters_of_one_job`.
5. `chapter_gets_one_try_plus_one_retry_then_it_is_final`: 2 calls, `chapter_retry_used` after; `retry_is_part_of_the_one_import_and_does_not_use_a_slot`.
5b. `duplicate_chapter_indices_call_the_model_once`, `skipped_chapters_are_rejected_and_never_called`, `one_job_cannot_exceed_its_call_cap`.
6. `unmapped_book_is_mapped_by_its_first_import`
7. `two_engine_errors_stop_job_early_and_stay_retryable`
8. `same_pdf_twice_no_duplicate_objectives_or_cards`: card count unchanged, one `Imported` line.
9. `concurrent_identical_uploads_one_book`
10. `cancel_marks_failed_and_keeps_ready_chapters`
10b. `page_texts_failure_mid_import_ends_job_with_error`
10c. `chapter_with_no_cards_is_failed_not_stuck`
10d. `toc_under_two_chapters_falls_back_to_chunks`
10e. `crash_between_chapter_done_and_cards_is_reconciled`
11. `restart_recovers_running_job_and_resume_imports_rest`: recovery happens at workspace open, before any textbook route is called.
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
6b. `titles_and_errors_are_served_as_plain_json_strings`: a title like `<img src=x onerror=1>` comes back byte-identical in a JSON string, content type `application/json`.
6c. `jobs_list_after_restart_shows_no_running_textbook_job`
7. `check_answer_idempotent_client_id`
8. `chapter_not_ready_is_409_with_reason`
9. `health_under_200ms_during_400_page_import`: a synthetic 400-page PDF, generated like `multi_page_pdf` in `adapters.rs`, with `GET /health` polled while the job runs. Marked `#[ignore]` only if Poppler is absent.
10. Round-trip tests for every DTO in `dto/textbook.rs`, plus the OpenAPI drift check (`make api-check`).

**Fixture generation.** `crates/lp-service/tests/fixtures/make_textbook_pdf.py` (new; Python standard library only, our own text, so license-clean) writes `textbook.pdf`, which is committed, under 64 kB. The two-column page uses two text objects at different x positions. The image page holds a tiny inline image and no text.

**Manual.** `scripts/real_smoke.py` gains a `--textbook <pdf>` mode: upload, poll `GET …/jobs` until done, print the chapters and two objectives per chapter, and assert `autoCalls <= 40`. It is not in CI.

## 10. Build order (file by file)

Each step compiles and passes `make gate` on its own, including `api-check` (`git diff --exit-code` on `web/openapi/openapi.json`). Any step that changes an OpenAPI schema or path (steps 7 and 8) regenerates `web/openapi/openapi.json` and the TypeScript client in the same commit (`learnpuppy openapi`, then `web/scripts/gen-api.ts`; never hand-merged). Commit per step, or squash at the end.

1. `crates/lp-core/src/textbook.rs` (new) and `lib.rs`: types (including `RawObjective`), constants, functions, lp-core tests and proptests.
2. `crates/lp-core/src/ports.rs`: `JobKind::Textbook`. Fix every match: `crates/lp-agent/src/workbench/mod.rs` (the name match at line 128, the dispatch match in `run`, and `recover` skipping Textbook in both loops), `crates/lp-service/src/chat/jobs.rs` (`kind_name`, `title_of`, `tool_kind`), `crates/lp-server/src/seed.rs`. Grep `JobKind::` again. If the enum is exposed in a DTO or schema, regenerate OpenAPI here. Add `workbench_recover_skips_textbook_jobs` in `crates/lp-agent/src/workbench/tests.rs`.
3. `crates/lp-store/src/textbooks.rs` (new), `lib.rs`, store tests. Then `crates/lp-service/src/stores.rs`: the `textbooks` field.
4. `crates/lp-agent/src/workbench/jobs.rs` (trait `Pdf`: `page_texts`, `outline`) and `adapters.rs` (`PdftotextPdf` impls, `parse_outline_xml`, the 50 MiB cap). Then `crates/lp-agent/src/course/textbook.rs` (new), `course/mod.rs` (`pub mod textbook;`), and `prompts/textbook_toc.md` and `prompts/textbook_objectives.md` (new).
5. `crates/lp-service/src/course/mod.rs`: the `card_for` refactor, and `PARKED_DUE`, `shuffled` and `grade` made `pub(crate)`. Course tests must stay unchanged and green.
6. Add the `uuid` and `bytes` dependencies (section 6). `crates/lp-service/src/textbook.rs` (new) and `lib.rs`. `crates/lp-service/src/chat/runtime.rs`: `job_info` made `pub`, and `JobView` gains `book_id`, `chapters_done` and `chapters_total` (plain structs, so no OpenAPI change yet). Service tests and the fixture (`crates/lp-service/tests/fixtures/`).
7. `crates/lp-server/src/dto/textbook.rs` (new) and `dto/mod.rs`; `dto/chat.rs` `JobItemDto` gets the three new fields. Register the new schemas in `openapi.rs`, then regenerate `web/openapi/openapi.json` and the client in this same commit.
8. `crates/lp-server/src/app.rs` (`textbook_service`, `role_engines`, recover at workspace open), `routes/sources.rs` (form mode enum, `POST …/sources/textbook`), `routes/textbook.rs` (new), `routes/mod.rs`, `routes/jobs.rs` (abort routing, list builds the service), `openapi.rs` paths. Route tests in `crates/lp-server/tests/api.rs`. Regenerate OpenAPI and the client in this same commit.
9. Run `make gate` and `make ci` end to end on the final tree, and confirm `git diff --exit-code web/openapi/openapi.json` is clean.
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
| Upload makes 0 chapter calls; a chapter imports when opened; one try plus one retry; 8 imports per book per hour | service tests 1, 1c, 4, 5 and 5b, usage route, server test `textbook_import_is_limited_to_8_chapters_per_hour` |

## 12. Review log

Two independent reviews (Reviewer 1: claude, Reviewer 2: agy) returned REVISE. D7 was closed by the owner's reviewer as a hard lifetime cap. Code claims were re-checked with grep on this worktree. The `OKF`-style claim sets below could not be verified beyond the cited paths, and where a reviewer asserted an outside fact it is marked.

| Finding | Resolution |
|---|---|
| R1.1 and R2.5 cost cap, fresh 40 per request | Accepted. D7 rewritten as a lifetime cap counted from ledger `Usage` totals; section 6.2 Cap rewritten; tests 5 and 5b. |
| R1.2 retry can pass the cap | Accepted. A retry re-checks the cap; test 5b. |
| R2.5 multi-part chapter failing on its last part cannot retry | Accepted. A chapter starts only if all its parts fit; a part that cannot be retried abandons the whole chapter, nothing half-written, `reason: budget`. |
| R1.3 stale Running job after restart | Accepted. `recover()` is called at workspace open and by the jobs list, not from lazy `new`. Test 11 and server test 6c. |
| R1.4 crash between `ChapterDone` and card upsert | Accepted. Reconcile in 6.4 (after done, in `start_check`, in `recover`); test 10e. |
| R1.5 `page_texts` count and trailing blank page | Verified: `page_range` pops a trailing blank page (`adapters.rs:391`). Padding rule and test 3b added. |
| R1.6 missing `uuid` and `bytes` deps | Verified by grep: `lp-service` has neither. Added in step 6. The other items R1 listed as checked were not re-verified one by one. |
| R1.7 directory without `Imported` line | Accepted. `book()` gives `NotFound`, `list()` omits it; store test 6. |
| R1.8 untrusted `error` and titles | Accepted. Plain-text rule in sections 7.3 and 8, with a U8 acceptance duty and server test 6b. |
| R1.9 security chain | No change needed. |
| R1.10 `Ready` chapter re-import | Accepted as a note: out of scope for U7 (section 6.5). |
| R2.1 build order breaks `api-check` | Verified in `Makefile` (`gate` runs `api-check`). Section 10 rewritten: OpenAPI regeneration is in steps 7 and 8, not a later step. |
| R2.2 `workbench/mod.rs:128` match | Verified by grep (`let name = match job.kind`). Added to section 5 and step 2. |
| R2.2 `RawObjective` missing from 3.2 | Accepted; added. |
| R2.3 `review::history` signature | Verified (`review.rs:227`, `(tracker, card_id)`). 6.3 now uses `tracker.reviews()` once. |
| R2.4 `createdAt` dropped in fold | Accepted. `BookState.created_at` from the first `Imported.ts`; fold test 10b. |
| R2.6 stored XSS claim from untrusted PDF text | Partly accepted. The backend serves JSON and never HTML, so it cannot execute script itself; the risk is on the U8 render path, so the rule is plain-text rendering, enforced by U8 and a backend test of byte-identical strings. Sanitising or escaping content on the server was rejected: it would corrupt stored text and double-escape in React. |
| R2.7 `page_texts` failing mid-import | Accepted. Failure row in 6.5, test 10b. |
| R2.7 items without cards, chapter never `Due` | Accepted. A chapter with no cards at all fails as `no_cards`; test 10c. |
| R2.7 TOC under 2 chapters | Accepted. Falls back to `chunk_chapters`; test 10d. |

### R4 (lazy import) changes

D7 changed from a lifetime 40 per book to a per-chapter try plus one retry, 8 manual chapter imports per book per rolling hour (server clock, counted from ledger `Usage` lines), and a 48-call backstop per job. Upload no longer starts a job: `Upload.job`, `TextbookUploadDto.jobId`, `ChapterRowState::Queued` and `callsCap` are gone; `ChapterRowDto.canImport`, `hourUsed`, `hourCap` and `nextSlotAt` are new, and a new upload returns `201`. `pdftotext` was already one spawn per 10-page window; a test now pins it. `chapters_from_outline` now picks leaf chapters under `Part` containers. A security review found that a repeated or `Skipped` chapter index could re-run calls past the hourly cap; the list is now deduplicated, `Skipped` is rejected, and the job re-checks each chapter's state before its call.
