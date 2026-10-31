//! Every model call of a chat turn goes through [`Gated`]: it waits for a slot of the process-wide
//! CLI semaphore (docs/web/01 §Concurrency), fires scripted faults, remembers the streamed text of
//! the current step (kept as `interrupted` on cancel) and, when the CLI pool is rate limited,
//! queues the turn and retries once the cooldown is over, or gives up after the queue limit.

use std::collections::VecDeque;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use lp_cli_driver::accounts::{parse_cooldown, AccountHealth, AccountPool};
use lp_cli_driver::engine::{
    DriverError, LlmEngine, StreamDelta, StructuredOutput, StructuredRequest, TurnRequest,
    TurnResponse,
};
use parking_lot::Mutex;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};
use tokio_util::sync::CancellationToken;

use super::script::FaultPlan;

/// Wait assumed when a rate-limit message names no reset time and no account is cooling down.
const DEFAULT_RATE_LIMIT_WAIT: Duration = Duration::from_secs(30);
const FORWARD_CAP: usize = 256;

/// The process-wide cap on concurrent CLI subprocesses. Resizable from settings.
pub struct CliPermits {
    sem: Arc<Semaphore>,
    size: Mutex<usize>,
    waiters: Mutex<VecDeque<(String, VecDeque<u64>)>>,
    changed: tokio::sync::Notify,
    ticket: AtomicU64,
}

impl CliPermits {
    pub fn new(size: usize) -> Self {
        let size = size.max(1);
        Self {
            sem: Arc::new(Semaphore::new(size)),
            size: Mutex::new(size),
            waiters: Mutex::default(),
            changed: tokio::sync::Notify::new(),
            ticket: AtomicU64::new(0),
        }
    }

    pub fn size(&self) -> usize {
        *self.size.lock()
    }

    /// Permits free right now.
    pub fn available(&self) -> usize {
        self.sem.available_permits()
    }

    /// Changes the cap. Shrinking takes free permits now and the rest as running calls finish.
    pub fn resize(&self, size: usize) {
        let size = size.max(1);
        let mut cur = self.size.lock();
        if size > *cur {
            self.sem.add_permits(size - *cur);
        } else if size < *cur {
            let want = *cur - size;
            let owed = want - self.sem.forget_permits(want);
            if owed > 0 {
                let sem = self.sem.clone();
                if let Ok(rt) = tokio::runtime::Handle::try_current() {
                    std::mem::drop(rt.spawn(async move {
                        if let Ok(p) = sem.acquire_many_owned(owed as u32).await {
                            p.forget();
                        }
                    }));
                }
            }
        }
        *cur = size;
    }

    pub fn try_acquire(&self) -> Option<OwnedSemaphorePermit> {
        let waiting = self.waiters.lock();
        if !waiting.is_empty() {
            return None;
        }
        self.sem.clone().try_acquire_owned().ok()
    }

    /// FIFO within a workspace, round-robin across workspaces (foreground and jobs share it).
    pub async fn acquire(
        &self,
        workspace: &str,
        cancel: &CancellationToken,
    ) -> Result<OwnedSemaphorePermit, DriverError> {
        let id = self.ticket.fetch_add(1, Ordering::Relaxed);
        {
            let mut waiting = self.waiters.lock();
            match waiting.iter_mut().find(|(ws, _)| ws == workspace) {
                Some((_, ids)) => ids.push_back(id),
                None => waiting.push_back((workspace.to_owned(), VecDeque::from([id]))),
            }
        }
        let _ticket = QueueTicket {
            permits: self,
            workspace,
            id,
        };
        loop {
            let changed = self.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            let first = self
                .waiters
                .lock()
                .front()
                .is_some_and(|(ws, ids)| ws == workspace && ids.front() == Some(&id));
            if first {
                return tokio::select! {
                    biased;
                    () = cancel.cancelled() => Err(DriverError::Cancelled),
                    p = self.sem.clone().acquire_owned() => p.map_err(|_| DriverError::Cancelled),
                };
            }
            tokio::select! {
                () = cancel.cancelled() => return Err(DriverError::Cancelled),
                () = &mut changed => {},
            }
        }
    }
}

struct QueueTicket<'a> {
    permits: &'a CliPermits,
    workspace: &'a str,
    id: u64,
}
impl Drop for QueueTicket<'_> {
    fn drop(&mut self) {
        let mut waiting = self.permits.waiters.lock();
        if let Some(at) = waiting.iter().position(|(ws, _)| ws == self.workspace) {
            if let Some((ws, mut ids)) = waiting.remove(at) {
                let head = ids.front() == Some(&self.id);
                ids.retain(|id| *id != self.id);
                if !ids.is_empty() {
                    if at == 0 && head {
                        waiting.push_back((ws, ids));
                    } else {
                        waiting.insert(at, (ws, ids));
                    }
                }
            }
        }
        self.permits.changed.notify_waiters();
    }
}

