//! `learnpuppy seed`: deterministic fixture data roots for tests and demos (docs/web/07
//! §Fixtures).
//!
//! Everything is a pure function of the fixture name: a seeded RNG, a fixed clock
//! ([`FIXTURE_NOW`]) and fixed ids, so seeding twice yields byte-identical ledgers and chats.
//! E2E runs pin the server clock to [`FIXTURE_NOW`] (`test-hooks`), which makes "due today" and
//! relative times stable.
//!
//! A fixture root holds a global `settings.json` and `state.json`, two Claude logins (the root
//! doubles as `$HOME` for `serve --fixture`), and these workspaces:
//!
//! | id | name | state |
//! |---|---|---|
//! | `ws_5eed0001` | Rust async | live, sized by the fixture |
//! | `ws_5eed0002` | Distributed systems | live, small |
//! | `ws_5eed0003` | Old course | archived |
//! | `ws_0badc0de` | (none) | damaged: corrupt `workspace.json` |
//! | `ws_5eed0004` | Scratch notes | in the trash |

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use anyhow::Context;
use chrono::{DateTime, Utc};
use lp_agent::loops::{Call, Kind, Msg, Session};
use lp_core::fsrs::{self, Card, Rating, ReviewEvent};
use lp_core::graph::{GraphChain, GraphNode, Provenance};
use lp_core::plan::{PlanNode, PlanTree};
use lp_core::ports::{
    Alternative, ChatRecord, ChatStore, CodingSession, DecisionBasis, DecisionRecord,
    DeliveryStatus, FlashCard, FsrsReviewLog, GraphStore, JobKind, JobRecord, JobStatus, JobStore,
    SessionCheckpoint, TrackerStore,
};
use lp_service::digest::CardLinks;
use lp_service::settings::Settings;
use lp_service::stores::Stores;
use lp_service::workspace::{State, WorkspaceMeta};
use lp_service::{graph as graph_svc, paths};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use serde::Serialize;
use serde_json::json;

/// The instant fixtures treat as "now": 2025-10-29T12:00:00Z.
pub const FIXTURE_NOW: i64 = 1_761_739_200_000;
/// The device id fixture reviews and settings carry.
pub const FIXTURE_DEVICE: &str = "seed-device";
const MS_DAY: i64 = 86_400_000;
const MS_MIN: i64 = 60_000;

pub const MAIN_WS: &str = "ws_5eed0001";
pub const SECOND_WS: &str = "ws_5eed0002";
pub const ARCHIVED_WS: &str = "ws_5eed0003";
pub const TRASHED_WS: &str = "ws_5eed0004";
pub const DAMAGED_WS: &str = "ws_0badc0de";

#[derive(Clone, Copy, Debug, PartialEq, Eq, clap::ValueEnum)]
pub enum Fixture {
    /// 40 nodes.
    Small,
    /// 800 nodes, 1.2k chains, 6 chats, 120 cards, 30 decisions.
    Medium,
    /// 5k nodes.
    Large,
    /// 10k nodes.
    Huge,
}

struct Spec {
    nodes: usize,
    chains: usize,
    extra_chats: usize,
    cards: usize,
    decisions: usize,
    /// Compute `positions.json` now (cheap enough); bigger graphs lay out on first request.
    layout: bool,
}

impl Fixture {
    pub fn name(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
            Self::Huge => "huge",
        }
    }

    fn spec(self) -> Spec {
        match self {
            Self::Small => Spec {
                nodes: 40,
                chains: 55,
                extra_chats: 0,
                cards: 24,
                decisions: 8,
                layout: true,
            },
            Self::Medium => Spec {
                nodes: 800,
                chains: 1_200,
                extra_chats: 1,
                cards: 120,
                decisions: 30,
                layout: true,
            },
            Self::Large => Spec {
                nodes: 5_000,
                chains: 7_500,
                extra_chats: 7,
                cards: 400,
                decisions: 120,
                layout: false,
            },
            Self::Huge => Spec {
                nodes: 10_000,
                chains: 15_000,
                extra_chats: 15,
                cards: 800,
                decisions: 200,
                layout: false,
            },
        }
    }
}

/// What a run wrote, for the CLI to print.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct SeedReport {
    pub fixture: String,
    pub root: String,
    pub now: i64,
    pub main_workspace: String,
    pub nodes: usize,
    pub chains: usize,
    pub chats: usize,
    pub cards: usize,
    pub decisions: usize,
}

