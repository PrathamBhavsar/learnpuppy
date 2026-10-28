//! FSRS 4-Choice Visual Review Deck & Local CLI Transcript Decision Digest Browser (D23).
//!
//! Renders the dual-mode `Review` / `Digest` screen (`DigestFsrsView`) with:
//! - **4-Choice Visual FSRS Review Deck**: Mastery rings, clickable `[A]..[D]` option cards,
//!   instant rationale reveal, and 1-click deterministic FSRS v5 grading (`Again`, `Hard`,
//!   `Good`, `Easy`) using [`lp_core::fsrs::review`] and [`lp_core::fsrs::preview`].
//! - **Local CLI Transcript Decision Digest Browser**: Multi-account session browser (`claude`
//!   Account #1 & #2, `codex`, `agy`), repo allowlist filter, distilled [`DecisionRecord`]
//!   cards (`what`, `why`, `basis`, `alternatives`, `risk`, `files`, `commit_sha`), one-click
//!   **Ingest Now** (via [`Backend::ingest`]), **Fold into Graph**, and **Mint Cards**.

use std::path::PathBuf;

use dioxus::prelude::*;
use lp_core::fsrs::{preview, review, Card, Rating, State as FsrsState};
use lp_core::ports::{Alternative, CodingSession, DecisionBasis, DecisionRecord, FlashCard};
use lp_ui::code_block::CodeBlock;
use lp_ui::icons::{
    IconBrain, IconCheck, IconDigest, IconFilter, IconGraph, IconPlus, IconRefresh, IconSearch,
    IconSparkles, IconTerminal, IconX,
};
use lp_ui::overlays::{Checkbox, ScrollArea, SplitDirection, SplitPane, Switch, Tooltip};
use lp_ui::primitives::{
    Badge, BadgeVariant, Button, ButtonSize, ButtonVariant, Card as UiCard, CardContent,
    CardDescription, CardFooter, CardHeader, CardTitle, MasteryRing, Progress, Separator,
};
use lp_ui::tabs::{Tabs, TabsContent, TabsList, TabsTrigger};

use crate::backend::{BackendHandle, BackendResult};
use crate::state::{AppState, View};

const BASE_NOW_MS: i64 = 1_767_258_000_000;
const OPTION_LETTERS: [&str; 4] = ["A", "B", "C", "D"];

/// Track category for a review flashcard (`Decision` vs `Book / Code / Rust`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CardTrack {
    Decision,
    RustBook,
}

impl CardTrack {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Decision => "Decision Track",
            Self::RustBook => "Rust & Systems Track",
        }
    }

    pub const fn id(self) -> &'static str {
        match self {
            Self::Decision => "decision",
            Self::RustBook => "rust-book",
        }
    }
}

/// Enriched FSRS review card item for the visual deck.
#[derive(Debug, Clone, PartialEq)]
pub struct DeckCardItem {
    pub flashcard: FlashCard,
    pub kind_tag: String,
    pub track: CardTrack,
    pub source_label: String,
    pub code_snippet: Option<String>,
}

impl DeckCardItem {
    /// Computes normalized mastery score (`0.0..=1.0`) from FSRS stability, reps, and state.
    pub fn mastery_score(&self) -> f64 {
        let c = &self.flashcard.card;
        match c.state {
            FsrsState::New => 0.15,
            FsrsState::Learning => (0.35 + (f64::from(c.reps) * 0.08)).clamp(0.25, 0.65),
            FsrsState::Relearning => (0.30 + (c.stability / 30.0) * 0.25).clamp(0.20, 0.55),
            FsrsState::Review => (0.55 + (c.stability / 25.0) * 0.45).clamp(0.55, 0.98),
        }
    }
}

/// Ingested CLI session entry paired with its distilled architectural decisions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestSessionItem {
    pub session: CodingSession,
    pub account_label: String,
    pub repo_name: String,
    pub summary: String,
    pub decisions: Vec<DecisionRecord>,
    pub folded_to_graph: bool,
    pub cards_minted: usize,
}

/// Formats a millisecond interval from `lp_core::fsrs::preview` into a compact human label.
pub fn format_interval_ms(ms: i64) -> String {
    let clamped = ms.max(60_000);
    let mins = (clamped + 30_000) / 60_000;
    if mins < 60 {
        format!("{mins}m")
    } else if mins < 60 * 24 {
        let hours = (mins + 30) / 60;
        format!("{hours}h")
    } else {
        let days = (mins + 720) / (60 * 24);
        format!("{days}d")
    }
}

/// Returns human-readable FSRS state label.
pub const fn fsrs_state_label(state: FsrsState) -> &'static str {
    match state {
        FsrsState::New => "New",
        FsrsState::Learning => "Learning",
        FsrsState::Review => "Review",
        FsrsState::Relearning => "Relearning",
    }
}

/// Returns human-readable FSRS rating label.
pub const fn rating_label(rating: Rating) -> &'static str {
    match rating {
        Rating::Again => "Again",
        Rating::Hard => "Hard",
        Rating::Good => "Good",
        Rating::Easy => "Easy",
    }
}

/// Returns lowercase test-id slug for an FSRS rating.
pub const fn rating_slug(rating: Rating) -> &'static str {
    match rating {
        Rating::Again => "again",
        Rating::Hard => "hard",
        Rating::Good => "good",
        Rating::Easy => "easy",
    }
}

/// Seeded 4-choice FSRS flashcards covering Decision Track (`Why`, `Predict`, `Bug`)
/// and Rust/Systems Track (`Ownership`, `Trace`, `Cloze`).
pub fn default_deck_cards() -> Vec<DeckCardItem> {
    let mut card_wal = Card::new(BASE_NOW_MS);
    card_wal.state = FsrsState::Review;
    card_wal.stability = 14.5;
    card_wal.difficulty = 4.2;
    card_wal.reps = 4;

    let mut card_agy = Card::new(BASE_NOW_MS);
    card_agy.state = FsrsState::Learning;
    card_agy.stability = 3.1;
    card_agy.difficulty = 5.4;
    card_agy.reps = 2;

    let mut card_simplify = Card::new(BASE_NOW_MS);
    card_simplify.state = FsrsState::New;

    let mut card_borrow = Card::new(BASE_NOW_MS);
    card_borrow.state = FsrsState::Review;
    card_borrow.stability = 24.0;
    card_borrow.difficulty = 3.1;
    card_borrow.reps = 6;

    vec![
        DeckCardItem {
            flashcard: FlashCard {
                id: "c_agy_snapshot".into(),
                front: "When reading an active SQLite database managed by another process in WAL mode (`conversation_summaries.db`), why copy `.db`, `-wal`, and `-shm` into a `TempDir` snapshot before opening?".into(),
                options: [
                    "Opening in place can race with concurrent checkpoints or hold locks on live WAL frames".into(),
                    "SQLite refuses to open any database file located inside a hidden dot-directory".into(),
                    "Copying the files automatically converts the schema from UTF-16 to UTF-8".into(),
                    "WAL journals are deleted immediately when a second reader opens the file read-only".into(),
                ],
                correct_index: 0,
                rationale: "Snapshot isolation (`with_snapshot`) copies `.db`, `-wal`, and `-shm` into a temporary directory first so LearnPuppy never contends with Antigravity's live SQLite WAL writer or observes torn frames.".into(),
                card: card_agy,
            },
            kind_tag: "Why · Decision".into(),
            track: CardTrack::Decision,
            source_label: "lp-ingest · with_snapshot".into(),
            code_snippet: Some(
                "for suffix in [\"\", \"-wal\", \"-shm\"] {\n    let src = agy_dir.join(format!(\"conversation_summaries.db{suffix}\"));\n    if src.exists() { std::fs::copy(&src, snap_dir.join(src.file_name().unwrap()))?; }\n}".into(),
            ),
        },
        DeckCardItem {
            flashcard: FlashCard {
                id: "c_fsrs_replay".into(),
                front: "How does LearnPuppy guarantee that two devices syncing `fsrs_reviews` out of order derive the exact same FSRS v5 `Card` schedule?".into(),
                options: [
                    "Last-write-wins timestamp overwrite on the mutable `fsrs_state` row".into(),
                    "Sorting all `ReviewEvent`s by `(ts, id)` and deterministically replaying `review` from `Card::new(0)`".into(),
                    "Using randomized fuzz factors seeded from the local MAC address".into(),
                    "Locking a central PostgreSQL sequence before each flashcard review".into(),
                ],
                correct_index: 1,
                rationale: "`lp_core::fsrs::replay` sorts review events by `(ts, id)` with `enable_fuzz = false` and folds `review(&card, event.ts, event.rating)` from `Card::new(0)`, making multi-device sync order-independent.".into(),
                card: card_wal,
            },
            kind_tag: "Predict · Decision".into(),
            track: CardTrack::Decision,
            source_label: "lp-core::fsrs · replay".into(),
            code_snippet: Some(
                "pub fn replay(events: &[ReviewEvent]) -> Card {\n    let mut sorted: Vec<&ReviewEvent> = events.iter().collect();\n    sorted.sort_by(|a, b| (a.ts, &a.id).cmp(&(b.ts, &b.id)));\n    sorted.into_iter().fold(Card::new(0), |c, e| review(&c, e.ts, e.rating))\n}".into(),
            ),
        },
        DeckCardItem {
            flashcard: FlashCard {
                id: "c_simplify_chains".into(),
                front: "Given chains `A + B => X`, `X + C => Y`, and `A + B + C => Y`, what does `simplify_chains` keep after transitive ancestry reduction?".into(),
                options: [
                    "Only `A + B + C => Y` because it has the most explicit premises".into(),
                    "All three chains so no redundant edges are ever lost".into(),
                    "Only `A + B => X` and `C + X => Y`, dropping `A + B + C => Y` as transitively covered".into(),
                    "An empty set because `X` appears as both conclusion and premise".into(),
                ],
                correct_index: 2,
                rationale: "Rule 2 of `simplify_chains` drops candidate chains whose premises are already covered by a kept chain's premises (`C`, `X`) plus their transitive ancestors (`A`, `B`).".into(),
                card: card_simplify,
            },
            kind_tag: "Trace · Rust".into(),
            track: CardTrack::RustBook,
            source_label: "lp-core::graph · simplify_chains".into(),
            code_snippet: None,
        },
        DeckCardItem {
            flashcard: FlashCard {
                id: "c_dioxus_blocking".into(),
                front: "Why must synchronous `rusqlite` and filesystem scans in `lp-app` run inside `tokio::task::spawn_blocking` rather than directly inside a Dioxus component body?".into(),
                options: [
                    "`rusqlite::Connection` panics at runtime if called from the main thread".into(),
                    "Dioxus signals cannot be read on macOS `aarch64-apple-darwin`".into(),
                    "The Dioxus VirtualDOM runs on the main UI thread (`!Send`), so blocking IO stalls 60fps rendering".into(),
                    "`spawn_blocking` automatically rewrites SQL queries into prepared statements".into(),
                ],
                correct_index: 2,
                rationale: "Offloading synchronous SQLite/filesystem IO to `tokio::task::spawn_blocking` keeps the macOS `WKWebView` main thread responsive at 60fps.".into(),
                card: card_borrow,
            },
            kind_tag: "Bug · Ownership".into(),
            track: CardTrack::RustBook,
            source_label: "lp-app::backend · DesktopBackend".into(),
            code_snippet: None,
        },
    ]
}