/// How long to wait before retrying `e`, if it says the CLI pool is rate limited.
pub fn rate_limit_wait(e: &DriverError, pool: &AccountPool) -> Option<Duration> {
    let DriverError::Exit { stderr, .. } = e else {
        return None;
    };
    let low = stderr.to_ascii_lowercase();
    let limited = [
        "rate limit",
        "rate_limit",
        "usage limit",
        "limit reached",
        "too many requests",
        "status 429",
        "error 429",
    ]
    .iter()
    .any(|p| low.contains(p));
    let cooling = pool_cooldown(pool);
    // `CliPoolEngine` reports exhausted failover as "no usable CLI account" / "<id> unavailable";
    // that is a rate limit only while some account is cooling down.
    let exhausted = low.contains("no usable cli account") || low.ends_with(" unavailable");
    if limited {
        let epoch = u64::try_from(crate::now_ms() / 1000).unwrap_or(0);
        Some(
            parse_cooldown(stderr, epoch)
                .or(cooling)
                .unwrap_or(DEFAULT_RATE_LIMIT_WAIT),
        )
    } else if exhausted {
        cooling
    } else {
        None
    }
}

/// Shortest remaining cooldown of an enabled account.
pub fn pool_cooldown(pool: &AccountPool) -> Option<Duration> {
    let now = Instant::now();
    pool.snapshot()
        .into_iter()
        .filter(|a| !a.disabled)
        .filter_map(|a| match a.health {
            AccountHealth::Cooldown { until, .. } => Some(until.saturating_duration_since(now)),
            _ => None,
        })
        .min()
}

/// What the gate reports to the turn that owns it.
pub trait GateObserver: Send + Sync {
    /// Waiting for a CLI slot (`retry_at = None`) or for a rate limit to pass (epoch ms).
    fn queued(&self, retry_at: Option<i64>);
    /// A model call is running.
    fn running(&self);
    /// Guidance received during the preceding model step, also saved by the turn runner.
    fn guidance(&self) -> String {
        String::new()
    }
}

/// Shared by all model calls of one turn.
pub struct TurnGate {
    pub workspace: String,
    pub permits: Arc<CliPermits>,
    pub faults: Arc<FaultPlan>,
    pub pool: Arc<AccountPool>,
    /// The longest a turn waits in the rate-limit queue before the error is surfaced.
    pub queue_limit: Duration,
    pub observer: Arc<dyn GateObserver>,
    /// Text streamed by the current model call (cleared when a call starts).
    pub partial: Mutex<String>,
    /// Set when a rate limit outlasted the queue: the wait the learner is told, in ms.
    pub surfaced_retry_after: Mutex<Option<u64>>,
}

impl TurnGate {
    async fn permit(
        &self,
        cancel: &CancellationToken,
    ) -> Result<OwnedSemaphorePermit, DriverError> {
        if let Some(p) = self.permits.try_acquire() {
            return Ok(p);
        }
        self.observer.queued(None);
        let p = self.permits.acquire(&self.workspace, cancel).await?;
        self.observer.running();
        Ok(p)
    }
}

/// An engine behind a [`TurnGate`].
pub struct Gated<E> {
    pub inner: Arc<E>,
    pub gate: Arc<TurnGate>,
}

impl<E: LlmEngine> Gated<E> {
    /// Runs one call, recording its text and passing every delta on.
    async fn forward(
        &self,
        mut req: TurnRequest,
        out: &mpsc::Sender<StreamDelta>,
        cancel: CancellationToken,
    ) -> Result<TurnResponse, DriverError> {
        let (tx, mut rx) = mpsc::channel(FORWARD_CAP);
        req.prompt.push_str(&self.gate.observer.guidance());
        let call = self.inner.stream_turn(req, tx, cancel);
        let pass = async {
            while let Some(d) = rx.recv().await {
                if let StreamDelta::Text(t) = &d {
                    self.gate.partial.lock().push_str(t);
                }
                // A closed receiver only means nobody watches the stream.
                let _ = out.send(d).await;
            }
        };
        let (res, ()) = tokio::join!(call, pass);
        res
    }
}

