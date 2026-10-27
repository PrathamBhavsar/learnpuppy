//! `Backend`: the real `DataSource`, over the on-disk stores, the account pool and the agent loops.
//!
//! Commands that call a model run on a blocking thread (`run_turn` futures are not `Send`) and
//! publish their result through `Live`; the app's periodic `Refresh` swaps it into the next snapshot.

use std::collections::HashSet;
use std::future::Future;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Instant, SystemTime};

use anyhow::anyhow;
use lp_agent::extract::{Pipeline, SourceMessage};
use lp_agent::loops::{Kind, Session};
use lp_agent::workbench::{HttpWeb, PdftotextPdf, Workbench};
use lp_cli_driver::accounts::{AccountHealth, AccountPool};
use lp_cli_driver::engine::{LlmEngine, StreamDelta};
use lp_core::fsrs;
use lp_core::graph::GraphSnapshot;
use lp_core::ports::{
    ChatStore, DecisionBasis, DecisionRecord, GraphStore, SessionCheckpoint, TrackerStore,
};
use lp_store::FsJobStore;
use parking_lot::Mutex;
use tokio::runtime::Handle;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use crate::app::{AccountRow, Command, DataSource, Health, SlotPin, ViewSnapshot};
use crate::review;
use crate::session::{classroom_view, pending_call, render_answer, run_to_rest};
use crate::state::{now_ms, Settings, Stores};
use crate::views::classroom::Classroom;
use crate::views::graph::GraphData;
use crate::views::quiz::{Decision, DigestData, QuizCard, Rating};

/// Model slots, in the order the accounts modal lists them.
pub const ROLES: [&str; 6] = [
    "prober",
    "teacher",
    "practice",
    "extractor",
    "auditor",
    "distiller",
];
pub const DEFAULT_PIN: &str = "claude:auto";
/// Bounded stream channel capacity (PLAN 3.1).
const STREAM_DELTA_CAP: usize = 512;

fn keep<T: Default, X: std::fmt::Display>(r: Result<T, X>, errs: &mut Vec<String>) -> T {
    r.unwrap_or_else(|e| {
        errs.push(e.to_string());
        T::default()
    })
}

/// `(role, pin) -> engine`; called per turn so a re-pinned slot applies immediately.
type EngineFor<E> = Box<dyn Fn(&str, &str) -> Arc<E> + Send + Sync>;

#[derive(Default)]
struct Live {
    session: Option<Session>,
    busy: bool,
    /// Assistant text streamed so far in the running turn.
    stream: String,
    error: Option<String>,
    /// Cancels the running turn (a child of `Inner::shutdown`).
    turn: CancellationToken,
}

struct Inner<E> {
    root: PathBuf,
    /// Directory the agents' file tools read from.
    cwd: PathBuf,
    stores: Stores,
    pool: Arc<AccountPool>,
    settings: Mutex<Settings>,
    engine_for: EngineFor<E>,
    live: Mutex<Live>,
    /// Replication device id for FSRS reviews.
    device: String,
    /// Cancelled on quit; every turn and fold hangs off it.
    shutdown: CancellationToken,
    wb: Arc<Workbench<E, FsJobStore>>,
    graph: Mutex<GraphData>,
    /// Last store scan; `Refresh` reuses it until a store file (or the minute) changes.
    scan: Mutex<Option<Scan>>,
    scans: AtomicUsize,
}

struct Scan {
    key: ScanKey,
    graph: GraphSnapshot,
    digest: DigestData,
    errs: Vec<String>,
}

/// `(minute, [(file, mtime, len)])` of the graph and tracker store dirs: the cheap change detector.
type ScanKey = (i64, Vec<(std::ffi::OsString, Option<SystemTime>, u64)>);

fn scan_key(root: &std::path::Path) -> ScanKey {
    let mut files = Vec::new();
    for dir in ["graph", "tracker"] {
        for e in std::fs::read_dir(root.join(dir))
            .into_iter()
            .flatten()
            .flatten()
        {
            let m = e.metadata().ok();
            files.push((
                e.path().into_os_string(),
                m.as_ref().and_then(|m| m.modified().ok()),
                m.map_or(0, |m| m.len()),
            ));
        }
    }
    files.sort();
    (now_ms() / 60_000, files)
}

pub struct Backend<E> {
    inner: Arc<Inner<E>>,
    rt: Handle,
}