/// Writes the fixture into `out` (created if needed; must not already hold workspaces).
pub fn seed(fixture: Fixture, out: &Path) -> anyhow::Result<SeedReport> {
    if paths::workspaces_dir(out).exists() {
        anyhow::bail!(
            "{} already holds workspaces; seed into an empty directory",
            out.display()
        );
    }
    std::fs::create_dir_all(out).with_context(|| out.display().to_string())?;
    let spec = fixture.spec();

    write_global(out)?;
    write_accounts(out)?;

    let main = main_workspace(out, &spec)?;
    small_workspace(out, SECOND_WS, "Distributed systems", "network", "teal", 1)?;
    let old = small_workspace(out, ARCHIVED_WS, "Old course", "graduation-cap", "gold", 2)?;
    let mut meta = WorkspaceMeta::load(&old)?;
    meta.archived = true;
    meta.save(&old)?;
    trashed_workspace(out)?;
    let bad = paths::workspace_dir(out, DAMAGED_WS);
    std::fs::create_dir_all(&bad)?;
    std::fs::write(
        bad.join("workspace.json"),
        "{ \"id\": \"ws_0badc0de\", \"name\": ",
    )?;

    State {
        last_workspace: Some(MAIN_WS.into()),
        device_id: FIXTURE_DEVICE.into(),
        schema: lp_service::workspace::state::STATE_SCHEMA,
    }
    .save(out)?;
    std::fs::write(
        out.join("fixture.json"),
        serde_json::to_vec_pretty(&json!({
            "fixture": fixture.name(),
            "now": FIXTURE_NOW,
            "mainWorkspace": MAIN_WS,
            "device": FIXTURE_DEVICE,
        }))?,
    )?;

    Ok(SeedReport {
        fixture: fixture.name().into(),
        root: out.display().to_string(),
        now: FIXTURE_NOW,
        main_workspace: MAIN_WS.into(),
        nodes: main.nodes,
        chains: main.chains,
        chats: main.chats,
        cards: main.cards,
        decisions: main.decisions,
    })
}

fn write_global(out: &Path) -> anyhow::Result<()> {
    let mut s = Settings::default();
    s.apply_defaults();
    s.device_id = FIXTURE_DEVICE.into();
    s.save(out)
}

/// Two Claude logins under `out`, which `serve --fixture` uses as `$HOME`.
fn write_accounts(out: &Path) -> anyhow::Result<()> {
    for (dir, email, tier) in [
        (".claude", "ada@example.com", "default_claude_max_5x"),
        (
            ".claude-accounts/work",
            "ada@work.example",
            "default_claude_pro",
        ),
    ] {
        let d = out.join(dir);
        std::fs::create_dir_all(&d)?;
        std::fs::write(
            d.join(".claude.json"),
            serde_json::to_vec_pretty(&json!({
                "oauthAccount": {
                    "accountUuid": uuid_for(email),
                    "emailAddress": email,
                    "userRateLimitTier": tier,
                }
            }))?,
        )?;
    }
    Ok(())
}