#[async_trait]
impl<E: LlmEngine> LlmEngine for Gated<E> {
    async fn stream_turn(
        &self,
        req: TurnRequest,
        event_tx: mpsc::Sender<StreamDelta>,
        cancel: CancellationToken,
    ) -> Result<TurnResponse, DriverError> {
        let mut queued_since = None;
        loop {
            self.gate.partial.lock().clear();
            let permit = self.gate.permit(&cancel).await?;
            let res = match self.gate.faults.next() {
                Some(f) => {
                    if let Some(text) = &f.partial_text {
                        self.gate.partial.lock().push_str(text);
                        let _ = event_tx.send(StreamDelta::Text(text.clone())).await;
                    }
                    Err(f.error())
                }
                None => self.forward(req.clone(), &event_tx, cancel.clone()).await,
            };
            drop(permit);
            let err = match res {
                Err(e) if !cancel.is_cancelled() => e,
                other => {
                    if other.is_ok() {
                        self.gate.partial.lock().clear();
                    }
                    return other;
                }
            };
            let Some(wait) = rate_limit_wait(&err, &self.gate.pool) else {
                return Err(err);
            };
            let started = *queued_since.get_or_insert_with(Instant::now);
            let remaining = self.gate.queue_limit.saturating_sub(started.elapsed());
            let retry_at =
                crate::now_ms().saturating_add(i64::try_from(wait.as_millis()).unwrap_or(i64::MAX));
            tracing::info!(
                wait_ms = wait.as_millis() as u64,
                "turn queued on a rate limit"
            );
            self.gate.observer.queued(Some(retry_at));
            tokio::select! {
                () = cancel.cancelled() => return Err(DriverError::Cancelled),
                () = tokio::time::sleep(wait.min(remaining)) => {}
            }
            if wait > remaining {
                let retry_after = wait.saturating_sub(remaining);
                *self.gate.surfaced_retry_after.lock() =
                    Some(u64::try_from(retry_after.as_millis()).unwrap_or(u64::MAX));
                return Err(err);
            }
            self.gate.observer.running();
        }
    }

    async fn complete_structured<T: DeserializeOwned + JsonSchema + Send>(
        &self,
        req: StructuredRequest,
        cancel: CancellationToken,
    ) -> Result<StructuredOutput<T>, DriverError> {
        let _permit = self.gate.permit(&cancel).await?;
        self.inner.complete_structured(req, cancel).await
    }
}

/// A workbench engine: waits for a CLI slot, nothing else.
pub struct Permitted<E> {
    pub workspace: String,
    pub inner: Arc<E>,
    pub permits: Arc<CliPermits>,
}

#[async_trait]
impl<E: LlmEngine> LlmEngine for Permitted<E> {
    async fn stream_turn(
        &self,
        req: TurnRequest,
        event_tx: mpsc::Sender<StreamDelta>,
        cancel: CancellationToken,
    ) -> Result<TurnResponse, DriverError> {
        let _permit = self.permits.acquire(&self.workspace, &cancel).await?;
        self.inner.stream_turn(req, event_tx, cancel).await
    }