impl<E: LlmEngine + 'static> Backend<E> {
    /// Must be called inside a tokio runtime. Resumes the most recently updated chat.
    pub fn new(
        root: PathBuf,
        cwd: PathBuf,
        stores: Stores,
        settings: Settings,
        pool: Arc<AccountPool>,
        engine_for: impl Fn(&str, &str) -> Arc<E> + Send + Sync + 'static,
    ) -> Self {
        let engine_for = Arc::new(engine_for);
        let engine_for2 = engine_for.clone();
        let latest = stores
            .chats
            .list_chats()
            .unwrap_or_default()
            .into_iter()
            .max_by_key(|c| c.updated_at)
            .and_then(|c| Session::load(stores.chats.as_ref(), &c.id).ok());
        let mut live = Live {
            session: latest,
            ..Live::default()
        };
        let wb = Workbench::routed(
            |role| engine_for(role, DEFAULT_PIN),
            stores.jobs.clone(),
            Arc::new(HttpWeb::default()),
            Arc::new(PdftotextPdf),
            root.join("artifacts"),
        );
        if let Err(e) = wb.recover() {
            live.error = Some(e.to_string());
        }
        let device = if settings.device_id.is_empty() {
            "local".to_owned()
        } else {
            settings.device_id.clone()
        };
        Self {
            inner: Arc::new(Inner {
                root,
                cwd,
                stores,
                pool,
                settings: Mutex::new(settings),
                engine_for: Box::new(move |r, p| engine_for2(r, p)),
                live: Mutex::new(live),
                device,
                shutdown: CancellationToken::new(),
                wb,
                graph: Mutex::new(GraphData::default()),
                scan: Mutex::new(None),
                scans: AtomicUsize::new(0),
            }),
            rt: Handle::current(),
        }
    }

    /// Runs `make()`'s future to completion on a blocking thread.
    fn background<F: Future<Output = ()>>(
        &self,
        make: impl FnOnce(Arc<Inner<E>>) -> F + Send + 'static,
    ) {
        let (inner, rt) = (self.inner.clone(), self.rt.clone());
        self.rt.spawn_blocking(move || rt.block_on(make(inner)));
    }

    /// Starts a model turn on `s` unless one is already running.
    fn start(&self, s: Session) {
        let i = &self.inner;
        {
            let mut live = i.live.lock();
            if live.busy {
                return;
            }
            live.busy = true;
            live.error = None;
            live.turn = i.shutdown.child_token();
            live.session = Some(s.clone());
        }
        if let Err(e) = s.save(i.stores.chats.as_ref(), now_ms() / 1000) {
            i.live.lock().error = Some(e.to_string());
        }
        let cancel = live_turn(i);
        self.background(move |i| i.drive(s, cancel));
    }
}

fn live_turn<E>(i: &Inner<E>) -> CancellationToken {
    i.live.lock().turn.clone()
}

impl<E: LlmEngine + 'static> Inner<E> {
    fn engine(&self, role: &str) -> Arc<E> {
        let pin = self.settings.lock().pins.get(role).cloned();
        (self.engine_for)(role, pin.as_deref().unwrap_or(DEFAULT_PIN))
    }

    async fn drive(self: Arc<Self>, mut s: Session, cancel: CancellationToken) {
        let (tx, mut rx) = mpsc::channel(STREAM_DELTA_CAP);
        let me = self.clone();
        let drain = tokio::spawn(async move {
            while let Some(d) = rx.recv().await {
                if let StreamDelta::Text(t) = d {
                    me.live.lock().stream.push_str(&t);
                }
            }
        });
        let res = run_to_rest(&|r| self.engine(r), &self.cwd, &mut s, &tx, &cancel).await;
        let launched = match &res {
            Ok(()) => self.wb.launch(&mut s).err().map(|e| e.to_string()),
            Err(_) => None,
        };
        drop(tx);
        let _ = drain.await;
        let saved = s.save(self.stores.chats.as_ref(), now_ms() / 1000);
        let mut live = self.live.lock();
        live.stream.clear();
        live.busy = false;
        live.session = Some(s);
        live.error = res
            .err()
            .map(|e| e.to_string())
            .or_else(|| saved.err().map(|e| e.to_string()))
            .or(launched);
    }

    /// Folds one decision into the knowledge graph through the Extractor -> Auditor -> Merger pipeline.
    async fn fold(self: Arc<Self>, id: String) {
        let res: anyhow::Result<()> = async {
            let d = self
                .stores
                .tracker
                .decisions()?
                .into_iter()
                .find(|d| d.id == id)
                .ok_or_else(|| anyhow!("unknown decision {id}"))?;
            let pipe = Pipeline::new(self.engine("extractor"), self.stores.graph.clone());
            let msg = SourceMessage {
                id: "decision".into(),
                text: decision_text(&d),
            };
            pipe.extract(&d.id, &[msg], self.shutdown.child_token())
                .await?;
            self.stores.graph.append_checkpoint(&SessionCheckpoint {
                chat_id: d.id,
                folded_at: now_ms() / 1000,
            })?;
            Ok(())
        }
        .await;
        if let Err(e) = res {
            self.live.lock().error = Some(e.to_string());
        }
    }

    fn graph_snapshot(&self, errs: &mut Vec<String>) -> GraphSnapshot {
        let g = &self.stores.graph;
        GraphSnapshot::from_ledgers(&keep(g.nodes(), errs), &keep(g.chains(), errs))
    }

    fn fail(&self, e: impl std::fmt::Display) {
        self.live.lock().error = Some(e.to_string());
    }
}