fn uuid_for(seed: &str) -> String {
    let mut h = 0xcbf2_9ce4_8422_2325_u64;
    for b in seed.bytes() {
        h = (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    format!(
        "{:08x}-0000-4000-8000-{:012x}",
        (h >> 32) as u32,
        h & 0xffff_ffff_ffff
    )
}

fn meta(id: &str, name: &str, icon: &str, hue: &str, opened_days_ago: i64) -> WorkspaceMeta {
    let opened = FIXTURE_NOW - opened_days_ago * MS_DAY;
    let mut m: WorkspaceMeta = serde_json::from_value(json!({
        "id": id,
        "name": name,
        "slug": name.to_lowercase().replace(' ', "-"),
        "icon": icon,
        "hue": hue,
        "description": format!("Fixture workspace: {name}."),
        "created_at": opened - 40 * MS_DAY,
        "last_opened_at": opened,
    }))
    .expect("static workspace json");
    m.sources.allow = vec![PathBuf::from(format!("/Users/ada/code/{}", m.slug))];
    m
}

fn small_workspace(
    out: &Path,
    id: &str,
    name: &str,
    icon: &str,
    hue: &str,
    opened_days_ago: i64,
) -> anyhow::Result<PathBuf> {
    let dir = paths::workspace_dir(out, id);
    std::fs::create_dir_all(&dir)?;
    meta(id, name, icon, hue, opened_days_ago).save(&dir)?;
    let stores = Stores::open(&dir)?;
    let spec = Fixture::Small.spec();
    let spec = Spec {
        nodes: 12,
        chains: 14,
        cards: 6,
        decisions: 2,
        ..spec
    };
    populate(&dir, &stores, &spec, id, false)?;
    Ok(dir)
}

fn trashed_workspace(out: &Path) -> anyhow::Result<()> {
    let dir = out
        .join(".trash")
        .join(format!("{}-{TRASHED_WS}", FIXTURE_NOW - 3 * MS_DAY));
    std::fs::create_dir_all(&dir)?;
    meta(TRASHED_WS, "Scratch notes", "notebook-pen", "rose", 9).save(&dir)?;
    Stores::open(&dir)?;
    Ok(())
}

struct Counts {
    nodes: usize,
    chains: usize,
    chats: usize,
    cards: usize,
    decisions: usize,
}

fn main_workspace(out: &Path, spec: &Spec) -> anyhow::Result<Counts> {
    let dir = paths::workspace_dir(out, MAIN_WS);
    std::fs::create_dir_all(&dir)?;
    let mut m = meta(MAIN_WS, "Rust async", "code", "violet", 0);
    m.description = "Futures, executors and the pinning story.".into();
    m.save(&dir)?;
    let stores = Stores::open(&dir)?;
    populate(&dir, &stores, spec, MAIN_WS, true)
}

// ------------------------------------------------------------------ content

const TOPICS: [&str; 24] = [
    "Future",
    "Poll",
    "Waker",
    "Executor",
    "Pin",
    "Stream",
    "Task",
    "Reactor",
    "Runtime",
    "Mutex",
    "Channel",
    "Select",
    "Cancellation",
    "Backpressure",
    "Timer",
    "Scheduler",
    "Await point",
    "Send bound",
    "Borrow",
    "Lifetime",
    "Drop",
    "Spawn",
    "Join handle",
    "Waker vtable",
];
const FACETS: [&str; 20] = [
    "ownership",
    "state machine",
    "wake-up",
    "cost",
    "safety",
    "ordering",
    "lifetime",
    "contract",
    "invariant",
    "failure mode",
    "fairness",
    "allocation",
    "cancellation safety",
    "thread affinity",
    "progress",
    "aliasing",
    "layout",
    "lowering",
    "polling",
    "teardown",
];
const CLAIMS: [&str; 12] = [
    "is decided by the type, not by convention",
    "must hold across every await point",
    "is checked at compile time",
    "is cheap until the first contended access",
    "breaks when the value moves",
    "needs a waker registered before it returns Pending",
    "is observable only through the poll contract",
    "is dropped exactly once, even on cancellation",
    "cannot be shared without a synchronisation point",
    "follows from how the compiler lowers async blocks",
    "trades latency for throughput",
    "is an implementation detail callers must not rely on",
];
const PROJECTS: [&str; 6] = [
    "tokio-notes",
    "mini-redis",
    "async-book",
    "executor-lab",
    "wasm-runtime",
    "net-kit",
];
const BRANCHES: [&str; 4] = ["main", "feat/timers", "fix/wake-leak", "spike/io-uring"];
const DECISIONS: [(&str, &str, &str, &str, &str); 8] = [
    (
        "Use a bounded channel for work hand-off",
        "Replaced the unbounded queue with a bounded mpsc channel.",
        "Unbounded queues hide overload until memory runs out.",
        "unbounded mpsc",
        "it grows without limit under a slow consumer",
    ),
    (
        "Pin futures on the heap in the executor",
        "Stored spawned futures as Pin<Box<dyn Future>>.",
        "The executor must own futures whose size it cannot know.",
        "generic task type",
        "monomorphisation bloat for little gain",
    ),
    (
        "Register the waker before the readiness check",
        "Moved waker registration above the readiness test.",
        "Otherwise a wake-up between check and registration is lost.",
        "check first, register after",
        "races with the reactor thread",
    ),
    (
        "Cancel via drop, not a flag",
        "Dropped the task future to cancel it.",
        "Drop runs destructors and keeps cancellation structural.",
        "cooperative cancel flag",
        "every await point would need to poll it",
    ),
    (
        "Keep timers in a hierarchical wheel",
        "Implemented a hashed timing wheel for timeouts.",
        "O(1) insert and expiry for many timers.",
        "binary heap",
        "O(log n) per timer under heavy churn",
    ),
    (
        "Make the runtime handle cheap to clone",
        "Wrapped runtime state in an Arc'd handle.",
        "Spawning from any thread needs shared access without a lock per call.",
        "global static runtime",
        "tests could not isolate runtimes",
    ),
    (
        "Poll the I/O driver on the scheduler thread",
        "Merged driver polling into the scheduler tick.",
        "Avoids a dedicated thread and a cross-thread wake on every event.",
        "separate I/O thread",
        "extra context switches per readiness event",
    ),
    (
        "Return Poll::Pending only after yielding budget",
        "Added a coop budget that forces a yield.",
        "A hot task must not starve its neighbours.",
        "no budget",
        "one busy loop blocked the executor",
    ),
];

fn pick<'a, T>(rng: &mut StdRng, xs: &'a [T]) -> &'a T {
    &xs[rng.gen_range(0..xs.len())]
}

fn at(ms: i64) -> DateTime<Utc> {
    DateTime::from_timestamp_millis(ms).unwrap_or(DateTime::UNIX_EPOCH)
}