    async fn complete_structured<T: DeserializeOwned + JsonSchema + Send>(
        &self,
        req: StructuredRequest,
        cancel: CancellationToken,
    ) -> Result<StructuredOutput<T>, DriverError> {
        let _permit = self.permits.acquire(&self.workspace, &cancel).await?;
        self.inner.complete_structured(req, cancel).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat::script::{Fault, FaultKind};
    use lp_cli_driver::accounts::Strategy;
    use lp_cli_driver::engine::FauxEngine;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[derive(Default)]
    struct Seen {
        queued: AtomicUsize,
        running: AtomicUsize,
    }

    impl GateObserver for Seen {
        fn queued(&self, _: Option<i64>) {
            self.queued.fetch_add(1, Ordering::SeqCst);
        }
        fn running(&self) {
            self.running.fetch_add(1, Ordering::SeqCst);
        }
    }

    fn gate(limit: Duration, seen: Arc<Seen>, permits: Arc<CliPermits>) -> Arc<TurnGate> {
        Arc::new(TurnGate {
            workspace: "test".into(),
            permits,
            faults: Arc::new(FaultPlan::default()),
            pool: Arc::new(AccountPool::new(vec![], Strategy::Priority)),
            queue_limit: limit,
            observer: seen,
            partial: Mutex::default(),
            surfaced_retry_after: Mutex::default(),
        })
    }

    fn rate_limit(msg: &str) -> Fault {
        Fault {
            kind: FaultKind::RateLimit,
            message: Some(msg.into()),
            partial_text: None,
        }
    }

    #[test]
    fn rate_limits_are_recognised_with_their_wait() {
        let pool = AccountPool::new(vec![], Strategy::Priority);
        let exit = |s: &str| DriverError::Exit {
            code: Some(1),
            stderr: s.into(),
        };
        assert_eq!(
            rate_limit_wait(&exit("Rate limit reached; try again in 2 minutes"), &pool),
            Some(Duration::from_secs(120))
        );
        assert_eq!(
            rate_limit_wait(&exit("HTTP status 429"), &pool),
            Some(DEFAULT_RATE_LIMIT_WAIT)
        );
        assert_eq!(rate_limit_wait(&exit("segfault"), &pool), None);
        // Exhausted failover with nobody cooling down is not a rate limit.
        assert_eq!(
            rate_limit_wait(&exit("no usable CLI account; run auth"), &pool),
            None
        );
        assert_eq!(rate_limit_wait(&DriverError::FauxExhausted, &pool), None);
    }

    #[tokio::test]
    async fn short_rate_limit_queues_then_retries() {
        let faux = Arc::new(FauxEngine::new());
        faux.faux_text("hello");
        let seen = Arc::new(Seen::default());
        let g = gate(
            Duration::from_secs(5),
            seen.clone(),
            Arc::new(CliPermits::new(1)),
        );
        g.faults
            .schedule(0, rate_limit("rate limit; try again in 1 seconds"));
        let e = Gated {
            inner: faux,
            gate: g.clone(),
        };
        let (tx, _rx) = mpsc::channel(8);
        let t0 = Instant::now();
        let r = e
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(r.text, "hello");
        assert!(t0.elapsed() >= Duration::from_millis(900));
        assert_eq!(seen.queued.load(Ordering::SeqCst), 1);
        assert!(g.partial.lock().is_empty());
        assert!(g.surfaced_retry_after.lock().is_none());
    }

    #[tokio::test]
    async fn long_rate_limit_is_surfaced_with_its_wait() {
        let g = gate(
            Duration::from_millis(20),
            Arc::new(Seen::default()),
            Arc::new(CliPermits::new(1)),
        );
        g.faults
            .schedule(0, rate_limit("usage limit reached; try again in 5 minutes"));
        let e = Gated {
            inner: Arc::new(FauxEngine::new()),
            gate: g.clone(),
        };
        let (tx, _rx) = mpsc::channel(8);
        let r = e
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await;
        assert!(matches!(r, Err(DriverError::Exit { .. })));
        assert!((299_900..=300_000).contains(&g.surfaced_retry_after.lock().unwrap()));
    }

    #[tokio::test]
    async fn calls_wait_for_a_cli_slot_and_the_cap_resizes() {
        let permits = Arc::new(CliPermits::new(1));
        let held = permits.try_acquire().unwrap();
        let faux = Arc::new(FauxEngine::new());
        faux.faux_text("x");
        let seen = Arc::new(Seen::default());
        let e = Gated {
            inner: faux,
            gate: gate(Duration::from_secs(5), seen.clone(), permits.clone()),
        };
        let task = tokio::spawn(async move {
            let (tx, _rx) = mpsc::channel(8);
            e.stream_turn(TurnRequest::default(), tx, CancellationToken::new())
                .await
        });
        while seen.queued.load(Ordering::SeqCst) == 0 {
            tokio::task::yield_now().await;
        }
        drop(held);
        assert_eq!(task.await.unwrap().unwrap().text, "x");
        assert_eq!(seen.running.load(Ordering::SeqCst), 1);

        permits.resize(3);
        assert_eq!((permits.size(), permits.available()), (3, 3));
        let p = permits.try_acquire().unwrap();
        permits.resize(1);
        // Two free permits are taken now; the held one stays usable until dropped.
        assert_eq!(permits.available(), 0);
        drop(p);
        assert_eq!(permits.available(), 1);
    }
    #[tokio::test]
    async fn workspace_round_robin_preserves_fifo_and_cancellation_removes_waiters() {
        let permits = Arc::new(CliPermits::new(1));
        let held = permits.try_acquire().unwrap();
        let (sent, mut received) = mpsc::channel(3);
        let mut tasks = Vec::new();
        for (workspace, label) in [("a", "a1"), ("a", "a2"), ("b", "b1")] {
            let (p, tx) = (permits.clone(), sent.clone());
            tasks.push(tokio::spawn(async move {
                let _permit = p
                    .acquire(workspace, &CancellationToken::new())
                    .await
                    .unwrap();
                tx.send(label).await.unwrap();
            }));
            tokio::task::yield_now().await;
        }
        drop(held);
        let mut labels = Vec::new();
        for _ in 0..3 {
            labels.push(received.recv().await.unwrap());
        }
        assert_eq!(labels, ["a1", "b1", "a2"]);
        for task in tasks {
            task.await.unwrap();
        }
        let held = permits.try_acquire().unwrap();
        let cancel = CancellationToken::new();
        let p = permits.clone();
        let c = cancel.clone();
        let waiter = tokio::spawn(async move { p.acquire("cancelled", &c).await });
        tokio::task::yield_now().await;
        cancel.cancel();
        assert!(matches!(waiter.await.unwrap(), Err(DriverError::Cancelled)));
        drop(held);
        assert!(permits.try_acquire().is_some());
    }
}
