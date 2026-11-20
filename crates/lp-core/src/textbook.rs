//! Textbook mode domain (U7): chapter maps, stored objectives, the ledger fold and reading checks.
//!
//! Spec: `brain/learning-method/textbook-spec.md`. No IO, no clock, no RNG: `now` and seeds are
//! arguments. Extracted PDF text is untrusted, so everything here sanitises before it is kept.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::course::{gate_item, Item, Level};
use crate::fsrs::Rating;

// ------------------------------------------------------------------ constants

/// Manual chapter imports per book in any rolling `IMPORT_WINDOW_MS` (D7, R4). A chapter import
/// is one `(job, chapter)` that sent at least one model call.
pub const MAX_MANUAL_IMPORTS_PER_HOUR: u32 = 8;
/// The rolling window of [`MAX_MANUAL_IMPORTS_PER_HOUR`], in epoch milliseconds.
pub const IMPORT_WINDOW_MS: i64 = 3_600_000;
pub const PART_WORDS: usize = 8_000;
pub const MAX_PARTS: usize = 3;
pub const MAX_PART_WORDS: usize = 10_000;
pub const MIN_CHAPTER_WORDS: usize = 300;
pub const MIN_OBJECTIVES: usize = 5;
pub const MAX_OBJECTIVES: usize = 12;
pub const MIN_KEPT_OBJECTIVES: usize = 3;
pub const MAX_ITEMS_PER_OBJECTIVE: usize = 3;
pub const MAX_ATTEMPTS: u32 = 2;
pub const MAX_CHAPTERS: usize = 200;
pub const TOC_PAGES: usize = 20;
pub const TOC_CHARS: usize = 24_000;
pub const CHUNK_PAGES: u32 = 20;
pub const CHECK_ITEMS: usize = 8;
pub const MAX_PAGE_CHARS: usize = 20_000;

const MAX_TITLE_CHARS: usize = 200;
const MAX_OBJECTIVE_CHARS: usize = 300;

// ------------------------------------------------------------------ types