fn populate(
    dir: &Path,
    stores: &Stores,
    spec: &Spec,
    ws: &str,
    full: bool,
) -> anyhow::Result<Counts> {
    // One RNG per workspace, keyed by its id, so adding a workspace never shifts another.
    let seed = ws.bytes().fold(0x5eed_u64, |h, b| {
        h.wrapping_mul(31).wrapping_add(u64::from(b))
    });
    let mut rng = StdRng::seed_from_u64(seed ^ (spec.nodes as u64));

    // Sessions: ingested coding sessions that hold decisions and nodes, plus the seeded chats.
    let chat_ids: Vec<String> = chat_plan(spec).iter().map(|c| c.id.clone()).collect();
    let n_sessions = (spec.nodes / 12).max(4);
    let sources = ["claude", "claude", "claude", "codex", "agy", "git"];
    let sessions: Vec<(String, &str)> = (0..n_sessions)
        .map(|i| {
            let src = sources[i % sources.len()];
            (format!("{src}:s{i:04}"), src)
        })
        .collect();

    // --- decisions and their sessions
    let t = &*stores.tracker;
    let mut decisions = Vec::new();
    for i in 0..spec.decisions {
        let (sid, src) = &sessions[i % sessions.len()];
        let (title, what, why, alt, because) = DECISIONS[i % DECISIONS.len()];
        let n = i / DECISIONS.len();
        decisions.push(DecisionRecord {
            id: format!("d_{i:04}").into(),
            session_id: sid.as_str().into(),
            title: if n == 0 {
                title.to_owned()
            } else {
                format!("{title} (#{})", n + 1)
            },
            what: what.into(),
            why: why.into(),
            basis: if i % 3 == 2 {
                DecisionBasis::Inferred
            } else {
                DecisionBasis::Stated
            },
            alternatives: vec![Alternative {
                option: alt.into(),
                rejected_because: because.into(),
            }],
            risk: if i % 4 == 0 {
                "Hard to reverse once callers depend on it.".into()
            } else {
                String::new()
            },
            files: vec![format!(
                "src/{}.rs",
                ["executor", "reactor", "timer", "channel"][i % 4]
            )],
            commit_sha: (i % 5 == 0).then(|| format!("{:040x}", 0xabc0_0000_u64 + i as u64)),
        });
        let _ = src;
    }
    let mut seen_sessions = HashSet::new();
    for (i, d) in decisions.iter().enumerate() {
        let sid = d.session_id.to_string();
        if !seen_sessions.insert(sid.clone()) {
            continue;
        }
        let idx = sessions.iter().position(|(s, _)| *s == sid).unwrap_or(0);
        t.upsert_session(&CodingSession {
            id: sid.as_str().into(),
            source: sessions[idx].1.into(),
            cwd: format!("/Users/ada/code/{}", PROJECTS[idx % PROJECTS.len()]),
            git_branch: Some((*pick(&mut rng, &BRANCHES)).to_owned()),
            // Spread over the last fortnight, a few sessions per day.
            started_at: FIXTURE_NOW
                - (idx as i64 / 2) * MS_DAY
                - 3 * 3_600_000
                - (i as i64 % 3) * 45 * MS_MIN,
            transcript: format!("[{sid}] fixture transcript"),
        })?;
    }
    for d in &decisions {
        t.upsert_decision(d)?;
    }
    for (i, d) in decisions.iter().enumerate() {
        if i % 2 == 0 {
            stores.graph.append_checkpoint(&SessionCheckpoint {
                chat_id: d.id.clone(),
                folded_at: FIXTURE_NOW - (i as i64) * MS_MIN,
            })?;
        }
    }

    // --- nodes
    let g = &*stores.graph;
    let mut node_ids = Vec::with_capacity(spec.nodes);
    let mut used = HashSet::new();
    for i in 0..spec.nodes {
        let (sid, _) = &sessions[(i * n_sessions / spec.nodes.max(1)).min(n_sessions - 1)];
        let (chat_born, decision_born) =
            (i % 10 == 9, decisions.iter().find(|d| d.session_id == *sid));
        let provenance = if chat_born && !chat_ids.is_empty() {
            let c = &chat_ids[i % chat_ids.len()];
            vec![Provenance {
                chat_id: c.as_str().into(),
                message_id: format!("m{}", 1 + i % 5).into(),
            }]
        } else {
            vec![Provenance {
                chat_id: sid.as_str().into(),
                message_id: decision_born
                    .map_or_else(|| "m0".to_owned(), |d| d.id.to_string())
                    .into(),
            }]
        };
        let mut name = format!("{} {}", pick(&mut rng, &TOPICS), pick(&mut rng, &FACETS));
        if !used.insert(name.clone()) {
            name = format!("{name} {}", i + 1);
            used.insert(name.clone());
        }
        let statement = format!("The {} {}.", name.to_lowercase(), pick(&mut rng, &CLAIMS));
        let id = format!("n_{i:05}");
        g.append_node(&GraphNode {
            id: id.as_str().into(),
            name,
            statement,
            aliases: if i % 7 == 0 {
                vec![format!("alias {i}")]
            } else {
                vec![]
            },
            created_at: at(FIXTURE_NOW - (spec.nodes - i) as i64 * 45 * MS_MIN),
            provenance,
        })?;
        node_ids.push(id);
    }

    // --- chains: local to a window so clusters read as clusters, a few cross-links
    let mut keys = HashSet::new();
    let mut made = 0;
    let mut tries = 0;
    while made < spec.chains && tries < spec.chains * 20 && node_ids.len() > 3 {
        tries += 1;
        let c = rng.gen_range(2..node_ids.len());
        let span = if rng.gen_ratio(1, 12) {
            node_ids.len()
        } else {
            24.min(c)
        };
        let lo = c.saturating_sub(span);
        let k = rng.gen_range(1..=3.min(c - lo).max(1));
        let mut prem: Vec<usize> = Vec::new();
        for _ in 0..k {
            let p = rng.gen_range(lo..c);
            if !prem.contains(&p) {
                prem.push(p);
            }
        }
        prem.sort_unstable();
        let key = format!("{prem:?}>{c}");
        if !keys.insert(key) {
            continue;
        }
        g.append_chain(&GraphChain {
            id: format!("c_{made:05}").into(),
            premises: prem.iter().map(|p| node_ids[*p].as_str().into()).collect(),
            conclusion: node_ids[c].as_str().into(),
            provenance: vec![],
            created_at: at(FIXTURE_NOW - (spec.chains - made) as i64 * 30 * MS_MIN),
        })?;
        made += 1;
    }

    // --- cards (decision-linked ones first), FSRS states, reviews
    let card_count = write_cards(stores, dir, spec, &decisions, &mut rng)?;

    // --- chats, jobs, artifacts, ingest runs
    let mut chats = 0;
    if full {
        chats = write_chats(stores, dir, spec)?;
        write_ingest_runs(dir)?;
    }

    if spec.layout {
        let loaded = graph_svc::Loaded::new(graph_svc::snapshot(g)?);
        graph_svc::write_positions(&dir.join("graph"), &graph_svc::compute_positions(&loaded))?;
    }
    Ok(Counts {
        nodes: node_ids.len(),
        chains: made,
        chats,
        cards: card_count,
        decisions: decisions.len(),
    })
}