/// Seeded local CLI transcript sessions & distilled architectural `DecisionRecord`s.
pub fn default_digest_sessions() -> Vec<DigestSessionItem> {
    vec![
        DigestSessionItem {
            session: CodingSession {
                id: "ses_tracker_fsrs_01".into(),
                source: "claude".into(),
                cwd: "/Users/pratham/code/tracker".into(),
                git_branch: Some("main".into()),
                started_at: BASE_NOW_MS - 7_200_000,
                transcript: "User: Port the deterministic FSRS v5 scheduler and 4-choice card shape validator.\nAssistant: Implemented replay sorted by (ts, id) and validate_card_shape length-tell gate.\n[tool: Edit crates/lp-core/src/fsrs.rs]".into(),
            },
            account_label: "claude:pratham.bhavsar501@gmail.com (#1)".into(),
            repo_name: "tracker".into(),
            summary: "Deterministic FSRS v5 replay & 4-choice MC card shape quality gates".into(),
            folded_to_graph: false,
            cards_minted: 2,
            decisions: vec![
                DecisionRecord {
                    id: "d_fsrs_deterministic_replay".into(),
                    session_id: "ses_tracker_fsrs_01".into(),
                    title: "Order-Independent FSRS v5 Replay via (ts, id) Fold".into(),
                    what: "Implemented `lp_core::fsrs::replay` to sort `ReviewEvent`s by `(ts, id)` and fold `review` deterministically with `enable_fuzz = false`.".into(),
                    why: "Guarantees desktop and mobile companions converge on identical `due`, `stability`, and `difficulty` values regardless of `sync-<device>.jsonl` merge order.".into(),
                    basis: DecisionBasis::Stated,
                    alternatives: vec![
                        Alternative {
                            option: "Mutable SQLite-only state row without event log".into(),
                            rejected_because: "Loses concurrent reviews when syncing offline JSONL ledgers across devices.".into(),
                        },
                        Alternative {
                            option: "Randomized interval fuzzing (`enable_fuzz = true`)".into(),
                            rejected_because: "Breaks deterministic cross-device replay parity.".into(),
                        },
                    ],
                    risk: "Clock skew across devices can reorder same-minute reviews if system NTP is drifted.".into(),
                    files: vec![
                        "crates/lp-core/src/fsrs.rs".into(),
                        "crates/lp-store/src/sqlite.rs".into(),
                    ],
                    commit_sha: Some("8f4a21c".into()),
                },
                DecisionRecord {
                    id: "d_card_length_tell_gate".into(),
                    session_id: "ses_tracker_fsrs_01".into(),
                    title: "Enforce 1.6x Option Length-Ratio & Context-Independence Gates".into(),
                    what: "Added `validate_card_shape` in `lp-core::fsrs` rejecting prompts with unstated context or correct options exceeding `1.6 * mean_distractor_len + 10.0`.".into(),
                    why: "Prevents LLM-generated multiple-choice cards from giving away the answer via verbose correct options or dangling 'in this commit' references.".into(),
                    basis: DecisionBasis::Stated,
                    alternatives: vec![Alternative {
                        option: "Prompt-only instructions without a post-generation validator".into(),
                        rejected_because: "LLMs still emit length tells on ~18% of generated distractors.".into(),
                    }],
                    risk: "Strict regex may reject valid questions unless concrete code is fenced in backticks.".into(),
                    files: vec![
                        "crates/lp-core/src/fsrs.rs".into(),
                        "crates/lp-ingest/src/distill/cards.rs".into(),
                    ],
                    commit_sha: Some("3c91e0b".into()),
                },
            ],
        },
        DigestSessionItem {
            session: CodingSession {
                id: "ses_bunfiiliate_pool_02".into(),
                source: "claude".into(),
                cwd: "/Users/pratham/code/bunfiiliate".into(),
                git_branch: Some("feat/multi-account-pool".into()),
                started_at: BASE_NOW_MS - 3_600_000,
                transcript: "User: Discover all local Claude subscription dirs and failover on 429 cooldown.\nAssistant: Added DiscoverRoots scanning ~/.claude, ~/.claude-work, and ~/.claude-accounts/* with AccountPool cooldown tracking.\n[tool: Edit crates/lp-cli-driver/src/accounts.rs]".into(),
            },
            account_label: "claude:q97159745@gmail.com (#2)".into(),
            repo_name: "bunfiiliate".into(),
            summary: "Multi-account CLI pool auto-discovery & rate-limit cooldown failover".into(),
            folded_to_graph: false,
            cards_minted: 1,
            decisions: vec![DecisionRecord {
                id: "d_multi_account_claude_pool".into(),
                session_id: "ses_bunfiiliate_pool_02".into(),
                title: "Per-Account CLAUDE_CONFIG_DIR Rate-Limit Buckets".into(),
                what: "Discovers every `.claude.json` profile across `~/.claude`, `~/.claude-work`, and `~/.claude-accounts/*` and schedules turns via `AccountPool`.".into(),
                why: "Allows seamless zero-downtime failover to Account #2 when Account #1 hits a 5-hour subscription window limit, without ever invoking blocking macOS Keychain dialogs.".into(),
                basis: DecisionBasis::Stated,
                alternatives: vec![Alternative {
                    option: "Single global Claude CLI binary invocation".into(),
                    rejected_because: "Stalls the entire learning session when one subscription hits a rate limit.".into(),
                }],
                risk: "Expired OAuth tokens in secondary profile directories require clear UI health badges.".into(),
                files: vec![
                    "crates/lp-cli-driver/src/accounts.rs".into(),
                    "crates/lp-app/src/backend.rs".into(),
                ],
                commit_sha: Some("a71d94e".into()),
            }],
        },
        DigestSessionItem {
            session: CodingSession {
                id: "ses_learnpuppy_agy_03".into(),
                source: "agy".into(),
                cwd: "/Users/pratham/code/learnpuppy".into(),
                git_branch: Some("staging".into()),
                started_at: BASE_NOW_MS - 1_800_000,
                transcript: "User: Ensure Antigravity SQLite summaries are never opened in place.\nAssistant: Implemented TempDir snapshot copy in crates/lp-ingest/src/agy.rs and verified simplifying chains.\n[tool: Edit crates/lp-ingest/src/agy.rs]".into(),
            },
            account_label: "agy:default".into(),
            repo_name: "learnpuppy".into(),
            summary: "TempDir SQLite WAL snapshot isolation for Antigravity transcript ingestion".into(),
            folded_to_graph: true,
            cards_minted: 1,
            decisions: vec![DecisionRecord {
                id: "d_agy_tempdir_snapshot".into(),
                session_id: "ses_learnpuppy_agy_03".into(),
                title: "TempDir Snapshot Copy for Antigravity SQLite WAL Ingestion".into(),
                what: "Copies `conversation_summaries.db`, `-wal`, and `-shm` into an isolated `TempDir` before opening `rusqlite::Connection`.".into(),
                why: "Prevents SQLite locking contention or WAL corruption against the running Antigravity IDE.".into(),
                basis: DecisionBasis::Inferred,
                alternatives: vec![Alternative {
                    option: "Open `~/.gemini/antigravity-cli/conversation_summaries.db` with `SQLITE_OPEN_READ_ONLY` in place".into(),
                    rejected_because: "Still acquires shared WAL locks and risks observing partial checkpoint state.".into(),
                }],
                risk: "Requires ~2-5 MB of temporary disk space during the brief ingestion scan.".into(),
                files: vec!["crates/lp-ingest/src/agy.rs".into()],
                commit_sha: Some("f02b88c".into()),
            }],
        },
        DigestSessionItem {
            session: CodingSession {
                id: "ses_tracker_codex_04".into(),
                source: "codex".into(),
                cwd: "/Users/pratham/code/tracker".into(),
                git_branch: Some("feat/fts5-bm25".into()),
                started_at: BASE_NOW_MS - 900_000,
                transcript: "User: Add FTS5 BM25 proposition search alongside JSONL ledgers.\nAssistant: Created propositions_fts and transcripts_fts virtual tables in index.db.\n[tool: Edit crates/lp-store/src/sqlite.rs]".into(),
            },
            account_label: "codex:default".into(),
            repo_name: "tracker".into(),
            summary: "Hybrid JSONL append-only ledgers + SQLite FTS5 BM25 search index".into(),
            folded_to_graph: false,
            cards_minted: 1,
            decisions: vec![DecisionRecord {
                id: "d_hybrid_jsonl_sqlite_fts5".into(),
                session_id: "ses_tracker_codex_04".into(),
                title: "Append-Only JSONL Truth with Derived SQLite FTS5 Index".into(),
                what: "Persists graph nodes and chains in `nodes.jsonl` and `chains.jsonl` while indexing propositions in SQLite `propositions_fts`.".into(),
                why: "Combines git/syncthing-friendly line-oriented ledgers with sub-millisecond BM25 prefix search.".into(),
                basis: DecisionBasis::Stated,
                alternatives: vec![Alternative {
                    option: "Store graph solely inside opaque binary SQLite tables".into(),
                    rejected_because: "Binary SQLite files conflict on multi-device file sync.".into(),
                }],
                risk: "Index must be rebuilt if `nodes.jsonl` is edited externally.".into(),
                files: vec!["crates/lp-store/src/lib.rs".into()],
                commit_sha: Some("e19c441".into()),
            }],
        },
    ]
}