/// Physical, 1-based, inclusive.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageRange {
    pub start: u32,
    pub end: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutlineEntry {
    pub title: String,
    pub page: u32,
    pub depth: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChapterKind {
    Chapter,
    Front,
    Back,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Chapter {
    /// 0-based, dense, in page order.
    pub index: u32,
    pub title: String,
    pub pages: PageRange,
    pub kind: ChapterKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MapSource {
    Outline,
    Toc,
    Chunks,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Objective {
    /// `ch{index:03}-o{n:02}`, n from 1.
    pub id: String,
    pub text: String,
    pub pages: PageRange,
    pub items: Vec<Item>,
}

/// Parsed model reply, before the gate.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RawObjective {
    pub text: String,
    pub start_page: u32,
    pub end_page: u32,
    pub items: Vec<Item>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BookMeta {
    /// `tb_` plus 16 lowercase hex.
    pub id: String,
    pub title: String,
    pub file_name: String,
    pub bytes: u64,
    pub sha256: String,
    pub total_pages: u32,
    /// Physical minus printed; `None` if not found.
    #[serde(default)]
    pub page_offset: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TextbookEvent {
    Imported {
        ts: i64,
        book: BookMeta,
    },
    Mapped {
        ts: i64,
        source: MapSource,
        chapters: Vec<Chapter>,
    },
    ChapterDone {
        ts: i64,
        chapter: u32,
        parts: u8,
        truncated: bool,
        objectives: Vec<Objective>,
    },
    ChapterFailed {
        ts: i64,
        chapter: u32,
        reason: String,
        retryable: bool,
    },
    /// Too short, or front/back matter.
    ChapterSkipped {
        ts: i64,
        chapter: u32,
        reason: String,
    },
    Usage {
        ts: i64,
        job: String,
        chapter: Option<u32>,
        role: String,
        tokens_in: u64,
        tokens_out: u64,
        calls: u32,
        requested: bool,
    },
    CheckIssued {
        ts: i64,
        check: String,
        chapter: u32,
        items: Vec<String>,
    },
    Answered {
        ts: i64,
        check: String,
        item: String,
        correct: bool,
        close: bool,
        rating: Option<Rating>,
        #[serde(default)]
        client_id: Option<String>,
    },
}

/// Derived, never stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ChapterState {
    Pending,
    Ready,
    Failed,
    Skipped,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ChapterView {
    pub chapter: Chapter,
    pub state: ChapterState,
    pub objectives: Vec<Objective>,
    pub error: Option<String>,
    pub truncated: bool,
    /// At least one `Answered` for this chapter.
    pub checked: bool,
    /// Only meaningful while `state == Failed`. (Added to the spec shape: `pending_chapters`
    /// needs it and the fold is the only place that sees the ledger.)
    pub retryable: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct BookState {
    pub meta: Option<BookMeta>,
    /// Ts of the first `Imported` line.
    pub created_at: Option<i64>,
    pub source: Option<MapSource>,
    pub chapters: Vec<ChapterView>,
    pub auto_calls: u32,
    pub requested_calls: u32,
    pub cheap_in: u64,
    pub cheap_out: u64,
    /// Check id, chapter, item ids.
    pub checks: Vec<(String, u32, Vec<String>)>,
    /// `(check, item)` already answered.
    pub answered: Vec<(String, String)>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChapterStatus {
    NotRead,
    Checked,
    Due,
}

// ------------------------------------------------------------------ text

fn is_stripped(c: char) -> bool {
    if c == '\n' || c == '\t' {
        return false;
    }
    c.is_control()
        || c == '\u{FEFF}'
        || ('\u{200B}'..='\u{200F}').contains(&c)
        || ('\u{202A}'..='\u{202E}').contains(&c)
        || ('\u{2066}'..='\u{2069}').contains(&c)
}

/// Strip control and bidi characters, collapse long blank runs, cap the length.
pub fn sanitize_page(text: &str) -> String {
    let cleaned: String = text.chars().filter(|c| !is_stripped(*c)).collect();
    let mut out = String::with_capacity(cleaned.len().min(MAX_PAGE_CHARS + 4));
    let mut blank_run = 0usize;
    let mut first = true;
    for line in cleaned.split('\n') {
        if line.trim().is_empty() {
            blank_run += 1;
            if blank_run > 2 {
                continue;
            }
        } else {
            blank_run = 0;
        }
        if !first {
            out.push('\n');
        }
        first = false;
        out.push_str(line);
    }
    match out.char_indices().nth(MAX_PAGE_CHARS) {
        Some((i, _)) => {
            out.truncate(i);
            out
        }
        None => out,
    }
}

pub fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

fn cut_chars(s: &str, max: usize) -> String {
    s.chars().take(max).collect()
}

fn clean_title(s: &str) -> String {
    let flat: String = sanitize_page(s)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    cut_chars(&flat, MAX_TITLE_CHARS)
}

// ------------------------------------------------------------------ ids

/// The caller passes lowercase hex.
pub fn book_id(sha256_hex: &str) -> String {
    format!("tb_{}", cut_chars(sha256_hex, 16))
}

pub fn valid_book_id(id: &str) -> bool {
    match id.strip_prefix("tb_") {
        Some(h) => {
            h.len() == 16
                && h.bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        }
        None => false,
    }
}

pub fn card_id(book: &str, item: &str) -> String {
    format!("t:{book}:{item}")
}

// ------------------------------------------------------------------ chapter map

fn printed_number(page: &str) -> Option<i32> {
    let mut lines = page.lines().map(str::trim).filter(|l| !l.is_empty());
    let first = lines.next();
    let last = lines.next_back().or(first);
    [first, last].into_iter().flatten().find_map(|l| {
        (!l.is_empty() && l.len() <= 4 && l.bytes().all(|b| b.is_ascii_digit()))
            .then(|| l.parse::<i32>().ok())
            .flatten()
    })
}

/// Physical minus printed page number, from the first page whose offset the next two numbered
/// pages repeat. Roman numerals are never matched.
pub fn printed_offset(pages: &[String]) -> Option<i32> {
    let numbered: Vec<i32> = pages
        .iter()
        .enumerate()
        .filter_map(|(i, p)| printed_number(p).map(|n| (i as i32 + 1) - n))
        .collect();
    // `numbered` holds offsets of the pages that carry a number, in page order.
    numbered
        .windows(3)
        .find(|w| w[0] == w[1] && w[1] == w[2])
        .map(|w| w[0])
}

pub fn kind_of(title: &str) -> ChapterKind {
    const FRONT: [&str; 9] = [
        "contents",
        "table of contents",
        "copyright",
        "preface",
        "foreword",
        "dedication",
        "acknowledgments",
        "acknowledgements",
        "about this book",
    ];
    const BACK: [&str; 7] = [
        "index",
        "colophon",
        "about the author",
        "about the authors",
        "bibliography",
        "references",
        "glossary",
    ];
    let t = title.trim().to_lowercase();
    if t == "title page" || FRONT.contains(&t.as_str()) {
        ChapterKind::Front
    } else if BACK.contains(&t.as_str()) {
        ChapterKind::Back
    } else {
        ChapterKind::Chapter
    }
}

/// Sort by page, merge shared start pages, cut, then derive end pages and kinds.
fn finish_chapters(mut starts: Vec<(String, u32)>, total: u32) -> Vec<Chapter> {
    starts.retain(|(_, p)| *p >= 1 && *p <= total);
    starts.sort_by_key(|(_, p)| *p); // stable
    let mut merged: Vec<(Vec<String>, u32)> = Vec::new();
    for (title, page) in starts {
        match merged.last_mut() {
            Some((titles, p)) if *p == page => titles.push(title),
            _ => merged.push((vec![title], page)),
        }
    }
    let n = merged.len().min(MAX_CHAPTERS);
    // End pages come from the full list; truncating first would let the last kept
    // chapter run to the end of the PDF.
    (0..n)
        .map(|i| {
            let (titles, start) = &merged[i];
            let end = merged.get(i + 1).map_or(total, |(_, next)| next - 1);
            let kinds: Vec<ChapterKind> = titles.iter().map(|t| kind_of(t)).collect();
            let kind = if kinds.iter().all(|k| *k == kinds[0]) {
                kinds[0]
            } else {
                ChapterKind::Chapter
            };
            Chapter {
                index: i as u32,
                title: clean_title(&titles.join(" / ")),
                pages: PageRange { start: *start, end },
                kind,
            }
        })
        .collect()
}

/// "Part I", "Part 2: Basics", "Book Three": a container that only groups chapters.
fn is_part_container(title: &str) -> bool {
    const WORDS: [&str; 10] = [
        "one", "two", "three", "four", "five", "six", "seven", "eight", "nine", "ten",
    ];
    let t = title.trim().to_lowercase();
    let Some(rest) = ["part", "book"].iter().find_map(|p| t.strip_prefix(p)) else {
        return false;
    };
    if !rest.starts_with(char::is_whitespace) {
        return false;
    }
    let tok: String = rest
        .trim_start()
        .chars()
        .take_while(|c| c.is_alphanumeric())
        .collect();
    !tok.is_empty()
        && (tok.chars().all(|c| c.is_ascii_digit())
            || tok.chars().all(|c| "ivxlc".contains(c))
            || WORDS.contains(&tok.as_str()))
}

pub fn chapters_from_outline(entries: &[OutlineEntry], total: u32) -> Vec<Chapter> {
    let d0 = entries.iter().filter(|e| e.depth == 0).count();
    let d1 = entries.iter().filter(|e| e.depth == 1).count();
    if d0 < 3 && d1 >= 3 {
        let starts = entries
            .iter()
            .filter(|e| e.depth == 1)
            .map(|e| (e.title.clone(), e.page))
            .collect();
        return finish_chapters(starts, total);
    }
    // Leaf chapters under `Part` containers; other top-level entries (Preface, Index) stay.
    let mut starts = Vec::new();
    let mut inside = false;
    let mut parts = 0;
    for (i, e) in entries.iter().enumerate() {
        match e.depth {
            0 if is_part_container(&e.title)
                && entries.get(i + 1).is_some_and(|n| n.depth == 1) =>
            {
                inside = true;
                parts += 1;
            }
            0 => {
                inside = false;
                starts.push((e.title.clone(), e.page));
            }
            1 if inside => starts.push((e.title.clone(), e.page)),
            _ => {}
        }
    }
    if parts == 0 {
        starts = entries
            .iter()
            .filter(|e| e.depth == 0)
            .map(|e| (e.title.clone(), e.page))
            .collect();
    }
    finish_chapters(starts, total)
}

/// First-call time of every distinct `(job, chapter)` that sent a requested model call at or after
/// `since`, oldest first. This is what the hourly import limit counts, straight from the ledger.
pub fn manual_import_times(events: &[TextbookEvent], since: i64) -> Vec<i64> {
    let mut seen: Vec<(&str, u32)> = Vec::new();
    let mut times = Vec::new();
    for ev in events {
        if let TextbookEvent::Usage {
            ts,
            job,
            chapter: Some(ch),
            requested: true,
            ..
        } = ev
        {
            if !seen.contains(&(job.as_str(), *ch)) {
                seen.push((job.as_str(), *ch));
                if *ts >= since {
                    times.push(*ts);
                }
            }
        }
    }
    times.sort_unstable();
    times
}

pub fn chapters_from_toc(
    entries: &[(String, u32)],
    offset: Option<i32>,
    pages: &[String],
    total: u32,
) -> Vec<Chapter> {
    let off = i64::from(offset.unwrap_or(0));
    let mut starts = Vec::new();
    for (title, printed) in entries {
        let mapped = i64::from(*printed) + off;
        if mapped < 1 || mapped > i64::from(total) {
            continue;
        }
        let needle: String = cut_chars(&sanitize_page(title), 30).trim().to_lowercase();
        let lo = (mapped - 3).max(1);
        let hi = (mapped + 3).min(i64::from(total));
        let found = if needle.is_empty() {
            None
        } else {
            (lo..=hi).find(|p| {
                pages
                    .get((*p - 1) as usize)
                    .is_some_and(|t| sanitize_page(t).to_lowercase().contains(&needle))
            })
        };
        starts.push((title.clone(), found.unwrap_or(mapped) as u32));
    }
    finish_chapters(starts, total)
}

pub fn chunk_chapters(total: u32) -> Vec<Chapter> {
    if total == 0 {
        return Vec::new();
    }
    let size = CHUNK_PAGES.max(total.div_ceil(MAX_CHAPTERS as u32));
    let mut out = Vec::new();
    let mut start = 1u32;
    while start <= total {
        let end = (start + size - 1).min(total);
        out.push(Chapter {
            index: out.len() as u32,
            title: format!("Pages {start}\u{2013}{end}"),
            pages: PageRange { start, end },
            kind: ChapterKind::Chapter,
        });
        start = end + 1;
    }
    out
}

// ------------------------------------------------------------------ parts and objectives

/// One entry per page of the chapter, `(page, words)`. Returns the call parts; the bool is true
/// when that part has more than `MAX_PART_WORDS` words, so its text is cut.
pub fn split_parts(page_words: &[(u32, usize)]) -> Vec<(PageRange, bool)> {
    let (Some(first), Some(last)) = (page_words.first(), page_words.last()) else {
        return Vec::new();
    };
    let total: usize = page_words.iter().map(|(_, w)| *w).sum();
    if total < PART_WORDS {
        return vec![(
            PageRange {
                start: first.0,
                end: last.0,
            },
            false,
        )];
    }
    let k = MAX_PARTS.min(total.div_ceil(PART_WORDS));
    let mut parts = Vec::new();
    let (mut start, mut cum, mut group_words) = (first.0, 0usize, 0usize);
    for (i, (page, w)) in page_words.iter().enumerate() {
        cum += w;
        group_words += w;
        let closed = parts.len();
        let more_pages = i + 1 < page_words.len();
        if closed + 1 < k && more_pages && cum * k >= total * (closed + 1) {
            parts.push((
                PageRange { start, end: *page },
                group_words > MAX_PART_WORDS,
            ));
            start = page_words[i + 1].0;
            group_words = 0;
        }
    }
    parts.push((
        PageRange { start, end: last.0 },
        group_words > MAX_PART_WORDS,
    ));
    parts
}

pub fn objective_bounds(parts: usize) -> (usize, usize) {
    let p = parts.max(1);
    (MIN_OBJECTIVES.div_ceil(p), MAX_OBJECTIVES / p)
}

fn clean_item(it: &Item) -> Item {
    let s = |x: &str| sanitize_page(x);
    Item {
        id: String::new(),
        kind: it.kind,
        q: s(&it.q),
        options: it.options.iter().map(|o| s(o)).collect(),
        answer: s(&it.answer),
        accept: it.accept.iter().map(|o| s(o)).collect(),
        why: s(&it.why),
        tests: None,
        level: it.level,
        must_name: it.must_name.iter().map(|o| s(o)).collect(),
        misconceptions: it.misconceptions.iter().map(|o| s(o)).collect(),
    }
}

/// Gate a parsed reply: clamp pages, cut and drop items, number ids from `first_n`.
pub fn accept_objectives(
    chapter: u32,
    first_n: usize,
    part: PageRange,
    raw: Vec<RawObjective>,
) -> Vec<Objective> {
    let mut out: Vec<Objective> = Vec::new();
    for r in raw {
        let text = cut_chars(clean_title(&r.text).as_str(), MAX_OBJECTIVE_CHARS);
        if text.is_empty() {
            continue;
        }
        let pages = if r.start_page > r.end_page {
            part
        } else {
            PageRange {
                start: r.start_page.clamp(part.start, part.end),
                end: r.end_page.clamp(part.start, part.end),
            }
        };
        let n = first_n.max(1) + out.len();
        let oid = format!("ch{chapter:03}-o{n:02}");
        let mut items = Vec::new();
        for it in r.items.iter().take(MAX_ITEMS_PER_OBJECTIVE) {
            let mut item = clean_item(it);
            if gate_item(&item).iter().any(|f| f.level == Level::Drop) {
                continue;
            }
            item.id = format!("{oid}-i{}", items.len() + 1);
            items.push(item);
        }
        if items.is_empty() {
            continue;
        }
        out.push(Objective {
            id: oid,
            text,
            pages,
            items,
        });
    }
    out
}

// ------------------------------------------------------------------ fold

/// Replay the ledger. First `Imported`, first `Mapped` and each chapter's first `ChapterDone` win.
pub fn fold_book(events: &[TextbookEvent]) -> BookState {
    let mut st = BookState::default();
    for ev in events {
        match ev {
            TextbookEvent::Imported { ts, book } => {
                if st.meta.is_none() {
                    st.meta = Some(book.clone());
                    st.created_at = Some(*ts);
                }
            }
            TextbookEvent::Mapped {
                source, chapters, ..
            } => {
                if st.source.is_none() {
                    st.source = Some(*source);
                    st.chapters = chapters
                        .iter()
                        .map(|c| ChapterView {
                            chapter: c.clone(),
                            state: ChapterState::Pending,
                            objectives: Vec::new(),
                            error: None,
                            truncated: false,
                            checked: false,
                            retryable: false,
                        })
                        .collect();
                }
            }
            TextbookEvent::ChapterDone {
                chapter,
                truncated,
                objectives,
                ..
            } => {
                if let Some(v) = view_mut(&mut st, *chapter) {
                    if v.state != ChapterState::Ready {
                        v.state = ChapterState::Ready;
                        v.objectives = objectives.clone();
                        v.truncated = *truncated;
                        v.error = None;
                        v.retryable = false;
                    }
                }
            }
            TextbookEvent::ChapterFailed {
                chapter,
                reason,
                retryable,
                ..
            } => {
                if let Some(v) = view_mut(&mut st, *chapter) {
                    if v.state != ChapterState::Ready {
                        v.state = ChapterState::Failed;
                        v.error = Some(reason.clone());
                        v.retryable = *retryable;
                    }
                }
            }
            TextbookEvent::ChapterSkipped {
                chapter, reason, ..
            } => {
                if let Some(v) = view_mut(&mut st, *chapter) {
                    if v.state != ChapterState::Ready {
                        v.state = ChapterState::Skipped;
                        v.error = Some(reason.clone());
                        v.retryable = false;
                    }
                }
            }
            TextbookEvent::Usage {
                tokens_in,
                tokens_out,
                calls,
                requested,
                ..
            } => {
                if *requested {
                    st.requested_calls = st.requested_calls.saturating_add(*calls);
                } else {
                    st.auto_calls = st.auto_calls.saturating_add(*calls);
                }
                st.cheap_in = st.cheap_in.saturating_add(*tokens_in);
                st.cheap_out = st.cheap_out.saturating_add(*tokens_out);
            }
            TextbookEvent::CheckIssued {
                check,
                chapter,
                items,
                ..
            } => {
                if !st.checks.iter().any(|(c, _, _)| c == check) {
                    st.checks.push((check.clone(), *chapter, items.clone()));
                }
            }
            TextbookEvent::Answered { check, item, .. } => {
                let key = (check.clone(), item.clone());
                if !st.answered.contains(&key) {
                    st.answered.push(key);
                }
                let ch = st
                    .checks
                    .iter()
                    .find(|(c, _, _)| c == check)
                    .map(|(_, ch, _)| *ch);
                if let Some(v) = ch.and_then(|ch| view_mut(&mut st, ch)) {
                    v.checked = true;
                }
            }
        }
    }
    st
}

fn view_mut(st: &mut BookState, chapter: u32) -> Option<&mut ChapterView> {
    st.chapters.get_mut(chapter as usize)
}

/// Chapters in `Pending` or retryable `Failed`, in index order, `kind == Chapter` only.
pub fn pending_chapters(state: &BookState) -> Vec<u32> {
    state
        .chapters
        .iter()
        .filter(|v| {
            v.chapter.kind == ChapterKind::Chapter
                && (v.state == ChapterState::Pending
                    || (v.state == ChapterState::Failed && v.retryable))
        })
        .map(|v| v.chapter.index)
        .collect()
}

// ------------------------------------------------------------------ reading check

fn stable_hash(seed: &str, item: &str) -> u64 {
    let mut h = Sha256::new();
    h.update(seed.as_bytes());
    h.update(item.as_bytes());
    let d = h.finalize();
    let mut b = [0u8; 8];
    b.copy_from_slice(&d[..8]);
    u64::from_be_bytes(b)
}

/// Card ids come in, item ids go out: one item per objective, at most `CHECK_ITEMS`.
pub fn select_check(
    state: &BookState,
    chapter: u32,
    due: &BTreeSet<String>,
    reviewed: &BTreeSet<String>,
    seed: &str,
) -> Vec<String> {
    let (Some(meta), Some(view)) = (&state.meta, state.chapters.get(chapter as usize)) else {
        return Vec::new();
    };
    if view.state != ChapterState::Ready {
        return Vec::new();
    }
    let book = &meta.id;
    let is_due = |i: &Item| due.contains(&card_id(book, &i.id));
    let is_reviewed = |i: &Item| reviewed.contains(&card_id(book, &i.id));
    let group = |o: &Objective| {
        if o.items.iter().any(is_due) {
            0
        } else if !o.items.iter().any(is_reviewed) {
            1
        } else {
            2
        }
    };
    let mut objectives: Vec<(u8, usize, &Objective)> = view
        .objectives
        .iter()
        .enumerate()
        .map(|(i, o)| (group(o), i, o))
        .collect();
    objectives.sort_by_key(|(g, i, _)| (*g, *i));
    objectives
        .into_iter()
        .take(CHECK_ITEMS)
        .filter_map(|(_, _, o)| {
            let pick = |pred: &dyn Fn(&Item) -> bool| {
                o.items
                    .iter()
                    .filter(|i| pred(i))
                    .min_by_key(|i| stable_hash(seed, &i.id))
            };
            pick(&is_due)
                .or_else(|| pick(&|i| !is_reviewed(i)))
                .or_else(|| o.items.first())
                .map(|i| i.id.clone())
        })
        .collect()
}

pub fn chapter_status(view: &ChapterView, due: &BTreeSet<String>, book: &str) -> ChapterStatus {
    let any_due = view
        .objectives
        .iter()
        .flat_map(|o| &o.items)
        .any(|i| due.contains(&card_id(book, &i.id)));
    if any_due {
        ChapterStatus::Due
    } else if view.checked {
        ChapterStatus::Checked
    } else {
        ChapterStatus::NotRead
    }
}

// ------------------------------------------------------------------ tests

#[cfg(test)]
mod tests {
    use super::*;
    use crate::course::ItemKind;
    use proptest::prelude::*;

    fn pg(s: &str) -> String {
        s.to_string()
    }

    fn oe(title: &str, page: u32, depth: u8) -> OutlineEntry {
        OutlineEntry {
            title: title.into(),
            page,
            depth,
        }
    }

    fn mcq(q: &str, ans: &str) -> Item {
        Item {
            kind: ItemKind::Mcq,
            q: q.into(),
            options: vec![ans.into(), "red".into(), "green".into(), "blue".into()],
            answer: ans.into(),
            ..Item::default()
        }
    }

    fn raw(text: &str, s: u32, e: u32, items: Vec<Item>) -> RawObjective {
        RawObjective {
            text: text.into(),
            start_page: s,
            end_page: e,
            items,
        }
    }

    fn meta() -> BookMeta {
        BookMeta {
            id: "tb_0123456789abcdef".into(),
            title: "T".into(),
            file_name: "t.pdf".into(),
            bytes: 1,
            sha256: "0".repeat(64),
            total_pages: 100,
            page_offset: None,
        }
    }

    fn chap(i: u32, s: u32, e: u32) -> Chapter {
        Chapter {
            index: i,
            title: format!("c{i}"),
            pages: PageRange { start: s, end: e },
            kind: ChapterKind::Chapter,
        }
    }

    fn obj(id: &str, items: &[&str]) -> Objective {
        Objective {
            id: id.into(),
            text: id.into(),
            pages: PageRange { start: 1, end: 1 },
            items: items
                .iter()
                .map(|i| Item {
                    id: (*i).into(),
                    ..mcq("q", "a")
                })
                .collect(),
        }
    }

    fn done(chapter: u32, objectives: Vec<Objective>) -> TextbookEvent {
        TextbookEvent::ChapterDone {
            ts: 1,
            chapter,
            parts: 1,
            truncated: false,
            objectives,
        }
    }

    fn mapped(chs: Vec<Chapter>) -> TextbookEvent {
        TextbookEvent::Mapped {
            ts: 1,
            source: MapSource::Outline,
            chapters: chs,
        }
    }

    fn imported(ts: i64) -> TextbookEvent {
        TextbookEvent::Imported { ts, book: meta() }
    }

    #[test]
    fn sanitize_strips_controls_and_bidi_keeps_newlines() {
        let s = sanitize_page("a\u{0}b\u{7}c\nd\te\u{202E}f\u{FEFF}g\u{200B}h\u{2066}i\r");
        assert_eq!(s, "abc\nd\tefghi");
        let blanks = sanitize_page("a\n\n\n\n\n\nb");
        assert_eq!(blanks, "a\n\n\nb");
        let long = sanitize_page(&"é".repeat(MAX_PAGE_CHARS + 50));
        assert_eq!(long.chars().count(), MAX_PAGE_CHARS);
    }

    #[test]
    fn printed_offset_skips_roman_front_matter() {
        let pages: Vec<String> = ["i", "ii", "iii", "iv", "1", "2", "3"]
            .iter()
            .map(|n| format!("{n}\nbody text\n"))
            .collect();
        assert_eq!(printed_offset(&pages), Some(4));
        // number on the last line works too
        let last: Vec<String> = ["body\n1", "body\n2", "body\n3"]
            .iter()
            .map(|s| pg(s))
            .collect();
        assert_eq!(printed_offset(&last), Some(0));
    }

    #[test]
    fn printed_offset_none_without_numbers() {
        let pages = vec![pg("hello world"), pg("more text"), pg("")];
        assert_eq!(printed_offset(&pages), None);
        assert_eq!(printed_offset(&[]), None);
    }

    #[test]
    fn outline_level_falls_to_depth_one_for_parts() {
        let e = vec![
            oe("Part I", 1, 0),
            oe("Part II", 50, 0),
            oe("Ch 1", 1, 1),
            oe("Ch 2", 20, 1),
            oe("Ch 3", 50, 1),
        ];
        let c = chapters_from_outline(&e, 80);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].pages, PageRange { start: 1, end: 19 });
        assert_eq!(c[2].pages, PageRange { start: 50, end: 80 });
        // three depth-0 entries keep depth 0
        let e2 = vec![oe("A", 1, 0), oe("B", 5, 0), oe("C", 9, 0), oe("x", 2, 1)];
        assert_eq!(chapters_from_outline(&e2, 12).len(), 3);
    }

    #[test]
    fn outline_picks_leaf_chapters_under_part_containers() {
        // document order: Preface, then three Parts with chapters, then Index
        let e = vec![
            oe("Preface", 1, 0),
            oe("Part I: Basics", 9, 0),
            oe("Chapter 1", 9, 1),
            oe("1.1 A section", 12, 2),
            oe("Chapter 2", 30, 1),
            oe("Part II", 60, 0),
            oe("Chapter 3", 61, 1),
            oe("Chapter 4", 90, 1),
            oe("Part III", 120, 0),
            oe("Chapter 5", 121, 1),
            oe("Index", 150, 0),
        ];
        let c = chapters_from_outline(&e, 160);
        let titles: Vec<&str> = c.iter().map(|c| c.title.as_str()).collect();
        assert_eq!(
            titles,
            [
                "Preface",
                "Chapter 1",
                "Chapter 2",
                "Chapter 3",
                "Chapter 4",
                "Chapter 5",
                "Index"
            ]
        );
        assert_eq!(c[1].pages, PageRange { start: 9, end: 29 });
        assert_eq!(
            c[5].pages,
            PageRange {
                start: 121,
                end: 149
            }
        );
        assert_eq!(c[0].kind, ChapterKind::Front);
        assert_eq!(c[6].kind, ChapterKind::Back);
        // chapters that merely start with "Part" stay chapters
        let e = vec![
            oe("Partial Orders", 1, 0),
            oe("Particles", 20, 0),
            oe("Parties", 40, 0),
            oe("x", 2, 1),
        ];
        assert_eq!(chapters_from_outline(&e, 60).len(), 3);
        assert!(is_part_container("Part 2"));
        assert!(is_part_container("Book Three"));
        assert!(!is_part_container("Part of Speech"));
    }

    #[test]
    fn manual_import_times_count_distinct_job_chapter_pairs_in_window() {
        let u = |ts, job: &str, chapter, requested| TextbookEvent::Usage {
            ts,
            job: job.into(),
            chapter,
            role: "placer".into(),
            tokens_in: 1,
            tokens_out: 1,
            calls: 1,
            requested,
        };
        let ev = vec![
            u(10, "j1", Some(0), true),
            u(11, "j1", Some(0), true), // second call of the same chapter import
            u(12, "j1", Some(1), true),
            u(13, "map", None, false), // the TOC call is never a chapter import
            u(14, "j1", Some(2), false),
            u(100, "j2", Some(0), true), // same chapter, a new import
        ];
        assert_eq!(manual_import_times(&ev, 0), vec![10, 12, 100]);
        assert_eq!(manual_import_times(&ev, 11), vec![12, 100]);
    }

    #[test]
    fn outline_same_start_pages_merge() {
        let e = vec![
            oe("Contents", 1, 0),
            oe("Chapter 1", 1, 0),
            oe("Chapter 2", 9, 0),
            oe("Index", 99, 0),
            oe("Off", 500, 0),
        ];
        let c = chapters_from_outline(&e, 100);
        assert_eq!(c.len(), 3);
        assert_eq!(c[0].title, "Contents / Chapter 1");
        assert_eq!(c[0].kind, ChapterKind::Chapter);
        assert_eq!(c[0].pages, PageRange { start: 1, end: 8 });
        assert_eq!(c[2].kind, ChapterKind::Back);
        assert_eq!(c[2].pages.end, 100);
    }

    #[test]
    fn toc_maps_printed_pages_with_offset_and_title_search() {
        let mut pages: Vec<String> = (1..=30).map(|i| format!("body {i}")).collect();
        pages[11] = pg("Streams And Pipes\nintro"); // physical 12
        let entries = vec![
            ("Basics".to_string(), 1u32),
            ("Streams And Pipes".to_string(), 7u32), // 7 + 4 = 11, found at 12
            ("Gone".to_string(), 900u32),
        ];
        let c = chapters_from_toc(&entries, Some(4), &pages, 30);
        assert_eq!(c.len(), 2);
        assert_eq!(c[0].pages, PageRange { start: 5, end: 11 });
        assert_eq!(c[1].pages, PageRange { start: 12, end: 30 });
        // no offset, no title hit: the mapped page stands
        let c2 = chapters_from_toc(&[("Zed".to_string(), 3)], None, &pages, 30);
        assert_eq!(c2[0].pages.start, 3);
    }

    #[test]
    fn kind_of_front_back_and_appendix() {
        assert_eq!(kind_of("  Preface "), ChapterKind::Front);
        assert_eq!(kind_of("TABLE OF CONTENTS"), ChapterKind::Front);
        assert_eq!(kind_of("Index"), ChapterKind::Back);
        assert_eq!(kind_of("About the Authors"), ChapterKind::Back);
        assert_eq!(kind_of("Appendix A"), ChapterKind::Chapter);
        assert_eq!(kind_of("Chapter 1"), ChapterKind::Chapter);
    }

    #[test]
    fn split_parts_8k_boundary_and_three_part_cap() {
        let pages = |n: u32, w: usize| (1..=n).map(|p| (p, w)).collect::<Vec<_>>();
        let one = split_parts(&pages(10, 799));
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].0, PageRange { start: 1, end: 10 });
        let edge = split_parts(&pages(10, 800)); // 8000 words: ceil gives k = 1
        assert_eq!(edge.len(), 1);
        let nine = split_parts(&pages(10, 900)); // 9000 -> k = 2
        assert_eq!(nine.len(), 2);
        assert!(nine.iter().all(|(_, t)| !t));
        let big = split_parts(&pages(40, 1000)); // 40k words
        assert_eq!(big.len(), 3);
        assert!(big.iter().all(|(_, t)| *t));
        assert_eq!(big[0].0.start, 1);
        assert_eq!(big[2].0.end, 40);
        assert!(split_parts(&[]).is_empty());
    }

    #[test]
    fn objective_bounds_per_parts() {
        assert_eq!(objective_bounds(1), (5, 12));
        assert_eq!(objective_bounds(2), (3, 6));
        assert_eq!(objective_bounds(3), (2, 4));
    }

    #[test]
    fn accept_objectives_clamps_pages_drops_bad_items_and_numbers_ids() {
        let part = PageRange { start: 10, end: 20 };
        let bad = Item {
            options: vec!["a".into()],
            ..mcq("Which?", "a")
        };
        let items = vec![
            mcq("What is the capital of France?", "Paris"),
            bad,
            mcq("What is two plus two?", "four"),
            mcq("fourth item cut off", "x"),
        ];
        let out = accept_objectives(
            3,
            4,
            part,
            vec![
                raw("  Know\u{202E} Paris ", 2, 99, items.clone()),
                raw("inverted", 19, 12, items.clone()),
                raw("no good items", 10, 11, vec![items[1].clone()]),
                raw("", 10, 11, items.clone()),
            ],
        );
        assert_eq!(out.len(), 2);
        assert_eq!(out[0].id, "ch003-o04");
        assert_eq!(out[0].text, "Know Paris");
        assert_eq!(out[0].pages, part);
        assert_eq!(out[1].id, "ch003-o05");
        assert_eq!(out[1].pages, part);
        // 3 taken, the bad one dropped, ids renumbered
        assert_eq!(out[0].items.len(), 2);
        assert_eq!(out[0].items[0].id, "ch003-o04-i1");
        assert_eq!(out[0].items[1].id, "ch003-o04-i2");
    }

    #[test]
    fn fold_created_at_is_first_imported_ts() {
        let st = fold_book(&[imported(5), imported(9)]);
        assert_eq!(st.created_at, Some(5));
    }

    #[test]
    fn fold_first_mapped_and_first_done_win() {
        let o1 = vec![obj("ch000-o01", &["ch000-o01-i1"])];
        let o2 = vec![obj("zzz", &["zzz-i1"])];
        let st = fold_book(&[
            imported(1),
            mapped(vec![chap(0, 1, 5), chap(1, 6, 9)]),
            mapped(vec![chap(0, 1, 2)]),
            done(0, o1.clone()),
            done(0, o2),
            done(7, vec![]),
        ]);
        assert_eq!(st.chapters.len(), 2);
        assert_eq!(st.chapters[0].objectives, o1);
        assert_eq!(st.chapters[0].state, ChapterState::Ready);
        assert_eq!(st.chapters[1].state, ChapterState::Pending);
    }

    #[test]
    fn fold_retry_done_overrides_failed() {
        let fail = |retryable| TextbookEvent::ChapterFailed {
            ts: 1,
            chapter: 0,
            reason: "parse".into(),
            retryable,
        };
        let base = vec![imported(1), mapped(vec![chap(0, 1, 5)]), fail(true)];
        let st = fold_book(&base);
        assert_eq!(st.chapters[0].state, ChapterState::Failed);
        assert_eq!(pending_chapters(&st), vec![0]);
        let mut non = base.clone();
        non.push(fail(false));
        assert!(pending_chapters(&fold_book(&non)).is_empty());
        let mut ok = base;
        ok.push(done(0, vec![obj("o", &["i"])]));
        ok.push(fail(true)); // late failure does not undo Ready
        let st = fold_book(&ok);
        assert_eq!(st.chapters[0].state, ChapterState::Ready);
        assert!(st.chapters[0].error.is_none());
        assert!(pending_chapters(&st).is_empty());
    }

    #[test]
    fn fold_usage_checks_and_answers() {
        let usage = |requested| TextbookEvent::Usage {
            ts: 1,
            job: "j".into(),
            chapter: Some(0),
            role: "placer".into(),
            tokens_in: 10,
            tokens_out: 5,
            calls: 1,
            requested,
        };
        let st = fold_book(&[
            imported(1),
            mapped(vec![chap(0, 1, 5)]),
            usage(false),
            usage(true),
            usage(true),
            TextbookEvent::CheckIssued {
                ts: 2,
                check: "rc_1".into(),
                chapter: 0,
                items: vec!["i".into()],
            },
            TextbookEvent::Answered {
                ts: 3,
                check: "rc_1".into(),
                item: "i".into(),
                correct: true,
                close: false,
                rating: Some(Rating::Good),
                client_id: None,
            },
        ]);
        assert_eq!((st.auto_calls, st.requested_calls), (1, 2));
        assert_eq!((st.cheap_in, st.cheap_out), (30, 15));
        assert!(st.chapters[0].checked);
        assert_eq!(st.answered, vec![("rc_1".to_string(), "i".to_string())]);
    }

    #[test]
    fn select_check_due_first_one_per_objective_deterministic() {
        let book = "tb_0123456789abcdef";
        let objectives = vec![
            obj("o1", &["o1-a", "o1-b"]),
            obj("o2", &["o2-a", "o2-b"]),
            obj("o3", &["o3-a"]),
        ];
        let st = fold_book(&[
            imported(1),
            mapped(vec![chap(0, 1, 5)]),
            done(0, objectives),
        ]);
        let set = |v: &[&str]| -> BTreeSet<String> { v.iter().map(|i| card_id(book, i)).collect() };
        // o3 due -> first; o2 never reviewed, o1 reviewed -> after
        let out = select_check(&st, 0, &set(&["o3-a"]), &set(&["o1-a", "o3-a"]), "s");
        assert_eq!(out[0], "o3-a");
        assert!(out[1].starts_with("o2"));
        assert!(out[2].starts_with("o1"));
        assert_eq!(out[2], "o1-b", "never-reviewed item preferred");
        assert_eq!(out.len(), 3);
        // due item inside an objective wins over the unreviewed one
        let out = select_check(&st, 0, &set(&["o1-a"]), &set(&["o1-a"]), "s");
        assert_eq!(out[0], "o1-a");
        // deterministic for the same seed
        let again = select_check(&st, 0, &set(&[]), &set(&[]), "seed-x");
        assert_eq!(again, select_check(&st, 0, &set(&[]), &set(&[]), "seed-x"));
        // not ready or unknown chapter gives nothing
        assert!(select_check(&st, 5, &set(&[]), &set(&[]), "s").is_empty());
    }

    #[test]
    fn select_check_caps_at_check_items() {
        let objectives: Vec<Objective> = (0..12)
            .map(|i| obj(&format!("o{i}"), &[&format!("o{i}-a")]))
            .collect();
        let st = fold_book(&[
            imported(1),
            mapped(vec![chap(0, 1, 5)]),
            done(0, objectives),
        ]);
        let none = BTreeSet::new();
        assert_eq!(select_check(&st, 0, &none, &none, "s").len(), CHECK_ITEMS);
    }

    #[test]
    fn chapter_status_due_checked_not_read() {
        let book = "tb_0123456789abcdef";
        let mut v = ChapterView {
            chapter: chap(0, 1, 5),
            state: ChapterState::Ready,
            objectives: vec![obj("o1", &["o1-a"])],
            error: None,
            truncated: false,
            checked: false,
            retryable: false,
        };
        let none = BTreeSet::new();
        let due: BTreeSet<String> = [card_id(book, "o1-a")].into();
        assert_eq!(chapter_status(&v, &none, book), ChapterStatus::NotRead);
        assert_eq!(chapter_status(&v, &due, book), ChapterStatus::Due);
        v.checked = true;
        assert_eq!(chapter_status(&v, &none, book), ChapterStatus::Checked);
        assert_eq!(chapter_status(&v, &due, book), ChapterStatus::Due);
    }

    #[test]
    fn book_id_rules() {
        let hex = "ab".repeat(32);
        let id = book_id(&hex);
        assert_eq!(id, "tb_abababababababab");
        assert!(valid_book_id(&id));
        for bad in [
            "",
            "tb_",
            "../x",
            "tb_XYZ",
            "tb_ABABABABABABABAB",
            "tb_abababababababa",
        ] {
            assert!(!valid_book_id(bad), "{bad}");
        }
        assert_eq!(card_id("tb_x", "i"), "t:tb_x:i");
    }

    #[test]
    fn event_json_round_trip() {
        let ev = TextbookEvent::Answered {
            ts: 1,
            check: "c".into(),
            item: "i".into(),
            correct: false,
            close: true,
            rating: None,
            client_id: Some("x".into()),
        };
        let s = serde_json::to_string(&ev).unwrap();
        assert!(s.contains(r#""type":"answered""#));
        assert_eq!(serde_json::from_str::<TextbookEvent>(&s).unwrap(), ev);
        // client_id is optional on old lines
        let old = r#"{"type":"answered","ts":1,"check":"c","item":"i","correct":true,"close":false,"rating":null}"#;
        assert!(serde_json::from_str::<TextbookEvent>(old).is_ok());
    }

    fn assert_invariants(c: &[Chapter], total: u32) {
        assert!(c.len() <= MAX_CHAPTERS);
        for (i, ch) in c.iter().enumerate() {
            assert_eq!(ch.index as usize, i);
            assert!(1 <= ch.pages.start && ch.pages.start <= ch.pages.end);
            assert!(ch.pages.end <= total);
            if let Some(n) = c.get(i + 1) {
                assert!(ch.pages.start < n.pages.start);
                assert!(ch.pages.end < n.pages.start);
            }
        }
        // A truncated list (MAX_CHAPTERS) deliberately stops before the end of the book.
        if c.len() < MAX_CHAPTERS {
            if let Some(l) = c.last() {
                assert_eq!(l.pages.end, total);
            }
        }
    }

    #[test]
    fn truncated_chapters_do_not_swallow_the_rest_of_the_book() {
        // 250 one-page-apart chapters in a 500-page book: the 200th kept chapter must end
        // where chapter 201 would have started, not at the end of the PDF.
        let total = 500;
        let starts: Vec<(String, u32)> = (0..250u32)
            .map(|i| (format!("Chapter {i}"), i * 2 + 1))
            .collect();
        let c = finish_chapters(starts, total);
        assert_eq!(c.len(), MAX_CHAPTERS);
        let last = c.last().unwrap();
        assert_eq!(last.pages.start, 199 * 2 + 1);
        assert_eq!(last.pages.end, 200 * 2); // next (dropped) chapter starts at page 401
        assert_invariants(&c, total);
    }

    proptest! {
        #[test]
        fn chapter_ranges_invariants(
            total in 0u32..600,
            outline in proptest::collection::vec(("[a-z ]{0,12}", 0u32..700, 0u8..3), 0..300),
            toc in proptest::collection::vec(("[a-z ]{0,12}", 0u32..700), 0..300),
            offset in proptest::option::of(-20i32..20),
        ) {
            let entries: Vec<OutlineEntry> =
                outline.into_iter().map(|(t, p, d)| OutlineEntry { title: t, page: p, depth: d }).collect();
            assert_invariants(&chapters_from_outline(&entries, total), total);
            let pages: Vec<String> = (0..total.min(50)).map(|i| format!("p{i}")).collect();
            assert_invariants(&chapters_from_toc(&toc, offset, &pages, total), total);
            let chunks = chunk_chapters(total);
            assert_invariants(&chunks, total);
            assert_eq!(chunks.is_empty(), total == 0);
        }

        #[test]
        fn split_parts_covers_exactly(words in proptest::collection::vec(0usize..4000, 1..80)) {
            let pw: Vec<(u32, usize)> = words.iter().enumerate().map(|(i, w)| (i as u32 + 5, *w)).collect();
            let parts = split_parts(&pw);
            prop_assert!(!parts.is_empty() && parts.len() <= MAX_PARTS);
            prop_assert_eq!(parts[0].0.start, 5);
            prop_assert_eq!(parts.last().map(|p| p.0.end), Some(pw.len() as u32 + 4));
            for w in parts.windows(2) {
                prop_assert_eq!(w[0].0.end + 1, w[1].0.start);
            }
            for (r, _) in &parts {
                prop_assert!(r.start <= r.end);
            }
        }
    }
}