/// Cards in every FSRS state with real review histories behind them, a daily streak ending
/// "today", and due dates spread from overdue to two weeks out.
fn write_cards(
    stores: &Stores,
    dir: &Path,
    spec: &Spec,
    decisions: &[DecisionRecord],
    rng: &mut StdRng,
) -> anyhow::Result<usize> {
    let t = &*stores.tracker;
    let mut links = CardLinks::default();
    let mut reviews_made = 0usize;
    for i in 0..spec.cards {
        let decision = decisions.get(i % decisions.len().max(1));
        let (front, good, wrong) = card_text(i, decision);
        let mut options = [
            wrong[0].clone(),
            wrong[1].clone(),
            wrong[2].clone(),
            good.clone(),
        ];
        let correct_index: u8 = rng.gen_range(0..4);
        options.swap(3, usize::from(correct_index));
        let id = format!("card_{i:04}");

        // The recipe decides the state; ages are jittered so dues spread out.
        let age = rng.gen_range(1..25) as i64;
        let recipe: &[(i64, Rating)] = match i % 5 {
            0 => &[],
            1 => &[(0, Rating::Again)],
            2 => &[(0, Rating::Good), (1, Rating::Good), (3, Rating::Good)],
            3 => &[(0, Rating::Good), (1, Rating::Good), (4, Rating::Again)],
            _ => &[(0, Rating::Good), (2, Rating::Easy), (6, Rating::Good)],
        };
        let last = recipe.last().map_or(0, |r| r.0);
        let first = FIXTURE_NOW - (age.max(last + 1)) * MS_DAY + 9 * 3_600_000;
        let events: Vec<ReviewEvent> = recipe
            .iter()
            .map(|(d, r)| ReviewEvent {
                id: FIXTURE_DEVICE.into(),
                ts: first + d * MS_DAY,
                rating: *r,
            })
            .collect();
        let card = if events.is_empty() {
            Card::new(FIXTURE_NOW - rng.gen_range(0..3) * MS_DAY)
        } else {
            fsrs::replay(&events)
        };
        let fc = FlashCard {
            id: id.as_str().into(),
            front,
            options,
            correct_index,
            rationale: format!("{good} — it follows from the decision's rationale."),
            card,
        };
        t.upsert_card(&fc)?;
        for e in &events {
            let log = FsrsReviewLog {
                card_id: fc.id.clone(),
                ts: e.ts,
                rating: e.rating,
                device_id: FIXTURE_DEVICE.into(),
            };
            t.append_review(&log)?;
            let mut line = serde_json::to_value(&log)?;
            line["kind"] = "review".into();
            t.append_sync(FIXTURE_DEVICE, &line)?;
            reviews_made += 1;
        }
        if let Some(d) = decision {
            links.link(&d.id, [id]);
        }
    }
    // A review streak: one extra review per day for the last six days, ending today.
    if let Some(card) = t.cards()?.into_iter().find(|c| c.card.reps > 0) {
        for back in 0..6 {
            let log = FsrsReviewLog {
                card_id: card.id.clone(),
                ts: FIXTURE_NOW - back * MS_DAY - 2 * 3_600_000,
                rating: Rating::Good,
                device_id: FIXTURE_DEVICE.into(),
            };
            t.append_review(&log)?;
            let mut line = serde_json::to_value(&log)?;
            line["kind"] = "review".into();
            t.append_sync(FIXTURE_DEVICE, &line)?;
            reviews_made += 1;
        }
    }
    let _ = reviews_made;
    links.save(&dir.join("tracker"))?;
    Ok(spec.cards)
}

fn card_text(i: usize, d: Option<&DecisionRecord>) -> (String, String, [String; 3]) {
    match d {
        Some(d) => (
            format!("Why did the team choose: \"{}\"?", d.title.to_lowercase()),
            d.why.clone(),
            [
                "It was the only option the compiler accepted.".to_owned(),
                "A benchmark showed it was twice as fast.".to_owned(),
                format!("Nobody objected in review #{}.", i + 1),
            ],
        ),
        None => (
            format!("Question {}", i + 1),
            "The correct choice".into(),
            [
                "A plausible miss".into(),
                "Another miss".into(),
                "A third miss".into(),
            ],
        ),
    }
}

// ------------------------------------------------------------------ chats and jobs

struct ChatSpec {
    id: String,
    title: String,
    session: Session,
    age_min: i64,
}