fn decision_text(d: &DecisionRecord) -> String {
    let alts: Vec<String> = d
        .alternatives
        .iter()
        .map(|a| format!("{} (rejected: {})", a.option, a.rejected_because))
        .collect();
    format!(
        "{}\n\nWhat: {}\n\nWhy: {}\n\nAlternatives: {}\n\nRisk: {}",
        d.title,
        d.what,
        d.why,
        alts.join("; "),
        d.risk
    )
}

fn fsrs_rating(r: Rating) -> fsrs::Rating {
    match r {
        Rating::Again => fsrs::Rating::Again,
        Rating::Hard => fsrs::Rating::Hard,
        Rating::Good => fsrs::Rating::Good,
        Rating::Easy => fsrs::Rating::Easy,
    }
}

impl<E: LlmEngine + 'static> DataSource for Backend<E> {
    fn snapshot(&self) -> ViewSnapshot {
        let i = &self.inner;
        let now = Instant::now();
        let accounts = i
            .pool
            .snapshot()
            .into_iter()
            .map(|a| AccountRow {
                id: a.id.0,
                health: match a.health {
                    AccountHealth::Healthy => Health::Healthy,
                    AccountHealth::Cooldown { until, .. } => Health::Cooldown {
                        secs_left: until.saturating_duration_since(now).as_secs(),
                    },
                    AccountHealth::AuthExpired => Health::AuthExpired,
                },
                turns: a.window_requests,
                tokens: a.window_input_tokens + a.window_output_tokens,
            })
            .collect();
        let slots = {
            let settings = i.settings.lock();
            ROLES
                .iter()
                .map(|r| SlotPin {
                    role: (*r).into(),
                    pin: settings
                        .pins
                        .get(*r)
                        .map_or(DEFAULT_PIN.to_owned(), Clone::clone),
                })
                .collect()
        };
        let key = scan_key(&i.root);
        let (snap, digest, errs) = {
            let mut scan = i.scan.lock();
            if scan.as_ref().is_none_or(|s| s.key != key) {
                i.scans.fetch_add(1, Ordering::Relaxed);
                let mut errs = Vec::new();
                let graph = i.graph_snapshot(&mut errs);
                let digest = digest(i, &mut errs);
                *scan = Some(Scan {
                    key,
                    graph,
                    digest,
                    errs,
                });
            }
            let s = scan.as_ref().expect("scan was just filled");
            (s.graph.clone(), s.digest.clone(), s.errs.clone())
        };
        let graph = {
            let mut cache = i.graph.lock();
            if cache.snap != snap {
                *cache = GraphData::new(snap);
            }
            cache.clone()
        };
        let classroom = {
            let live = i.live.lock();
            let mut cl = live
                .session
                .as_ref()
                .map_or_else(Classroom::default, |s| classroom_view(s, &live.stream));
            if let Some(e) = &live.error {
                cl.transcript.push(format!("⚠ {e}"));
            }
            cl
        };
        let mut classroom = classroom;
        classroom
            .transcript
            .extend(errs.iter().map(|e| format!("⚠ {e}")));
        ViewSnapshot {
            accounts,
            slots,
            graph,
            classroom,
            digest,
        }
    }

    fn apply(&self, cmd: Command) {
        let i = &self.inner;
        match cmd {
            Command::Refresh => {
                // Wake the teacher with finished background jobs, unless a turn is running.
                let mut s = {
                    let live = i.live.lock();
                    match live.session.clone() {
                        Some(s) if !live.busy => s,
                        _ => return,
                    }
                };
                match i.wb.deliver(&mut s) {
                    Ok(true) => self.start(s),
                    Ok(false) => {}
                    Err(e) => i.fail(e),
                }
            }
            Command::Cancel => i.live.lock().turn.cancel(),
            Command::StartTeach { topic } => {
                let mut s = Session::new(format!("te_{:x}", now_ms()), Kind::Teach);
                s.push_user(topic);
                self.start(s);
            }
            Command::PinSlot { role, pin } => {
                let mut s = i.settings.lock();
                s.pins.insert(role, pin);
                if let Err(e) = s.save(&i.root) {
                    i.fail(e);
                }
            }
            Command::Practice { chains } => {
                let snap = i.graph_snapshot(&mut Vec::new());
                let name = |id: &str| snap.nodes.get(id).map_or(id.to_owned(), |n| n.name.clone());
                let lines: Vec<String> = snap
                    .chains
                    .iter()
                    .filter(|c| chains.iter().any(|id| *id == c.id))
                    .map(|c| {
                        let premises: Vec<String> = c.premises.iter().map(|p| name(p)).collect();
                        format!("- {} ⟹ {}", premises.join(" ∧ "), name(&c.conclusion))
                    })
                    .collect();
                let mut s = Session::new(format!("pr_{:x}", now_ms()), Kind::Practice);
                s.push_user(format!(
                    "Practice these reasoning chains:\n{}",
                    lines.join("\n")
                ));
                self.start(s);
            }
            Command::DeleteNodes { nodes } => {
                // Deleting a node also drops every chain that touches it.
                let ids: Vec<_> = nodes.iter().map(|n| n.as_str().into()).collect();
                let mut errs = Vec::new();
                let doomed: Vec<_> = i.graph_snapshot(&mut errs).deletion_blast_radius(&ids);
                // A failed read must abort: rewriting from an empty default would wipe every chain.
                let chains = match i.stores.graph.chains() {
                    Ok(c) if errs.is_empty() => c,
                    Ok(_) => return i.fail(errs.join("; ")),
                    Err(e) => return i.fail(e),
                };
                let keep: Vec<_> = chains
                    .into_iter()
                    .filter(|c| !doomed.contains(&c.id))
                    .collect();
                if let Err(e) = i
                    .stores
                    .graph
                    .rewrite_chains(&keep)
                    .and_then(|()| i.stores.graph.delete_nodes(&ids))
                {
                    i.fail(e);
                }
            }
            Command::Answer(a) => {
                let mut s = {
                    let live = i.live.lock();
                    match live.session.clone() {
                        Some(s) if !live.busy => s,
                        _ => return,
                    }
                };
                let Some(call_id) = pending_call(&s).map(|c| c.id.clone()) else {
                    return;
                };
                s.answer(&call_id, render_answer(&a));
                self.start(s);
            }
            Command::Grade { card, rating } => {
                if let Err(e) = review::grade(
                    i.stores.tracker.as_ref(),
                    &i.device,
                    &card,
                    fsrs_rating(rating),
                    now_ms(),
                ) {
                    i.fail(e);
                }
            }
            Command::FoldDecision { id } => self.background(move |i| i.fold(id)),
        }
    }

    fn shutdown(&self) {
        self.inner.shutdown.cancel();
    }
}