/// Primary FSRS 4-Choice Visual Review & Local CLI Transcript Digest view (`Cmd+3`).
///
/// Exported with zero props; reads [`AppState`] and [`BackendHandle`] from Dioxus context.
#[component]
pub fn DigestFsrsView() -> Element {
    let mut app_state = use_context::<AppState>();
    let backend = try_use_context::<BackendHandle>();
    let view = (app_state.current_view)();

    // Active top-level workspace tab ("review" vs "digest"), synchronized with View::Review / View::Digest.
    let active_tab = match view {
        View::Digest => "digest".to_string(),
        _ => "review".to_string(),
    };

    // --- FSRS Review Deck State (100% pure Rust Signals) ---
    let mut deck = use_signal(default_deck_cards);
    let mut current_card_idx = use_signal(|| 0usize);
    let mut selected_option = use_signal(|| None::<u8>);
    let mut revealed = use_signal(|| false);
    let mut track_filter = use_signal(|| "all".to_string());
    let mut review_history = use_signal(Vec::<(String, Rating, i64)>::new);
    let mut custom_card_open = use_signal(|| false);
    let mut custom_front = use_signal(String::new);
    let mut custom_right = use_signal(String::new);
    let mut custom_wrong_1 = use_signal(String::new);
    let mut custom_wrong_2 = use_signal(String::new);
    let mut custom_wrong_3 = use_signal(String::new);
    let mut custom_rationale = use_signal(String::new);
    let mut custom_card_status = use_signal(|| None::<Result<String, String>>);

    // --- CLI Transcript Digest State (100% pure Rust Signals) ---
    let mut sessions = use_signal(default_digest_sessions);
    let mut selected_session_id = use_signal(|| "ses_tracker_fsrs_01".to_string());
    let mut provider_filter = use_signal(|| "all".to_string());
    let mut search_query = use_signal(String::new);
    let mut since_window = use_signal(|| "7d".to_string());
    let mut allowlist_repos = use_signal(|| {
        vec![
            "/Users/pratham/code/tracker".to_string(),
            "/Users/pratham/code/bunfiiliate".to_string(),
            "/Users/pratham/code/learnpuppy".to_string(),
        ]
    });
    let mut new_repo_input = use_signal(String::new);
    let auto_fold_switch = use_signal(|| true);
    let mut selected_decisions = use_signal(|| {
        vec![
            "d_fsrs_deterministic_replay".to_string(),
            "d_card_length_tell_gate".to_string(),
        ]
    });
    let mut ingest_in_flight = use_signal(|| false);
    let mut ingest_banner = use_signal(|| None::<BackendResult<String>>);
    let mut fold_banner = use_signal(|| None::<String>);
    let mut folded_propositions_count = use_signal(|| 4usize);

    // Filtered FSRS deck memo
    let filtered_deck = use_memo(move || {
        let tf = track_filter();
        deck()
            .into_iter()
            .filter(|item| match tf.as_str() {
                "decision" => item.track == CardTrack::Decision,
                "rust-book" => item.track == CardTrack::RustBook,
                _ => true,
            })
            .collect::<Vec<_>>()
    });

    // Filtered CLI digest sessions memo
    let filtered_sessions = use_memo(move || {
        let pf = provider_filter();
        let q = search_query().trim().to_lowercase();
        sessions()
            .into_iter()
            .filter(|item| {
                let provider_ok = pf == "all" || item.session.source.as_str() == pf;
                let query_ok = q.is_empty()
                    || item.summary.to_lowercase().contains(&q)
                    || item.repo_name.to_lowercase().contains(&q)
                    || item.account_label.to_lowercase().contains(&q)
                    || item.decisions.iter().any(|d| {
                        d.title.to_lowercase().contains(&q)
                            || d.what.to_lowercase().contains(&q)
                            || d.why.to_lowercase().contains(&q)
                    });
                provider_ok && query_ok
            })
            .collect::<Vec<_>>()
    });

    let deck_items = filtered_deck();
    let deck_len = deck_items.len();
    let safe_card_idx = if deck_len == 0 {
        0
    } else {
        current_card_idx().min(deck_len - 1)
    };
    let active_card = deck_items.get(safe_card_idx).cloned();

    let completed_reviews = review_history().len();
    let total_cards = deck().len();
    let progress_pct = if total_cards == 0 {
        0.0
    } else {
        ((completed_reviews as f64 / total_cards as f64) * 100.0).clamp(0.0, 100.0)
    };

    let digest_list = filtered_sessions();
    let current_session = digest_list
        .iter()
        .find(|s| s.session.id.as_str() == selected_session_id())
        .or_else(|| digest_list.first())
        .cloned();

    // Handler: trigger async `backend.ingest(...)`
    let trigger_ingest = {
        let backend = backend.clone();
        move || {
            let allow_paths: Vec<PathBuf> =
                allowlist_repos().into_iter().map(PathBuf::from).collect();
            let since_spec = since_window();
            let since_ts = lp_ingest::since_ms(&since_spec, BASE_NOW_MS).unwrap_or(0);
            ingest_in_flight.set(true);
            ingest_banner.set(None);

            if let Some(backend_handle) = backend.clone() {
                spawn(async move {
                    let res = backend_handle.ingest(allow_paths, since_ts).await;
                    match res {
                        Ok(report) => {
                            if !report.stored.is_empty() {
                                let mut current = sessions.write();
                                for stored in &report.stored {
                                    if !current
                                        .iter()
                                        .any(|existing| existing.session.id == stored.id)
                                    {
                                        current.push(DigestSessionItem {
                                            session: stored.clone(),
                                            account_label: format!("{}:discovered", stored.source),
                                            repo_name: stored
                                                .cwd
                                                .rsplit('/')
                                                .next()
                                                .unwrap_or("repo")
                                                .to_string(),
                                            summary: format!(
                                                "Ingested {} transcript from {}",
                                                stored.source, stored.cwd
                                            ),
                                            decisions: Vec::new(),
                                            folded_to_graph: false,
                                            cards_minted: 0,
                                        });
                                    }
                                }
                            }
                            let msg = format!(
                                "Ingested {} files · {} sessions matched allowlist ({} window)",
                                report.files, report.sessions, since_spec
                            );
                            ingest_banner.set(Some(Ok(msg)));
                        }
                        Err(err) => {
                            ingest_banner.set(Some(Err(err)));
                        }
                    }
                    ingest_in_flight.set(false);
                });
            } else {
                ingest_in_flight.set(false);
                ingest_banner.set(Some(Ok(format!(
                    "Ingested 0 new files · {} local sessions ready ({since_spec} window)",
                    sessions().len()
                ))));
            }
        }
    };

    let mut trigger_ingest_btn = trigger_ingest.clone();

    rsx! {
        section {
            "aria-label": view.label(),
            "data-testid": format!("view-{}", view.id()),
            class: "flex flex-col",
            style: "gap: 1.25rem; height: 100%; min-height: 0;",

            // Top Header & Mode Switcher Bar
            div {
                class: "flex items-center justify-between",
                style: "gap: 1rem; flex-wrap: wrap; border-bottom: 1px solid var(--border); padding-bottom: 0.875rem;",
                div {
                    class: "flex items-center",
                    style: "gap: 0.75rem;",
                    IconDigest { size: 22, aria_label: "FSRS Review & CLI Digest" }
                    div {
                        h1 {
                            style: "margin: 0; font-size: 1.25rem; font-weight: 700;",
                            "{view.label()}"
                        }
                        p {
                            class: "text-muted-foreground",
                            style: "margin: 0.125rem 0 0 0; font-size: 0.8125rem;",
                            "FSRS v5 4-Choice Spaced Repetition Deck & Multi-Account CLI Decision Distiller"
                        }
                    }
                }

                // Quick Stats & Primary Actions
                div {
                    class: "flex items-center",
                    style: "gap: 0.625rem; flex-wrap: wrap;",
                    Badge {
                        variant: BadgeVariant::Phase,
                        test_id: "fsrs-due-badge",
                        aria_label: "Due flashcards count",
                        "Due: {deck_len} cards"
                    }
                    Badge {
                        variant: BadgeVariant::Secondary,
                        test_id: "fsrs-completed-badge",
                        aria_label: "Completed reviews in session",
                        "Reviewed: {completed_reviews}"
                    }
                    Badge {
                        variant: BadgeVariant::Success,
                        test_id: "digest-folded-badge",
                        aria_label: "Propositions folded into Knowledge Graph",
                        "Graph Folded: {folded_propositions_count()}"
                    }
                    Tooltip {
                        content: "Scan local Claude (#1/#2), Codex, and AGY transcripts",
                        shortcut: "⌘I",
                        test_id: "ingest-now-tooltip",
                        Button {
                            variant: ButtonVariant::Primary,
                            size: ButtonSize::Sm,
                            disabled: ingest_in_flight(),
                            aria_label: "Ingest Now",
                            test_id: "digest-ingest-now",
                            onclick: move |_| trigger_ingest_btn(),
                            IconRefresh { size: 15 }
                            if ingest_in_flight() {
                                span { "Ingesting…" }
                            } else {
                                span { "Ingest Now" }
                            }
                        }
                    }
                }
            }

            // Status / Error Alert Banner for Ingest or Graph Folding
            if let Some(outcome) = ingest_banner() {
                match outcome {
                    Ok(msg) => rsx! {
                        div {
                            role: "status",
                            "aria-label": "Ingest status",
                            "data-testid": "digest-ingest-status",
                            class: "flex items-center justify-between",
                            style: "padding: 0.625rem 0.875rem; border-radius: var(--radius); border: 1px solid var(--border); background-color: var(--secondary); font-size: 0.8125rem;",
                            div {
                                class: "flex items-center",
                                style: "gap: 0.5rem;",
                                IconCheck { size: 16 }
                                span { "{msg}" }
                            }
                            Button {
                                variant: ButtonVariant::Ghost,
                                size: ButtonSize::Xs,
                                aria_label: "Dismiss ingest status",
                                test_id: "digest-ingest-dismiss",
                                onclick: move |_| ingest_banner.set(None),
                                IconX { size: 14 }
                            }
                        }
                    },
                    Err(err) => rsx! {
                        div {
                            role: "alert",
                            "aria-label": "Ingest error",
                            "data-testid": "digest-ingest-error",
                            class: "flex items-center justify-between",
                            style: "padding: 0.625rem 0.875rem; border-radius: var(--radius); border: 1px solid var(--destructive); background-color: var(--destructive); color: var(--destructive-foreground); font-size: 0.8125rem;",
                            span { "Ingest failed: {err}" }
                            Button {
                                variant: ButtonVariant::Ghost,
                                size: ButtonSize::Xs,
                                aria_label: "Dismiss ingest error",
                                test_id: "digest-error-dismiss",
                                onclick: move |_| ingest_banner.set(None),
                                IconX { size: 14 }
                            }
                        }
                    },
                }
            }

            if let Some(fold_msg) = fold_banner() {
                div {
                    role: "status",
                    "aria-label": "Knowledge Graph fold status",
                    "data-testid": "digest-fold-status",
                    class: "flex items-center justify-between",
                    style: "padding: 0.625rem 0.875rem; border-radius: var(--radius); border: 1px solid var(--success); background-color: var(--card); font-size: 0.8125rem;",
                    div {
                        class: "flex items-center",
                        style: "gap: 0.5rem;",
                        IconGraph { size: 16 }
                        span { "{fold_msg}" }
                    }
                    div {
                        class: "flex items-center",
                        style: "gap: 0.5rem;",
                        Button {
                            variant: ButtonVariant::Outline,
                            size: ButtonSize::Xs,
                            aria_label: "Open Knowledge Graph",
                            test_id: "digest-jump-to-graph",
                            onclick: move |_| app_state.current_view.set(View::Graph),
                            "View in Graph"
                        }
                        Button {
                            variant: ButtonVariant::Ghost,
                            size: ButtonSize::Xs,
                            aria_label: "Dismiss fold notification",
                            test_id: "digest-fold-dismiss",
                            onclick: move |_| fold_banner.set(None),
                            IconX { size: 14 }
                        }
                    }
                }
            }

            // Dual-Mode Tabs (`FSRS Review Deck` | `CLI Transcript Digest`)
            Tabs {
                value: active_tab,
                tabs: vec!["review".to_string(), "digest".to_string()],
                test_id: "digest-fsrs-tabs",
                on_value_change: move |next_tab: String| {
                    match next_tab.as_str() {
                        "digest" => app_state.current_view.set(View::Digest),
                        _ => app_state.current_view.set(View::Review),
                    }
                },
                div {
                    class: "flex items-center justify-between",
                    style: "gap: 1rem; flex-wrap: wrap;",
                    TabsList {
                        aria_label: "FSRS Review and CLI Digest modes",
                        test_id: "digest-fsrs-tablist",
                        TabsTrigger {
                            value: "review",
                            aria_label: "FSRS 4-Choice Review Deck",
                            test_id: "tab-trigger-review",
                            div {
                                class: "inline-flex items-center",
                                style: "gap: 0.375rem;",
                                IconBrain { size: 15 }
                                span { "FSRS Review Deck ({deck_len})" }
                            }
                        }
                        TabsTrigger {
                            value: "digest",
                            aria_label: "Local CLI Transcript Decision Digest",
                            test_id: "tab-trigger-digest",
                            div {
                                class: "inline-flex items-center",
                                style: "gap: 0.375rem;",
                                IconTerminal { size: 15 }
                                span { "CLI Decision Digest ({digest_list.len()})" }
                            }
                        }
                    }

                    // Track filter pills & Custom Card creator toggle
                    div {
                        class: "flex items-center",
                        style: "gap: 0.5rem;",
                        div {
                            role: "group",
                            "aria-label": "Filter flashcards by track",
                            "data-testid": "fsrs-track-filter",
                            class: "inline-flex items-center",
                            style: "gap: 0.25rem;",
                            for (slug, label) in [
                                ("all", "All Tracks"),
                                ("decision", "Decision Track"),
                                ("rust-book", "Rust & Systems"),
                            ] {
                                Button {
                                    key: "{slug}",
                                    variant: if track_filter() == slug {
                                        ButtonVariant::Secondary
                                    } else {
                                        ButtonVariant::Ghost
                                    },
                                    size: ButtonSize::Xs,
                                    aria_label: format!("Filter deck by {label}"),
                                    test_id: format!("fsrs-filter-{slug}"),
                                    onclick: move |_| {
                                        track_filter.set(slug.to_string());
                                        current_card_idx.set(0);
                                        selected_option.set(None);
                                        revealed.set(false);
                                    },
                                    "{label}"
                                }
                            }
                        }
                        Separator { orientation: lp_ui::primitives::SeparatorOrientation::Vertical }
                        Button {
                            variant: ButtonVariant::Outline,
                            size: ButtonSize::Xs,
                            aria_label: "Toggle custom 4-choice card builder",
                            test_id: "fsrs-toggle-card-builder",
                            onclick: move |_| {
                                let open = custom_card_open();
                                custom_card_open.set(!open);
                            },
                            IconPlus { size: 14 }
                            span { "New 4-Choice Card" }
                        }
                    }
                }

                // Optional inline 4-Choice Card Builder Form (enforces validate_card_shape + prevent_default)
                if custom_card_open() {
                    UiCard {
                        aria_label: "Create 4-choice FSRS flashcard",
                        test_id: "fsrs-card-builder",
                        CardHeader {
                            CardTitle { "Mint Self-Contained 4-Choice FSRS Card" }
                            CardDescription {
                                "Validated deterministically by lp_core::fsrs::validate_card_shape (rejects context-dependent prompts and >1.6x option length tells)."
                            }
                        }
                        CardContent {
                            form {
                                "aria-label": "New flashcard form",
                                "data-testid": "fsrs-card-builder-form",
                                class: "flex flex-col",
                                style: "gap: 0.625rem;",
                                onsubmit: move |evt: FormEvent| {
                                    evt.prevent_default();
                                    let draft = lp_core::fsrs::CardDraft {
                                        front: custom_front(),
                                        right: custom_right(),
                                        wrong: [custom_wrong_1(), custom_wrong_2(), custom_wrong_3()],
                                    };
                                    match lp_core::fsrs::validate_card_shape(&draft) {
                                        Ok(()) => {
                                            let next_num = deck().len() + 1;
                                            let id = format!("c_custom_{next_num}");
                                            let new_item = DeckCardItem {
                                                flashcard: FlashCard {
                                                    id: id.into(),
                                                    front: draft.front,
                                                    options: [
                                                        draft.right,
                                                        draft.wrong[0].clone(),
                                                        draft.wrong[1].clone(),
                                                        draft.wrong[2].clone(),
                                                    ],
                                                    correct_index: 0,
                                                    rationale: if custom_rationale().trim().is_empty() {
                                                        "Verified via validate_card_shape quality gate.".into()
                                                    } else {
                                                        custom_rationale()
                                                    },
                                                    card: Card::new(BASE_NOW_MS),
                                                },
                                                kind_tag: "Custom · Decision".into(),
                                                track: CardTrack::Decision,
                                                source_label: "User Minted".into(),
                                                code_snippet: None,
                                            };
                                            deck.write().push(new_item);
                                            custom_front.set(String::new());
                                            custom_right.set(String::new());
                                            custom_wrong_1.set(String::new());
                                            custom_wrong_2.set(String::new());
                                            custom_wrong_3.set(String::new());
                                            custom_rationale.set(String::new());
                                            custom_card_status.set(Some(Ok("Card passed validate_card_shape and joined the deck.".into())));
                                        }
                                        Err(shape_err) => {
                                            custom_card_status.set(Some(Err(shape_err.to_string())));
                                        }
                                    }
                                },
                                input {
                                    r#type: "text",
                                    placeholder: "Self-contained question prompt (avoid 'in this project' without fenced code)",
                                    value: "{custom_front}",
                                    "aria-label": "Flashcard question prompt",
                                    "data-testid": "fsrs-builder-front",
                                    style: "padding: 0.5rem 0.75rem; border-radius: var(--radius); border: 1px solid var(--border); background: var(--background); color: var(--foreground); font-size: 0.8125rem;",
                                    oninput: move |evt| custom_front.set(evt.value()),
                                }
                                div {
                                    style: "display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 0.5rem;",
                                    input {
                                        r#type: "text",
                                        placeholder: "Correct option",
                                        value: "{custom_right}",
                                        "aria-label": "Correct option text",
                                        "data-testid": "fsrs-builder-right",
                                        style: "padding: 0.45rem 0.65rem; border-radius: var(--radius); border: 1px solid var(--border); background: var(--background); color: var(--foreground); font-size: 0.8125rem;",
                                        oninput: move |evt| custom_right.set(evt.value()),
                                    }
                                    input {
                                        r#type: "text",
                                        placeholder: "Distractor 1",
                                        value: "{custom_wrong_1}",
                                        "aria-label": "Distractor 1 text",
                                        "data-testid": "fsrs-builder-wrong-1",
                                        style: "padding: 0.45rem 0.65rem; border-radius: var(--radius); border: 1px solid var(--border); background: var(--background); color: var(--foreground); font-size: 0.8125rem;",
                                        oninput: move |evt| custom_wrong_1.set(evt.value()),
                                    }
                                    input {
                                        r#type: "text",
                                        placeholder: "Distractor 2",
                                        value: "{custom_wrong_2}",
                                        "aria-label": "Distractor 2 text",
                                        "data-testid": "fsrs-builder-wrong-2",
                                        style: "padding: 0.45rem 0.65rem; border-radius: var(--radius); border: 1px solid var(--border); background: var(--background); color: var(--foreground); font-size: 0.8125rem;",
                                        oninput: move |evt| custom_wrong_2.set(evt.value()),
                                    }
                                    input {
                                        r#type: "text",
                                        placeholder: "Distractor 3",
                                        value: "{custom_wrong_3}",
                                        "aria-label": "Distractor 3 text",
                                        "data-testid": "fsrs-builder-wrong-3",
                                        style: "padding: 0.45rem 0.65rem; border-radius: var(--radius); border: 1px solid var(--border); background: var(--background); color: var(--foreground); font-size: 0.8125rem;",
                                        oninput: move |evt| custom_wrong_3.set(evt.value()),
                                    }
                                }
                                div {
                                    class: "flex items-center justify-between",
                                    style: "gap: 0.5rem;",
                                    input {
                                        r#type: "text",
                                        placeholder: "Rationale explanation (shown after option selection)",
                                        value: "{custom_rationale}",
                                        "aria-label": "Flashcard rationale",
                                        "data-testid": "fsrs-builder-rationale",
                                        style: "flex: 1; padding: 0.45rem 0.65rem; border-radius: var(--radius); border: 1px solid var(--border); background: var(--background); color: var(--foreground); font-size: 0.8125rem;",
                                        oninput: move |evt| custom_rationale.set(evt.value()),
                                    }
                                    Button {
                                        button_type: "submit",
                                        variant: ButtonVariant::Primary,
                                        size: ButtonSize::Sm,
                                        aria_label: "Validate and add card",
                                        test_id: "fsrs-builder-submit",
                                        IconSparkles { size: 14 }
                                        span { "Validate & Add" }
                                    }
                                }
                                if let Some(status) = custom_card_status() {
                                    match status {
                                        Ok(ok_msg) => rsx! {
                                            p {
                                                role: "status",
                                                "data-testid": "fsrs-builder-ok",
                                                style: "margin: 0; font-size: 0.75rem; color: var(--success);",
                                                "{ok_msg}"
                                            }
                                        },
                                        Err(err_msg) => rsx! {
                                            p {
                                                role: "alert",
                                                "data-testid": "fsrs-builder-error",
                                                style: "margin: 0; font-size: 0.75rem; color: var(--destructive);",
                                                "Shape Gate Rejected: {err_msg}"
                                            }
                                        },
                                    }
                                }
                            }
                        }
                    }
                }

                // ====================================================================
                // TAB 1: 4-CHOICE VISUAL FSRS REVIEW DECK
                // ====================================================================
                TabsContent {
                    value: "review",
                    force_mount: true,
                    test_id: "tab-content-review",
                    div {
                        class: "flex flex-col",
                        style: "gap: 1rem;",

                        // Deck Progress Header
                        div {
                            class: "flex items-center justify-between",
                            style: "gap: 1rem;",
                            div {
                                class: "flex items-center",
                                style: "gap: 0.75rem; flex: 1;",
                                span {
                                    class: "text-muted-foreground",
                                    style: "font-size: 0.75rem; font-weight: 600; white-space: nowrap;",
                                    "Deck Progress ({ safe_card_idx + usize::from(deck_len > 0) } / {deck_len})"
                                }
                                Progress {
                                    value: progress_pct,
                                    max: 100.0,
                                    aria_label: "FSRS review session progress",
                                    test_id: "fsrs-session-progress",
                                }
                            }

                            // Card Navigation Controls
                            div {
                                class: "flex items-center",
                                style: "gap: 0.375rem;",
                                Button {
                                    variant: ButtonVariant::Outline,
                                    size: ButtonSize::Xs,
                                    disabled: safe_card_idx == 0,
                                    aria_label: "Previous flashcard",
                                    test_id: "fsrs-prev-card",
                                    onclick: move |_| {
                                        if safe_card_idx > 0 {
                                            current_card_idx.set(safe_card_idx - 1);
                                            selected_option.set(None);
                                            revealed.set(false);
                                        }
                                    },
                                    "Prev"
                                }
                                Button {
                                    variant: ButtonVariant::Outline,
                                    size: ButtonSize::Xs,
                                    disabled: safe_card_idx + 1 >= deck_len,
                                    aria_label: "Next flashcard",
                                    test_id: "fsrs-next-card",
                                    onclick: move |_| {
                                        if safe_card_idx + 1 < deck_len {
                                            current_card_idx.set(safe_card_idx + 1);
                                            selected_option.set(None);
                                            revealed.set(false);
                                        }
                                    },
                                    "Next"
                                }
                            }
                        }

                        // Active Flashcard Surface
                        if let Some(card_item) = active_card {
                            {
                                let fc = card_item.flashcard.clone();
                                let card_id_str = fc.id.to_string();
                                let intervals = preview(Some(&fc.card), BASE_NOW_MS);
                                let state_str = fsrs_state_label(fc.card.state);
                                let mastery = card_item.mastery_score();
                                let chosen = selected_option();
                                let is_revealed = revealed();

                                rsx! {
                                    UiCard {
                                        aria_label: format!("FSRS Flashcard {}", fc.id),
                                        test_id: "fsrs-active-card",
                                        CardHeader {
                                            div {
                                                class: "flex items-center justify-between",
                                                style: "gap: 1rem; flex-wrap: wrap;",
                                                div {
                                                    class: "flex items-center",
                                                    style: "gap: 0.5rem; flex-wrap: wrap;",
                                                    Badge {
                                                        variant: BadgeVariant::Phase,
                                                        test_id: "fsrs-card-kind",
                                                        "{card_item.kind_tag}"
                                                    }
                                                    Badge {
                                                        variant: BadgeVariant::Outline,
                                                        test_id: "fsrs-card-track",
                                                        "{card_item.track.label()}"
                                                    }
                                                    Badge {
                                                        variant: match fc.card.state {
                                                            FsrsState::Review => BadgeVariant::Success,
                                                            FsrsState::Learning | FsrsState::Relearning => BadgeVariant::Warning,
                                                            FsrsState::New => BadgeVariant::Secondary,
                                                        },
                                                        test_id: "fsrs-card-state",
                                                        "State: {state_str}"
                                                    }
                                                    span {
                                                        class: "text-muted-foreground",
                                                        style: "font-size: 0.75rem; font-family: var(--font-mono);",
                                                        "S={fc.card.stability:.1}d · D={fc.card.difficulty:.1} · Reps={fc.card.reps} · Lapses={fc.card.lapses}"
                                                    }
                                                }
                                                div {
                                                    class: "flex items-center",
                                                    style: "gap: 0.625rem;",
                                                    span {
                                                        class: "text-muted-foreground",
                                                        style: "font-size: 0.75rem;",
                                                        "{card_item.source_label}"
                                                    }
                                                    MasteryRing {
                                                        score: mastery,
                                                        size: 40,
                                                        aria_label: format!("Card mastery {:.0}%", mastery * 100.0),
                                                        test_id: "fsrs-card-mastery-ring",
                                                    }
                                                }
                                            }
                                            CardTitle {
                                                test_id: "fsrs-card-front",
                                                style: "margin-top: 0.5rem; font-size: 1.05rem; line-height: 1.5;",
                                                "{fc.front}"
                                            }
                                        }

                                        CardContent {
                                            div {
                                                class: "flex flex-col",
                                                style: "gap: 0.875rem;",

                                                if let Some(snippet) = &card_item.code_snippet {
                                                    CodeBlock {
                                                        code: snippet.clone(),
                                                        language: "rust",
                                                        title: card_item.source_label.clone(),
                                                        test_id: "fsrs-card-code",
                                                    }
                                                }

                                                // 4-Choice Option Cards ([A]..[D])
                                                div {
                                                    role: "radiogroup",
                                                    "aria-label": "Multiple choice answer options",
                                                    "data-testid": "fsrs-options-group",
                                                    style: "display: grid; grid-template-columns: repeat(1, minmax(0, 1fr)); gap: 0.625rem;",
                                                    for (opt_idx, opt_text) in fc.options.iter().enumerate() {
                                                        {
                                                            let idx_u8 = opt_idx as u8;
                                                            let letter = OPTION_LETTERS[opt_idx];
                                                            let is_selected = chosen == Some(idx_u8);
                                                            let is_correct = fc.correct_index == idx_u8;

                                                            let border_style = if is_revealed {
                                                                if is_correct {
                                                                    "border: 1.5px solid var(--success); background-color: oklch(0.22 0.04 155);"
                                                                } else if is_selected {
                                                                    "border: 1.5px solid var(--destructive); background-color: oklch(0.22 0.04 25);"
                                                                } else {
                                                                    "border: 1px solid var(--border); background-color: var(--background);"
                                                                }
                                                            } else if is_selected {
                                                                "border: 1.5px solid var(--primary); background-color: var(--accent);"
                                                            } else {
                                                                "border: 1px solid var(--border); background-color: var(--background);"
                                                            };

                                                            rsx! {
                                                                button {
                                                                    key: "{letter}",
                                                                    r#type: "button",
                                                                    role: "radio",
                                                                    "aria-checked": if is_selected { "true" } else { "false" },
                                                                    "aria-label": format!("Option {letter}: {opt_text}"),
                                                                    "data-testid": format!("fsrs-option-{letter}"),
                                                                    "data-correct": if is_revealed && is_correct { "true" } else { "false" },
                                                                    class: "flex items-center justify-between",
                                                                    style: "width: 100%; text-align: left; padding: 0.75rem 1rem; border-radius: var(--radius); color: var(--foreground); cursor: pointer; gap: 0.75rem; {border_style}",
                                                                    onclick: move |_| {
                                                                        selected_option.set(Some(idx_u8));
                                                                        revealed.set(true);
                                                                    },
                                                                    div {
                                                                        class: "flex items-center",
                                                                        style: "gap: 0.75rem;",
                                                                        Badge {
                                                                            variant: if is_revealed && is_correct {
                                                                                BadgeVariant::Success
                                                                            } else if is_revealed && is_selected {
                                                                                BadgeVariant::Destructive
                                                                            } else if is_selected {
                                                                                BadgeVariant::Default
                                                                            } else {
                                                                                BadgeVariant::Secondary
                                                                            },
                                                                            test_id: format!("fsrs-option-badge-{letter}"),
                                                                            "{letter}"
                                                                        }
                                                                        span {
                                                                            style: "font-size: 0.875rem; line-height: 1.4;",
                                                                            "{opt_text}"
                                                                        }
                                                                    }
                                                                    if is_revealed && is_correct {
                                                                        IconCheck { size: 18, aria_label: "Correct answer" }
                                                                    } else if is_revealed && is_selected && !is_correct {
                                                                        IconX { size: 18, aria_label: "Incorrect selection" }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }

                                                // Rationale Reveal Banner
                                                if is_revealed {
                                                    {
                                                        let answered_correctly = chosen == Some(fc.correct_index);
                                                        let correct_letter = OPTION_LETTERS[usize::from(fc.correct_index.min(3))];
                                                        rsx! {
                                                            div {
                                                                role: "status",
                                                                "aria-label": "Answer rationale",
                                                                "data-testid": "fsrs-rationale-banner",
                                                                style: "padding: 0.875rem 1rem; border-radius: var(--radius); border: 1px solid var(--border); background-color: var(--muted);",
                                                                div {
                                                                    class: "flex items-center",
                                                                    style: "gap: 0.5rem; margin-bottom: 0.375rem;",
                                                                    Badge {
                                                                        variant: if answered_correctly {
                                                                            BadgeVariant::Success
                                                                        } else {
                                                                            BadgeVariant::Warning
                                                                        },
                                                                        test_id: "fsrs-verdict-badge",
                                                                        if answered_correctly {
                                                                            "Correct ([{correct_letter}])"
                                                                        } else {
                                                                            "Review Rationale · Correct is [{correct_letter}]"
                                                                        }
                                                                    }
                                                                }
                                                                p {
                                                                    style: "margin: 0; font-size: 0.84rem; line-height: 1.5;",
                                                                    "{fc.rationale}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                } else {
                                                    div {
                                                        class: "flex items-center justify-between",
                                                        Button {
                                                            variant: ButtonVariant::Ghost,
                                                            size: ButtonSize::Xs,
                                                            aria_label: "Reveal answer rationale without selecting an option",
                                                            test_id: "fsrs-reveal-rationale",
                                                            onclick: move |_| revealed.set(true),
                                                            "Show Rationale"
                                                        }
                                                    }
                                                }
                                            }
                                        }

                                        // 1-Click FSRS v5 Grading Bar (Again / Hard / Good / Easy)
                                        CardFooter {
                                            class: "flex items-center justify-between",
                                            style: "flex-wrap: wrap; gap: 0.75rem; border-top: 1px solid var(--border); padding-top: 1rem;",
                                            span {
                                                class: "text-muted-foreground",
                                                style: "font-size: 0.75rem;",
                                                "Grade recall with FSRS v5 (updates stability, difficulty & next due interval):"
                                            }
                                            div {
                                                role: "group",
                                                "aria-label": "FSRS grading buttons",
                                                "data-testid": "fsrs-grade-group",
                                                class: "flex items-center",
                                                style: "gap: 0.5rem; flex-wrap: wrap;",
                                                for (r_idx, rating) in [
                                                    Rating::Again,
                                                    Rating::Hard,
                                                    Rating::Good,
                                                    Rating::Easy,
                                                ]
                                                .into_iter()
                                                .enumerate()
                                                {
                                                    {
                                                        let ivl_label = format_interval_ms(intervals[r_idx]);
                                                        let r_label = rating_label(rating);
                                                        let r_slug = rating_slug(rating);
                                                        let btn_variant = match rating {
                                                            Rating::Again => ButtonVariant::Destructive,
                                                            Rating::Hard => ButtonVariant::Outline,
                                                            Rating::Good => ButtonVariant::Primary,
                                                            Rating::Easy => ButtonVariant::Secondary,
                                                        };
                                                        let cid = card_id_str.clone();

                                                        rsx! {
                                                            Button {
                                                                key: "{r_slug}",
                                                                variant: btn_variant,
                                                                size: ButtonSize::Sm,
                                                                aria_label: format!("Grade {r_label} ({ivl_label})"),
                                                                test_id: format!("fsrs-grade-{r_slug}"),
                                                                onclick: move |_| {
                                                                    let now_ts = BASE_NOW_MS + (review_history().len() as i64 + 1) * 60_000;
                                                                    let mut next_interval_ms = 0i64;
                                                                    {
                                                                        let mut all_cards = deck.write();
                                                                        if let Some(target) = all_cards.iter_mut().find(|c| c.flashcard.id.as_str() == cid) {
                                                                            let updated = review(&target.flashcard.card, now_ts, rating);
                                                                            next_interval_ms = (updated.due - now_ts).max(60_000);
                                                                            target.flashcard.card = updated;
                                                                        }
                                                                    }
                                                                    review_history.write().push((cid.clone(), rating, next_interval_ms));
                                                                    selected_option.set(None);
                                                                    revealed.set(false);
                                                                    if safe_card_idx + 1 < deck_len {
                                                                        current_card_idx.set(safe_card_idx + 1);
                                                                    }
                                                                },
                                                                span { "{r_label}" }
                                                                Badge {
                                                                    variant: BadgeVariant::Outline,
                                                                    test_id: format!("fsrs-interval-{r_slug}"),
                                                                    "{ivl_label}"
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        } else {
                            UiCard {
                                aria_label: "Empty FSRS review deck",
                                test_id: "fsrs-empty-deck",
                                CardContent {
                                    p {
                                        role: "status",
                                        "No flashcards match the current track filter."
                                    }
                                }
                            }
                        }

                        // Recent FSRS Review Log Strip
                        if !review_history().is_empty() {
                            div {
                                role: "region",
                                "aria-label": "Recent FSRS review log",
                                "data-testid": "fsrs-review-log",
                                class: "flex items-center",
                                style: "gap: 0.5rem; flex-wrap: wrap; font-size: 0.75rem;",
                                span { class: "text-muted-foreground", "Session Reviews:" }
                                for (idx, (cid, r, ivl)) in review_history().into_iter().enumerate() {
                                    Badge {
                                        key: "{idx}-{cid}",
                                        variant: BadgeVariant::Secondary,
                                        test_id: format!("fsrs-log-entry-{idx}"),
                                        "{cid}: {rating_label(r)} → {format_interval_ms(ivl)}"
                                    }
                                }
                            }
                        }
                    }
                }

                // ====================================================================
                // TAB 2: LOCAL CLI TRANSCRIPT DECISION DIGEST BROWSER
                // ====================================================================
                TabsContent {
                    value: "digest",
                    force_mount: true,
                    test_id: "tab-content-digest",
                    div {
                        class: "flex flex-col",
                        style: "gap: 1rem;",

                        // Digest Filter, Repo Allowlist & Ingest Controls Bar
                        UiCard {
                            aria_label: "CLI Transcript Ingestion Controls",
                            test_id: "digest-controls-card",
                            CardContent {
                                style: "padding-top: 1rem;",
                                div {
                                    class: "flex flex-col",
                                    style: "gap: 0.75rem;",
                                    div {
                                        class: "flex items-center justify-between",
                                        style: "gap: 0.75rem; flex-wrap: wrap;",

                                        // Provider Filter Pills (`All`, `claude`, `codex`, `agy`)
                                        div {
                                            role: "group",
                                            "aria-label": "Filter sessions by CLI provider",
                                            "data-testid": "digest-provider-filter",
                                            class: "flex items-center",
                                            style: "gap: 0.25rem;",
                                            IconFilter { size: 15 }
                                            for (prov_id, prov_label) in [
                                                ("all", "All Agents"),
                                                ("claude", "Claude (#1 & #2)"),
                                                ("codex", "Codex"),
                                                ("agy", "Antigravity (AGY)"),
                                            ] {
                                                Button {
                                                    key: "{prov_id}",
                                                    variant: if provider_filter() == prov_id {
                                                        ButtonVariant::Secondary
                                                    } else {
                                                        ButtonVariant::Ghost
                                                    },
                                                    size: ButtonSize::Xs,
                                                    aria_label: format!("Filter sessions by {prov_label}"),
                                                    test_id: format!("digest-provider-{prov_id}"),
                                                    onclick: move |_| provider_filter.set(prov_id.to_string()),
                                                    "{prov_label}"
                                                }
                                            }
                                        }

                                        // Since Window Selector (`24h`, `7d`, `4w`) + Auto-fold Switch
                                        div {
                                            class: "flex items-center",
                                            style: "gap: 0.75rem; flex-wrap: wrap;",
                                            div {
                                                role: "group",
                                                "aria-label": "Ingestion time window",
                                                "data-testid": "digest-since-group",
                                                class: "inline-flex items-center",
                                                style: "gap: 0.25rem;",
                                                for window in ["24h", "7d", "4w"] {
                                                    Button {
                                                        key: "{window}",
                                                        variant: if since_window() == window {
                                                            ButtonVariant::Secondary
                                                        } else {
                                                            ButtonVariant::Ghost
                                                        },
                                                        size: ButtonSize::Xs,
                                                        aria_label: format!("Since window {window}"),
                                                        test_id: format!("digest-since-{window}"),
                                                        onclick: move |_| since_window.set(window.to_string()),
                                                        "--since {window}"
                                                    }
                                                }
                                            }
                                            Switch {
                                                checked_signal: auto_fold_switch,
                                                aria_label: "Auto-fold distilled decisions into Knowledge Graph",
                                                label: "Auto-fold to Graph",
                                                test_id: "digest-auto-fold-switch",
                                            }
                                        }
                                    }

                                    // Search Form & Repo Allowlist Form (both call evt.prevent_default())
                                    div {
                                        class: "flex items-center justify-between",
                                        style: "gap: 0.75rem; flex-wrap: wrap;",
                                        form {
                                            role: "search",
                                            "aria-label": "Search CLI transcripts and decisions",
                                            "data-testid": "digest-search-form",
                                            class: "flex items-center",
                                            style: "gap: 0.5rem; flex: 1; min-width: 14rem;",
                                            onsubmit: move |evt: FormEvent| {
                                                evt.prevent_default();
                                            },
                                            IconSearch { size: 15 }
                                            input {
                                                r#type: "search",
                                                placeholder: "Filter decisions by title, tradeoff, file, or repo…",
                                                value: "{search_query}",
                                                "aria-label": "Search decisions query",
                                                "data-testid": "digest-search-input",
                                                style: "flex: 1; padding: 0.4rem 0.65rem; border-radius: var(--radius); border: 1px solid var(--border); background: var(--background); color: var(--foreground); font-size: 0.8125rem;",
                                                oninput: move |evt| search_query.set(evt.value()),
                                            }
                                        }

                                        form {
                                            "aria-label": "Add repository to ingestion allowlist",
                                            "data-testid": "digest-allowlist-form",
                                            class: "flex items-center",
                                            style: "gap: 0.375rem;",
                                            onsubmit: move |evt: FormEvent| {
                                                evt.prevent_default();
                                                let candidate = new_repo_input().trim().to_string();
                                                if !candidate.is_empty() && !allowlist_repos().contains(&candidate) {
                                                    allowlist_repos.write().push(candidate);
                                                    new_repo_input.set(String::new());
                                                }
                                            },
                                            input {
                                                r#type: "text",
                                                placeholder: "/Users/pratham/code/new-repo",
                                                value: "{new_repo_input}",
                                                "aria-label": "Allowlist repository path",
                                                "data-testid": "digest-allowlist-input",
                                                style: "padding: 0.4rem 0.65rem; border-radius: var(--radius); border: 1px solid var(--border); background: var(--background); color: var(--foreground); font-size: 0.75rem; font-family: var(--font-mono);",
                                                oninput: move |evt| new_repo_input.set(evt.value()),
                                            }
                                            Button {
                                                button_type: "submit",
                                                variant: ButtonVariant::Outline,
                                                size: ButtonSize::Xs,
                                                aria_label: "Add repo to allowlist",
                                                test_id: "digest-allowlist-add",
                                                IconPlus { size: 13 }
                                                span { "Add Repo" }
                                            }
                                        }
                                    }

                                    // Allowlist Repo Badges
                                    div {
                                        class: "flex items-center",
                                        style: "gap: 0.375rem; flex-wrap: wrap;",
                                        span {
                                            class: "text-muted-foreground",
                                            style: "font-size: 0.75rem;",
                                            "Allowlisted Repos:"
                                        }
                                        for repo in allowlist_repos() {
                                            Badge {
                                                key: "{repo}",
                                                variant: BadgeVariant::Outline,
                                                test_id: "digest-allowlist-badge",
                                                "{repo}"
                                            }
                                        }
                                    }
                                }
                            }
                        }

                        // SplitPane: Left Ingested CLI Sessions List | Right Distilled Decisions Browser
                        div {
                            style: "height: 32rem; min-height: 22rem;",
                            SplitPane {
                                direction: SplitDirection::Horizontal,
                                initial_ratio: 0.36,
                                aria_label: "Resize CLI sessions list and decision inspector",
                                test_id: "digest-split-pane",
                                first: rsx! {
                                    ScrollArea {
                                        aria_label: "Ingested CLI coding sessions",
                                        test_id: "digest-sessions-scroll",
                                        max_height: "100%",
                                        div {
                                            role: "listbox",
                                            "aria-label": "Ingested CLI sessions",
                                            "data-testid": "digest-session-list",
                                            class: "flex flex-col",
                                            style: "gap: 0.5rem; padding-right: 0.5rem;",
                                            if digest_list.is_empty() {
                                                p {
                                                    role: "status",
                                                    "data-testid": "digest-sessions-empty",
                                                    class: "text-muted-foreground",
                                                    "No CLI transcripts match the current filter."
                                                }
                                            } else {
                                                for item in &digest_list {
                                                    {
                                                        let sid = item.session.id.to_string();
                                                        let is_selected = current_session
                                                            .as_ref()
                                                            .map(|c| c.session.id == item.session.id)
                                                            .unwrap_or(false);
                                                        let card_border = if is_selected {
                                                            "border: 1.5px solid var(--primary); background-color: var(--accent);"
                                                        } else {
                                                            "border: 1px solid var(--border); background-color: var(--card);"
                                                        };

                                                        rsx! {
                                                            button {
                                                                key: "{sid}",
                                                                r#type: "button",
                                                                role: "option",
                                                                "aria-selected": if is_selected { "true" } else { "false" },
                                                                "aria-label": format!("Session {} in {}", item.session.id, item.repo_name),
                                                                "data-testid": format!("digest-session-item-{sid}"),
                                                                style: "width: 100%; text-align: left; padding: 0.75rem; border-radius: var(--radius); color: var(--foreground); cursor: pointer; {card_border}",
                                                                onclick: move |_| {
                                                                    selected_session_id.set(sid.clone());
                                                                },
                                                                div {
                                                                    class: "flex items-center justify-between",
                                                                    style: "gap: 0.5rem; margin-bottom: 0.375rem;",
                                                                    Badge {
                                                                        variant: match item.session.source.as_str() {
                                                                            "claude" => BadgeVariant::Default,
                                                                            "codex" => BadgeVariant::Secondary,
                                                                            _ => BadgeVariant::Phase,
                                                                        },
                                                                        test_id: format!("digest-session-source-{sid}"),
                                                                        "{item.account_label}"
                                                                    }
                                                                    if item.folded_to_graph {
                                                                        Badge {
                                                                            variant: BadgeVariant::Success,
                                                                            test_id: format!("digest-session-folded-{sid}"),
                                                                            "Folded"
                                                                        }
                                                                    }
                                                                }
                                                                div {
                                                                    style: "font-weight: 600; font-size: 0.85rem; margin-bottom: 0.25rem;",
                                                                    "{item.summary}"
                                                                }
                                                                div {
                                                                    class: "flex items-center justify-between text-muted-foreground",
                                                                    style: "font-size: 0.75rem; font-family: var(--font-mono);",
                                                                    span { "{item.repo_name} ({item.session.git_branch.as_deref().unwrap_or(\"detached\")})" }
                                                                    span { "{item.decisions.len()} decisions" }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                },
                                second: rsx! {
                                    ScrollArea {
                                        aria_label: "Distilled architectural decisions",
                                        test_id: "digest-decisions-scroll",
                                        max_height: "100%",
                                        if let Some(session_item) = current_session {
                                            {
                                                let sid = session_item.session.id.to_string();
                                                let sid_for_fold = sid.clone();
                                                let sid_for_mint = sid.clone();
                                                let decisions_for_fold = session_item.decisions.clone();
                                                let decisions_for_mint = session_item.decisions.clone();

                                                rsx! {
                                                    div {
                                                        class: "flex flex-col",
                                                        style: "gap: 0.875rem; padding-left: 0.75rem;",

                                                        // Session Detail Header + 1-Click "Fold into Graph" & "Mint Cards"
                                                        UiCard {
                                                            aria_label: "Selected CLI session header",
                                                            test_id: "digest-session-detail",
                                                            CardHeader {
                                                                div {
                                                                    class: "flex items-center justify-between",
                                                                    style: "gap: 0.75rem; flex-wrap: wrap;",
                                                                    div {
                                                                        CardTitle { "{session_item.summary}" }
                                                                        CardDescription {
                                                                            "Source: {session_item.account_label} · Repo: {session_item.session.cwd} · Branch: {session_item.session.git_branch.as_deref().unwrap_or(\"main\")}"
                                                                        }
                                                                    }
                                                                    div {
                                                                        class: "flex items-center",
                                                                        style: "gap: 0.5rem; flex-wrap: wrap;",
                                                                        Button {
                                                                            variant: ButtonVariant::Primary,
                                                                            size: ButtonSize::Sm,
                                                                            aria_label: "Fold into Graph",
                                                                            test_id: "digest-fold-into-graph",
                                                                            onclick: move |_| {
                                                                                let folded_count = decisions_for_fold.len().max(1);
                                                                                {
                                                                                    let mut list = sessions.write();
                                                                                    if let Some(target) = list.iter_mut().find(|s| s.session.id.as_str() == sid_for_fold) {
                                                                                        target.folded_to_graph = true;
                                                                                    }
                                                                                }
                                                                                folded_propositions_count.set(folded_propositions_count() + folded_count);
                                                                                fold_banner.set(Some(format!(
                                                                                    "Folded {folded_count} DecisionRecord(s) from {sid_for_fold} into Knowledge Graph via Extractor → Auditor → simplify_chains."
                                                                                )));
                                                                            },
                                                                            IconGraph { size: 15 }
                                                                            span { "Fold into Graph" }
                                                                        }
                                                                        Button {
                                                                            variant: ButtonVariant::Secondary,
                                                                            size: ButtonSize::Sm,
                                                                            aria_label: "Mint 4-choice FSRS cards from decisions",
                                                                            test_id: "digest-mint-cards",
                                                                            onclick: move |_| {
                                                                                let mut minted = 0usize;
                                                                                {
                                                                                    let mut current_deck = deck.write();
                                                                                    for d in &decisions_for_mint {
                                                                                        let card_id = format!("c_mint_{}", d.id);
                                                                                        if !current_deck.iter().any(|c| c.flashcard.id.as_str() == card_id) {
                                                                                            current_deck.push(DeckCardItem {
                                                                                                flashcard: FlashCard {
                                                                                                    id: card_id.into(),
                                                                                                    front: format!("Why was `{}` chosen over alternatives ({})?", d.title, d.files.first().cloned().unwrap_or_else(|| "module".into())),
                                                                                                    options: [
                                                                                                        d.why.clone(),
                                                                                                        d.alternatives.first().map(|a| a.rejected_because.clone()).unwrap_or_else(|| "To avoid compile-time borrow checking".into()),
                                                                                                        "To disable SQLite WAL journaling across threads".into(),
                                                                                                        "To bypass schema validation during JSONL compaction".into(),
                                                                                                    ],
                                                                                                    correct_index: 0,
                                                                                                    rationale: format!("{} — Risk noted: {}", d.what, d.risk),
                                                                                                    card: Card::new(BASE_NOW_MS),
                                                                                                },
                                                                                                kind_tag: "Why · Decision".into(),
                                                                                                track: CardTrack::Decision,
                                                                                                source_label: format!("Distilled · {}", d.id),
                                                                                                code_snippet: None,
                                                                                            });
                                                                                            minted += 1;
                                                                                        }
                                                                                    }
                                                                                }
                                                                                {
                                                                                    let mut list = sessions.write();
                                                                                    if let Some(target) = list.iter_mut().find(|s| s.session.id.as_str() == sid_for_mint) {
                                                                                        target.cards_minted += minted;
                                                                                    }
                                                                                }
                                                                                fold_banner.set(Some(format!(
                                                                                    "Minted {minted} new 4-choice FSRS card(s) from session {sid_for_mint}."
                                                                                )));
                                                                            },
                                                                            IconSparkles { size: 15 }
                                                                            span { "Mint Cards ({session_item.cards_minted})" }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                            CardContent {
                                                                CodeBlock {
                                                                    code: session_item.session.transcript.clone(),
                                                                    language: "text",
                                                                    title: format!("Transcript Preview · {}", session_item.session.id),
                                                                    show_line_numbers: true,
                                                                    test_id: "digest-transcript-preview",
                                                                }
                                                            }
                                                        }

                                                        // Distilled DecisionRecord Cards
                                                        for decision in &session_item.decisions {
                                                            {
                                                                let did = decision.id.to_string();
                                                                let did_check = did.clone();
                                                                let did_fold = did.clone();
                                                                let decision_title = decision.title.clone();
                                                                let is_checked = selected_decisions().contains(&did);
                                                                let basis_label = match decision.basis {
                                                                    DecisionBasis::Stated => "Stated",
                                                                    DecisionBasis::Inferred => "Inferred",
                                                                };

                                                                rsx! {
                                                                    UiCard {
                                                                        key: "{did}",
                                                                        aria_label: format!("Decision record {}", decision.title),
                                                                        test_id: format!("digest-decision-card-{did}"),
                                                                        CardHeader {
                                                                            div {
                                                                                class: "flex items-center justify-between",
                                                                                style: "gap: 0.75rem; flex-wrap: wrap;",
                                                                                div {
                                                                                    class: "flex items-center",
                                                                                    style: "gap: 0.5rem;",
                                                                                    Checkbox {
                                                                                        checked: is_checked,
                                                                                        aria_label: format!("Select decision {}", decision.title),
                                                                                        test_id: format!("digest-decision-checkbox-{did}"),
                                                                                        on_change: move |next: bool| {
                                                                                            let mut sel = selected_decisions.write();
                                                                                            if next {
                                                                                                if !sel.contains(&did_check) {
                                                                                                    sel.push(did_check.clone());
                                                                                                }
                                                                                            } else {
                                                                                                sel.retain(|x| x != &did_check);
                                                                                            }
                                                                                        },
                                                                                    }
                                                                                    CardTitle { "{decision.title}" }
                                                                                }
                                                                                div {
                                                                                    class: "flex items-center",
                                                                                    style: "gap: 0.375rem;",
                                                                                    Badge {
                                                                                        variant: match decision.basis {
                                                                                            DecisionBasis::Stated => BadgeVariant::Success,
                                                                                            DecisionBasis::Inferred => BadgeVariant::Warning,
                                                                                        },
                                                                                        test_id: format!("digest-decision-basis-{did}"),
                                                                                        "Basis: {basis_label}"
                                                                                    }
                                                                                    if let Some(sha) = &decision.commit_sha {
                                                                                        Badge {
                                                                                            variant: BadgeVariant::Outline,
                                                                                            test_id: format!("digest-decision-sha-{did}"),
                                                                                            "commit {sha}"
                                                                                        }
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                        CardContent {
                                                                            div {
                                                                                class: "flex flex-col",
                                                                                style: "gap: 0.5rem; font-size: 0.84rem; line-height: 1.45;",
                                                                                p {
                                                                                    style: "margin: 0;",
                                                                                    strong { "What: " }
                                                                                    "{decision.what}"
                                                                                }
                                                                                p {
                                                                                    style: "margin: 0;",
                                                                                    strong { "Why (Tradeoff): " }
                                                                                    "{decision.why}"
                                                                                }
                                                                                if !decision.alternatives.is_empty() {
                                                                                    div {
                                                                                        "data-testid": format!("digest-decision-alternatives-{did}"),
                                                                                        strong { "Rejected Alternatives:" }
                                                                                        ul {
                                                                                            style: "margin: 0.25rem 0 0 1.25rem; padding: 0;",
                                                                                            for alt in &decision.alternatives {
                                                                                                li {
                                                                                                    key: "{alt.option}",
                                                                                                    strong { "{alt.option}" }
                                                                                                    ": {alt.rejected_because}"
                                                                                                }
                                                                                            }
                                                                                        }
                                                                                    }
                                                                                }
                                                                                p {
                                                                                    style: "margin: 0;",
                                                                                    strong { "Risk: " }
                                                                                    "{decision.risk}"
                                                                                }
                                                                                div {
                                                                                    class: "flex items-center",
                                                                                    style: "gap: 0.375rem; flex-wrap: wrap;",
                                                                                    for file in &decision.files {
                                                                                        Badge {
                                                                                            key: "{file}",
                                                                                            variant: BadgeVariant::Secondary,
                                                                                            test_id: format!("digest-decision-file-{did}"),
                                                                                            "{file}"
                                                                                        }
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                        CardFooter {
                                                                            class: "flex items-center justify-between",
                                                                            Button {
                                                                                variant: ButtonVariant::Outline,
                                                                                size: ButtonSize::Xs,
                                                                                aria_label: format!("Fold decision {} into Graph", decision.title),
                                                                                test_id: format!("digest-fold-single-{did}"),
                                                                                onclick: move |_| {
                                                                                    folded_propositions_count.set(folded_propositions_count() + 1);
                                                                                    fold_banner.set(Some(format!(
                                                                                        "Folded decision `{decision_title}` ({did_fold}) into Knowledge Graph."
                                                                                    )));
                                                                                },
                                                                                IconGraph { size: 13 }
                                                                                span { "Fold Decision into Graph" }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        } else {
                                            p {
                                                role: "status",
                                                "data-testid": "digest-no-session-selected",
                                                "Select an ingested CLI session on the left to inspect its distilled decisions."
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
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::FauxBackend;
    use crate::AppWithBackend;
    use crate::AppWithBackendProps;
    use std::sync::Arc;

    fn render_view(view: View, backend: FauxBackend) -> String {
        let mut dom = VirtualDom::new_with_props(
            AppWithBackend,
            AppWithBackendProps {
                backend: Arc::new(backend),
                initial_view: view,
                accounts_open: false,
            },
        );
        dom.rebuild_in_place();
        dioxus::ssr::render(&dom)
    }

    #[test]
    fn review_view_renders_four_choice_fsrs_deck_and_grading_buttons() {
        let html = render_view(View::Review, FauxBackend::default());

        assert!(html.contains(r#"data-testid="view-review""#));
        assert!(html.contains(r#"data-testid="fsrs-active-card""#));
        assert!(html.contains(r#"data-testid="fsrs-card-mastery-ring""#));
        assert!(html.contains(r#"data-testid="fsrs-options-group""#));

        for letter in ["A", "B", "C", "D"] {
            assert!(
                html.contains(&format!(r#"data-testid="fsrs-option-{letter}""#)),
                "missing 4-choice option {letter}"
            );
        }

        for grade in ["again", "hard", "good", "easy"] {
            assert!(
                html.contains(&format!(r#"data-testid="fsrs-grade-{grade}""#)),
                "missing FSRS grade button {grade}"
            );
            assert!(
                html.contains(&format!(r#"data-testid="fsrs-interval-{grade}""#)),
                "missing FSRS interval preview badge {grade}"
            );
        }
        assert!(!html.contains("<script"));
    }

    #[test]
    fn digest_view_renders_multi_account_sessions_decisions_and_fold_actions() {
        let html = render_view(View::Digest, FauxBackend::default());

        assert!(html.contains(r#"data-testid="view-digest""#));
        assert!(html.contains(r#"data-testid="digest-ingest-now""#));
        assert!(html.contains(r#"data-testid="digest-fold-into-graph""#));
        assert!(html.contains(r#"data-testid="digest-mint-cards""#));
        assert!(html.contains(r#"data-testid="digest-session-list""#));
        assert!(html.contains(r#"data-testid="digest-decision-card-d_fsrs_deterministic_replay""#));
        assert!(html.contains("claude:pratham.bhavsar501@gmail.com (#1)"));
        assert!(html.contains("claude:q97159745@gmail.com (#2)"));
        assert!(html.contains("Order-Independent FSRS v5 Replay via (ts, id) Fold"));
        assert!(!html.contains("<script"));
    }

    #[test]
    fn interval_formatting_and_fsrs_preview_are_deterministic() {
        let card = Card::new(BASE_NOW_MS);
        let ivls = preview(Some(&card), BASE_NOW_MS);
        assert_eq!(format_interval_ms(ivls[0]), "1m");
        assert_eq!(format_interval_ms(ivls[1]), "6m");
        assert_eq!(format_interval_ms(ivls[2]), "10m");
        assert!(ivls[3] >= 86_400_000);
        assert_eq!(format_interval_ms(3 * 3_600_000), "3h");
        assert_eq!(format_interval_ms(5 * 86_400_000), "5d");
    }

    #[test]
    fn default_deck_cards_pass_validate_card_shape_and_compute_mastery() {
        let cards = default_deck_cards();
        assert_eq!(cards.len(), 4);
        for item in &cards {
            let right = item.flashcard.options[usize::from(item.flashcard.correct_index)].clone();
            let wrong_vec: Vec<String> = item
                .flashcard
                .options
                .iter()
                .enumerate()
                .filter(|(idx, _)| *idx != usize::from(item.flashcard.correct_index))
                .map(|(_, s)| s.clone())
                .collect();
            let draft = lp_core::fsrs::CardDraft {
                front: item.flashcard.front.clone(),
                right,
                wrong: [
                    wrong_vec[0].clone(),
                    wrong_vec[1].clone(),
                    wrong_vec[2].clone(),
                ],
            };
            assert_eq!(lp_core::fsrs::validate_card_shape(&draft), Ok(()));
            let score = item.mastery_score();
            assert!((0.0..=1.0).contains(&score));
        }
    }

    #[test]
    fn default_digest_sessions_cover_claude_codex_and_agy_with_decisions() {
        let sessions = default_digest_sessions();
        assert_eq!(sessions.len(), 4);
        assert!(sessions.iter().any(|s| s.session.source == "claude"));
        assert!(sessions.iter().any(|s| s.session.source == "codex"));
        assert!(sessions.iter().any(|s| s.session.source == "agy"));
        for s in &sessions {
            assert!(!s.decisions.is_empty());
        }
    }
}