fn quiz(node: &str, q: &str, explanation: &str) -> serde_json::Value {
    json!({
        "question": q,
        "options": [
            {"label": "The owner drops it", "value": "opt_a"},
            {"label": "The reader frees it", "value": "opt_b"},
            {"label": "Nobody; it leaks", "value": "opt_c"}
        ],
        "correctAnswer": "opt_a",
        "explanation": explanation,
        "type": "check",
        "nodeId": node
    })
}

fn call(id: &str, name: &str, args: serde_json::Value) -> Msg {
    Msg::Assistant {
        text: String::new(),
        calls: vec![Call {
            id: id.into(),
            name: name.into(),
            args,
        }],
    }
}

fn say(text: &str) -> Msg {
    Msg::Assistant {
        text: text.into(),
        calls: vec![],
    }
}

fn plan() -> PlanTree {
    let nodes = [
        ("n1", "Futures are state machines"),
        ("n2", "Polling and wakers"),
        ("n3", "Pinning"),
    ];
    PlanTree {
        nodes: nodes
            .iter()
            .map(|(id, title)| PlanNode {
                id: (*id).into(),
                title: (*title).into(),
                summary: String::new(),
            })
            .collect(),
        edges: vec![("n1".into(), "n2".into()), ("n2".into(), "n3".into())],
        version: 1,
    }
}

fn tool_ok(name: &str, text: &str) -> Msg {
    Msg::ToolResult {
        call_id: format!("{name}#0"),
        name: name.into(),
        ok: true,
        text: text.into(),
    }
}

fn chat_plan(spec: &Spec) -> Vec<ChatSpec> {
    let mut out = Vec::new();

    let mut probe = Session::new("chat_probe", Kind::Teach);
    probe.messages.push(Msg::User("Teach me async Rust".into()));
    probe.messages.push(say("Let's see what you already know."));
    probe.messages.push(call(
        "probe#1",
        "probe",
        quiz(
            "n1",
            "What does an async fn return?",
            "It returns a future that does nothing until polled.",
        ),
    ));
    probe.answer("probe#1", "selected: opt_a; confidence: 3/5");
    probe.messages.push(call(
        "probe#2",
        "probe",
        quiz(
            "n1",
            "Who decides when a future is polled?",
            "The executor decides; the future only reports Pending or Ready.",
        ),
    ));
    out.push(ChatSpec {
        id: "chat_probe".into(),
        title: "Async Rust from scratch".into(),
        session: probe,
        age_min: 20,
    });

    let mut planning = Session::new("chat_plan", Kind::Teach);
    planning.messages.push(Msg::User("Explain pinning".into()));
    planning.messages.push(call(
        "probe#1",
        "probe",
        quiz(
            "n3",
            "Why can't a self-referential struct move?",
            "Moving it would leave an internal pointer dangling.",
        ),
    ));
    planning.answer("probe#1", "selected: opt_a; confidence: 4/5");
    planning
        .messages
        .push(tool_ok("probe_done", "Probe complete. Handoff queued."));
    out.push(ChatSpec {
        id: "chat_plan".into(),
        title: "Pinning, explained".into(),
        session: planning,
        age_min: 3 * 60,
    });

    let mut teach = Session::new("chat_teach", Kind::Teach);
    teach.messages.push(Msg::User("How do wakers work?".into()));
    teach.messages.push(call(
        "probe#1",
        "probe",
        quiz(
            "n2",
            "What does a waker do?",
            "A waker re-queues its task on the scheduler.",
        ),
    ));
    teach.answer("probe#1", "selected: opt_a; confidence: 5/5");
    teach
        .messages
        .push(tool_ok("probe_done", "Probe complete."));
    teach.plan = Some(plan());
    teach.messages.push(tool_ok("plan_init", "Plan created."));
    teach.messages.push(Msg::Assistant {
        text: "A future is a state machine; polling advances it until it returns Pending.".into(),
        calls: vec![Call {
            id: "visual#1".into(),
            name: "visual".into(),
            args: json!({"title": "Poll lifecycle"}),
        }],
    });
    teach.messages.push(tool_ok(
        "visual",
        "dispatched as job_visual_done \u{2014} continue",
    ));
    teach.messages.push(call(
        "quiz#1",
        "quiz",
        quiz(
            "n1",
            "Who drops the value when the future completes?",
            "Ownership ends at the last move, so the final owner runs the drop.",
        ),
    ));
    out.push(ChatSpec {
        id: "chat_teach".into(),
        title: "How wakers work".into(),
        session: teach,
        age_min: 45,
    });

    let mut practice = Session::new("chat_practice", Kind::Practice);
    practice
        .messages
        .push(Msg::User("Practice: write a join combinator".into()));
    practice.messages.push(call(
        "problem#1",
        "problem",
        json!({
            "statement": "Implement join2 that polls two futures and returns both outputs.",
            "mode": "composition",
            "target": [],
            "modelSolution": "Poll each until Ready, store the first output, return the pair."
        }),
    ));
    out.push(ChatSpec {
        id: "chat_practice".into(),
        title: "Practice: join combinator".into(),
        session: practice,
        age_min: 9 * 60,
    });

    let mut done = Session::new("chat_done", Kind::Teach);
    done.messages.push(Msg::User("Why Send bounds?".into()));
    done.messages.push(call(
        "probe#1",
        "probe",
        quiz(
            "n1",
            "When must a future be Send?",
            "Whenever the executor may move it between threads.",
        ),
    ));
    done.answer("probe#1", "selected: opt_a; confidence: 4/5");
    done.messages.push(tool_ok("probe_done", "Probe complete."));
    done.plan = Some(plan());
    done.messages.push(tool_ok("plan_init", "Plan created."));
    for (i, node) in ["n1", "n2", "n3"].iter().enumerate() {
        let id = format!("quiz#{}", i + 1);
        done.messages.push(call(
            &id,
            "quiz",
            quiz(
                node,
                "Which side drops the value?",
                "The owner drops it when it leaves scope.",
            ),
        ));
        done.answer(&id, "selected: opt_a; confidence: 4/5");
    }
    done.messages
        .push(say("That covers the whole plan. Nicely done."));
    out.push(ChatSpec {
        id: "chat_done".into(),
        title: "Send bounds".into(),
        session: done,
        age_min: 2 * 24 * 60,
    });

    for i in 0..spec.extra_chats {
        let id = format!("chat_old{i:02}");
        let mut s = Session::new(id.as_str(), Kind::Teach);
        s.messages.push(Msg::User(format!("Older topic {}", i + 1)));
        s.messages
            .push(say("We covered this earlier; here is the summary."));
        out.push(ChatSpec {
            id,
            title: format!("Earlier session {}", i + 1),
            session: s,
            age_min: (3 + i as i64) * 24 * 60,
        });
    }
    out
}