fn digest<E>(i: &Inner<E>, errs: &mut Vec<String>) -> DigestData {
    let t = i.stores.tracker.as_ref();
    let folded: HashSet<_> = keep(i.stores.graph.checkpoints(), errs)
        .into_iter()
        .map(|c| c.chat_id)
        .collect();
    let now = now_ms();
    DigestData {
        decisions: keep(t.decisions(), errs)
            .into_iter()
            .map(|d| Decision {
                project: t
                    .session(&d.session_id)
                    .ok()
                    .flatten()
                    .and_then(|s| {
                        let name = std::path::Path::new(&s.cwd).file_name()?;
                        Some(name.to_string_lossy().into_owned())
                    })
                    .unwrap_or_else(|| "-".into()),
                // Session ids are `<source>:<id>`.
                tool: d.session_id.split(':').next().unwrap_or_default().into(),
                what: d.what,
                why: d.why,
                stated: d.basis == DecisionBasis::Stated,
                alternatives: d
                    .alternatives
                    .iter()
                    .map(|a| format!("{}: {}", a.option, a.rejected_because))
                    .collect(),
                risk: d.risk,
                commit: d.commit_sha.unwrap_or_default(),
                folded: folded.contains(&d.id),
                id: d.id.to_string(),
            })
            .collect(),
        due: keep(review::due_cards(t, now), errs)
            .into_iter()
            .map(|c| QuizCard {
                id: c.id.to_string(),
                deck: "Due Today".into(),
                kind: "mc".into(),
                question: c.front,
                options: c.options,
                correct: usize::from(c.correct_index),
                rationale: c.rationale,
                source: String::new(),
            })
            .collect(),
        streak: review::streak(t, now),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_cli_driver::accounts::Strategy;
    use lp_cli_driver::engine::FauxEngine;
    use lp_core::ports::TrackerStore;
    use serde_json::json;

    fn backend(root: &std::path::Path, faux: Arc<FauxEngine>) -> Backend<FauxEngine> {
        Backend::new(
            root.to_owned(),
            root.to_owned(),
            Stores::open(root).unwrap(),
            Settings::default(),
            Arc::new(AccountPool::new(vec![], Strategy::Priority)),
            move |_, _| faux.clone(),
        )
    }

    #[test]
    fn pins_persist_and_grading_clears_the_due_queue() {
        let d = tempfile::tempdir().unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _g = rt.enter();
        let b = backend(d.path(), Arc::new(FauxEngine::new()));
        b.inner
            .stores
            .tracker
            .upsert_card(&review::tests::card("c1", 0))
            .unwrap();
        assert_eq!(b.snapshot().digest.due.len(), 1);
        b.apply(Command::Grade {
            card: "c1".into(),
            rating: Rating::Good,
        });
        let snap = b.snapshot();
        assert!(snap.digest.due.is_empty());
        assert_eq!(snap.slots.len(), ROLES.len());

        b.apply(Command::PinSlot {
            role: "teacher".into(),
            pin: "agy:default".into(),
        });
        assert_eq!(
            Settings::load(d.path()).unwrap().pins["teacher"],
            "agy:default"
        );
    }

    #[test]
    fn delete_removes_nodes_and_chains_and_digest_shows_project() {
        use lp_core::graph::{GraphChain, GraphNode};
        use lp_core::ports::CodingSession;
        let d = tempfile::tempdir().unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _g = rt.enter();
        let b = backend(d.path(), Arc::new(FauxEngine::new()));
        let g = &b.inner.stores.graph;
        for id in ["n_a", "n_b", "n_c"] {
            g.append_node(&GraphNode {
                id: id.into(),
                name: id.into(),
                statement: id.into(),
                aliases: vec![],
                created_at: chrono::DateTime::UNIX_EPOCH,
                provenance: vec![],
            })
            .unwrap();
        }
        let chain = |id: &str, p: &str, c: &str| GraphChain {
            id: id.into(),
            premises: vec![p.into()],
            conclusion: c.into(),
            provenance: vec![],
            created_at: chrono::DateTime::UNIX_EPOCH,
        };
        g.append_chain(&chain("ch1", "n_a", "n_b")).unwrap();
        g.append_chain(&chain("ch2", "n_b", "n_c")).unwrap();
        b.apply(Command::DeleteNodes {
            nodes: vec!["n_a".into()],
        });
        let ids: Vec<_> = g.nodes().unwrap().into_iter().map(|n| n.id).collect();
        assert_eq!(ids, ["n_b", "n_c"]);
        let chains: Vec<_> = g.chains().unwrap().into_iter().map(|c| c.id).collect();
        assert_eq!(chains, ["ch2"]);

        let t = &b.inner.stores.tracker;
        t.upsert_session(&CodingSession {
            id: "claude:s1".into(),
            source: "claude".into(),
            cwd: "/home/me/code/tracker".into(),
            git_branch: None,
            started_at: 1,
            transcript: String::new(),
        })
        .unwrap();
        t.upsert_decision(&DecisionRecord {
            id: "d1".into(),
            session_id: "claude:s1".into(),
            title: "t".into(),
            what: "w".into(),
            why: "y".into(),
            basis: DecisionBasis::Stated,
            alternatives: vec![],
            risk: String::new(),
            files: vec![],
            commit_sha: None,
        })
        .unwrap();
        assert_eq!(b.snapshot().digest.decisions[0].project, "tracker");
    }

    #[test]
    fn practice_turn_then_answer_runs_in_background() {
        let d = tempfile::tempdir().unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _g = rt.enter();
        let faux = Arc::new(FauxEngine::new());
        faux.faux_tool_call(
            "problem",
            json!({"statement": "Reverse a list", "mode": "recall", "target": [], "modelSolution": "x"}),
        );
        let b = backend(d.path(), faux);
        b.apply(Command::Practice { chains: vec![] });
        for _ in 0..200 {
            if !b.inner.live.lock().busy {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        let cl = b.snapshot().classroom;
        assert!(cl.practice);
        assert!(cl.prompt.is_some(), "{:?}", cl.transcript);
    }

    impl Backend<FauxEngine> {
        fn snapshot_transcript(&self) -> Vec<String> {
            self.snapshot().classroom.transcript
        }
    }

    fn wait_idle(b: &Backend<FauxEngine>) {
        for _ in 0..300 {
            if !b.inner.live.lock().busy {
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        panic!("turn never finished");
    }

    #[test]
    fn cancel_and_shutdown_stop_a_streaming_turn() {
        let d = tempfile::tempdir().unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _g = rt.enter();
        let faux = Arc::new(FauxEngine::new());
        // Never released: only cancellation can end these turns.
        let _gate1 =
            faux.faux_gated_turn(vec![lp_cli_driver::engine::StreamDelta::Text("hi".into())]);
        let _gate2 =
            faux.faux_gated_turn(vec![lp_cli_driver::engine::StreamDelta::Text("hi".into())]);
        let b = backend(d.path(), faux);
        b.apply(Command::StartTeach {
            topic: "raft".into(),
        });
        assert!(b.inner.live.lock().busy);
        b.apply(Command::Cancel);
        wait_idle(&b);
        b.apply(Command::StartTeach {
            topic: "again".into(),
        });
        assert!(b.inner.live.lock().busy);
        b.shutdown();
        wait_idle(&b);
    }

    #[test]
    fn start_teach_seeds_a_teach_session() {
        let d = tempfile::tempdir().unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _g = rt.enter();
        let faux = Arc::new(FauxEngine::new());
        faux.faux_text("Let's begin.");
        let b = backend(d.path(), faux);
        b.apply(Command::StartTeach {
            topic: "raft".into(),
        });
        wait_idle(&b);
        assert_eq!(
            b.inner.live.lock().session.as_ref().unwrap().kind,
            Kind::Teach
        );
        assert!(!b.snapshot_transcript().is_empty());
    }

    #[test]
    fn refresh_rescans_the_stores_only_when_they_change() {
        use lp_core::graph::GraphNode;
        let d = tempfile::tempdir().unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _g = rt.enter();
        let b = backend(d.path(), Arc::new(FauxEngine::new()));
        let scans = || b.inner.scans.load(Ordering::Relaxed);
        b.snapshot();
        b.apply(Command::Refresh);
        b.snapshot();
        assert_eq!(scans(), 1, "unchanged stores must reuse the last scan");
        b.inner
            .stores
            .graph
            .append_node(&GraphNode {
                id: "n_a".into(),
                name: "a".into(),
                statement: "a".into(),
                aliases: vec![],
                created_at: chrono::DateTime::UNIX_EPOCH,
                provenance: vec![],
            })
            .unwrap();
        assert_eq!(b.snapshot().graph.snap.nodes.len(), 1);
        assert_eq!(scans(), 2);
    }

    #[test]
    fn corrupt_store_is_reported_and_delete_does_not_wipe_chains() {
        use lp_core::graph::GraphNode;
        let d = tempfile::tempdir().unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let _g = rt.enter();
        let b = backend(d.path(), Arc::new(FauxEngine::new()));
        b.inner
            .stores
            .graph
            .append_node(&GraphNode {
                id: "n_a".into(),
                name: "a".into(),
                statement: "a".into(),
                aliases: vec![],
                created_at: chrono::DateTime::UNIX_EPOCH,
                provenance: vec![],
            })
            .unwrap();
        let chains = d.path().join("graph/chains.jsonl");
        std::fs::remove_file(&chains).ok();
        std::fs::create_dir(&chains).unwrap(); // unreadable as a ledger
        let t = b.snapshot().classroom.transcript;
        assert!(t.iter().any(|l| l.starts_with('⚠')), "{t:?}");
        b.apply(Command::DeleteNodes {
            nodes: vec!["n_a".into()],
        });
        assert!(
            chains.is_dir(),
            "delete must not rewrite chains after a failed read"
        );
    }
}