fn write_chats(stores: &Stores, dir: &Path, spec: &Spec) -> anyhow::Result<usize> {
    let all = chat_plan(spec);
    for c in &all {
        let updated = FIXTURE_NOW - c.age_min * MS_MIN;
        stores.chats.save_chat(&ChatRecord {
            id: c.id.as_str().into(),
            title: c.title.clone(),
            created_at: updated - 20 * MS_MIN,
            updated_at: updated,
            body: serde_json::to_value(&c.session)?,
            turns: vec![],
            extractions: vec![],
        })?;
    }

    // Jobs in every status, all on the teaching chat.
    let job = |id: &str,
               kind: JobKind,
               status: JobStatus,
               result: Option<serde_json::Value>,
               age: i64| JobRecord {
        id: id.into(),
        chat_id: "chat_teach".into(),
        kind,
        status,
        delivery: if status == JobStatus::Running {
            DeliveryStatus::Pending
        } else {
            DeliveryStatus::Claimed
        },
        created_at: FIXTURE_NOW - age * MS_MIN,
        result,
        usage: vec![],
    };
    let jobs = [
        job(
            "job_visual_done",
            JobKind::Visual,
            JobStatus::Done,
            Some(json!({"title": "Poll lifecycle", "artifactId": "art_poll"})),
            40,
        ),
        job(
            "job_research_run",
            JobKind::Research,
            JobStatus::Running,
            None,
            2,
        ),
        job(
            "job_pdf_failed",
            JobKind::Pdf,
            JobStatus::Failed,
            Some(json!({"title": "Async book (PDF)", "error": "the PDF has no extractable text"})),
            30,
        ),
        job(
            "job_demo_done",
            JobKind::Demo,
            JobStatus::Done,
            Some(json!({"title": "Waker demo", "artifactId": "art_demo"})),
            35,
        ),
        job(
            "job_research_done",
            JobKind::Research,
            JobStatus::Done,
            Some(json!({"title": "Wakers in tokio", "artifactId": "art_research"})),
            38,
        ),
    ];
    for j in &jobs {
        stores.jobs.create_job(j)?;
    }
    let art = dir.join("artifacts");
    std::fs::create_dir_all(&art)?;
    std::fs::write(art.join("art_poll.html"), "<!doctype html><title>Poll lifecycle</title><h1>Poll lifecycle</h1><p>Pending, wake, poll again, Ready.</p>")?;
    std::fs::write(
        art.join("art_demo.html"),
        "<!doctype html><title>Waker demo</title><button>Wake</button>",
    )?;
    std::fs::write(
        art.join("art_research.md"),
        "# Wakers in tokio\n\nA waker re-queues its task on the scheduler.\n",
    )?;
    Ok(all.len())
}

fn write_ingest_runs(dir: &Path) -> anyhow::Result<()> {
    let runs = [
        (3, 4, 14, 41, 9, vec![]),
        (
            2,
            5,
            3,
            9,
            33,
            vec!["claude:s0007: transcript line 412 is not valid JSON".to_owned()],
        ),
        (
            1,
            2,
            0,
            0,
            52,
            vec![
                "codex:s0004: could not read the session file".to_owned(),
                "git: /Users/ada/code/net-kit is not a repository".to_owned(),
            ],
        ),
    ];
    let mut body = String::new();
    for (i, (days, sessions, decisions, cards, skipped, errors)) in runs.iter().enumerate() {
        let started = FIXTURE_NOW - days * MS_DAY;
        body.push_str(&serde_json::to_string(&json!({
            "id": format!("run_{i:03}"),
            "startedAt": started,
            "finishedAt": started + 38_000 + (i as i64) * 4_000,
            "report": {
                "sessions": sessions,
                "newDecisions": decisions,
                "newCards": cards,
                "skipped": skipped,
                "errors": errors,
            }
        }))?);
        body.push('\n');
    }
    let d = dir.join("ingest");
    std::fs::create_dir_all(&d)?;
    std::fs::write(d.join("runs.jsonl"), body)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    /// Every file under `root` except SQLite files, by relative path.
    fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
        fn walk(dir: &Path, root: &Path, out: &mut BTreeMap<String, Vec<u8>>) {
            for e in std::fs::read_dir(dir).unwrap().flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk(&p, root, out);
                } else {
                    let rel = p.strip_prefix(root).unwrap().display().to_string();
                    if !rel.contains(".db") {
                        out.insert(rel, std::fs::read(&p).unwrap());
                    }
                }
            }
        }
        let mut out = BTreeMap::new();
        walk(root, root, &mut out);
        out
    }

    #[test]
    fn seeding_twice_gives_identical_files_and_store_contents() {
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        seed(Fixture::Small, a.path()).unwrap();
        seed(Fixture::Small, b.path()).unwrap();
        let (ta, tb) = (tree(a.path()), tree(b.path()));
        assert_eq!(ta.keys().collect::<Vec<_>>(), tb.keys().collect::<Vec<_>>());
        for (k, v) in &ta {
            assert!(v == &tb[k], "{k} differs between runs");
        }
        assert!(ta.contains_key("workspaces/ws_5eed0001/graph/nodes.jsonl"));
        assert!(ta.contains_key("workspaces/ws_5eed0001/graph/positions.json"));

        let cards = |root: &Path| {
            let s = Stores::open(&paths::workspace_dir(root, MAIN_WS)).unwrap();
            (s.tracker.cards().unwrap(), s.tracker.reviews().unwrap())
        };
        assert_eq!(cards(a.path()), cards(b.path()));
    }

    #[test]
    fn small_fixture_has_the_documented_shape() {
        let d = tempfile::tempdir().unwrap();
        let r = seed(Fixture::Small, d.path()).unwrap();
        assert_eq!((r.nodes, r.chats, r.cards, r.decisions), (40, 5, 24, 8));
        assert!(r.chains > 30, "{}", r.chains);

        let s = Stores::open(&paths::workspace_dir(d.path(), MAIN_WS)).unwrap();
        assert_eq!(s.graph.nodes().unwrap().len(), 40);
        // Cards in every FSRS state.
        let states: HashSet<_> = s
            .tracker
            .cards()
            .unwrap()
            .iter()
            .map(|c| lp_service::review::state_name(c.card.state))
            .collect();
        assert_eq!(states.len(), 4, "{states:?}");
        // Jobs in every status.
        let (jobs, _) = s.jobs.all_report().unwrap();
        for st in [JobStatus::Running, JobStatus::Done, JobStatus::Failed] {
            assert!(jobs.iter().any(|j| j.status == st), "{st:?}");
        }
        // A chat at every phase.
        use crate::dto::chat::{ChatPhase, ChatSummaryDto};
        let phases: Vec<_> = s
            .chats
            .list_chats()
            .unwrap()
            .iter()
            .map(|c| ChatSummaryDto::new(&s.chats.load_chat(&c.id).unwrap(), &jobs).phase)
            .collect();
        for p in [
            ChatPhase::Probe,
            ChatPhase::Plan,
            ChatPhase::Teach,
            ChatPhase::Practice,
            ChatPhase::Done,
        ] {
            assert!(phases.contains(&p), "{p:?} missing from {phases:?}");
        }
        // Workspace set: live, archived, damaged, trashed; ingest runs carry errors.
        let reg = lp_service::workspace::WorkspaceRegistry::new(
            d.path(),
            std::sync::Arc::new(lp_service::workspace::NoopSink),
        )
        .unwrap();
        let all = reg.list(true).unwrap();
        assert_eq!(all.workspaces.len(), 3);
        assert_eq!(all.workspaces.iter().filter(|w| w.meta.archived).count(), 1);
        assert_eq!(all.damaged.len(), 1);
        assert_eq!(reg.trash_list().unwrap().len(), 1);
        let runs = std::fs::read_to_string(
            paths::workspace_dir(d.path(), MAIN_WS).join("ingest/runs.jsonl"),
        )
        .unwrap();
        assert!(runs.contains("not valid JSON"));
    }

    #[test]
    fn refuses_to_seed_over_existing_workspaces() {
        let d = tempfile::tempdir().unwrap();
        seed(Fixture::Small, d.path()).unwrap();
        assert!(seed(Fixture::Small, d.path()).is_err());
    }

    #[test]
    fn review_stats_are_stable_against_the_fixture_clock() {
        let d = tempfile::tempdir().unwrap();
        seed(Fixture::Small, d.path()).unwrap();
        let s = Stores::open(&paths::workspace_dir(d.path(), MAIN_WS)).unwrap();
        let st = lp_service::review::stats(&*s.tracker, FIXTURE_NOW).unwrap();
        assert!(st.due_now > 0 && st.due_now < 24, "{}", st.due_now);
        assert!(st.streak >= 5, "{}", st.streak);
        assert!(st.forecast.iter().sum::<u32>() > 0);
    }
}
