//! Typed server events and the hub that fans them out to SSE streams (docs/web/03 §SSE).
//!
//! Every published event gets a hub-wide monotonic id and lands in one ring buffer: the global
//! ring or its workspace's ring. A stream for workspace `ws_x` sees global events plus `ws_x`
//! events. Reconnecting with `Last-Event-ID` replays what was missed; if the ring no longer holds
//! it (overflow, or the id is from an earlier server run) the stream starts with `resync`.
//!
//! Ids start at the hub's boot time in epoch ms, so ids from a previous server run are always
//! below the new base and are recognised as stale.
//!
//! `chat.delta` text is coalesced to at most [`DELTAS_PER_SEC`] events per chat by
//! [`DeltaCoalescer`]; [`DeltaPump`] drives it on a timer and flushes a chat's pending text
//! before any other event of that chat so ordering holds.

use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;

/// Events kept per stream (global, and each workspace) for `Last-Event-ID` replay.
pub const RING_CAPACITY: usize = 4096;

/// Upper bound on `chat.delta` events per chat per second.
pub const DELTAS_PER_SEC: u32 = 30;

/// Minimum gap between two `chat.delta` events of one chat.
pub fn delta_interval() -> Duration {
    Duration::from_millis(1000 / u64::from(DELTAS_PER_SEC) + 1)
}

/// Who receives an event: every stream, or only streams of one workspace.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Scope {
    Global,
    Workspace(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeltaKind {
    Text,
    Thinking,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ToolCallInfo {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct JobInfo {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub title: String,
    pub artifact_id: Option<String>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct IngestReport {
    pub sessions: u64,
    pub new_decisions: u64,
    pub new_cards: u64,
    pub skipped: u64,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TurnError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

/// Every event type of the SSE protocol.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    Hello {
        server_time: i64,
        version: String,
    },
    WorkspaceCreated {
        workspace_id: String,
    },
    WorkspaceUpdated {
        workspace_id: String,
    },
    WorkspaceDeleted {
        workspace_id: String,
    },
    AccountsChanged,
    GraphChanged {
        version: u64,
    },
    ChatCreated {
        chat_id: String,
    },
    ChatUpdated {
        chat_id: String,
    },
    ChatDelta {
        chat_id: String,
        turn_id: String,
        seq: u64,
        kind: DeltaKind,
        text: String,
    },
    ChatToolCall {
        chat_id: String,
        turn_id: String,
        call: ToolCallInfo,
    },
    ChatRest {
        chat_id: String,
        view_version: u64,
    },
    ChatError {
        chat_id: String,
        turn_id: String,
        error: TurnError,
    },
    JobUpdated {
        job: JobInfo,
    },
    IngestProgress {
        run_id: String,
        done: u64,
        total: u64,
        phase: String,
    },
    IngestDone {
        run_id: String,
        report: IngestReport,
    },
    ReviewChanged,
    ExportReady {
        job_id: String,
        token: String,
    },
    Resync,
}

impl Event {
    /// The SSE `event:` name.
    pub fn name(&self) -> &'static str {
        match self {
            Event::Hello { .. } => "hello",
            Event::WorkspaceCreated { .. } => "workspace.created",
            Event::WorkspaceUpdated { .. } => "workspace.updated",
            Event::WorkspaceDeleted { .. } => "workspace.deleted",
            Event::AccountsChanged => "accounts.changed",
            Event::GraphChanged { .. } => "graph.changed",
            Event::ChatCreated { .. } => "chat.created",
            Event::ChatUpdated { .. } => "chat.updated",
            Event::ChatDelta { .. } => "chat.delta",
            Event::ChatToolCall { .. } => "chat.toolCall",
            Event::ChatRest { .. } => "chat.rest",
            Event::ChatError { .. } => "chat.error",
            Event::JobUpdated { .. } => "job.updated",
            Event::IngestProgress { .. } => "ingest.progress",
            Event::IngestDone { .. } => "ingest.done",
            Event::ReviewChanged => "review.changed",
            Event::ExportReady { .. } => "export.ready",
            Event::Resync => "resync",
        }
    }

    /// The chat a `chat.*` event belongs to.
    pub fn chat_id(&self) -> Option<&str> {
        match self {
            Event::ChatCreated { chat_id }
            | Event::ChatUpdated { chat_id }
            | Event::ChatDelta { chat_id, .. }
            | Event::ChatToolCall { chat_id, .. }
            | Event::ChatRest { chat_id, .. }
            | Event::ChatError { chat_id, .. } => Some(chat_id),
            _ => None,
        }
    }
}

/// A published event with its id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Stored {
    pub id: u64,
    pub scope: Scope,
    pub event: Event,
}

impl Stored {
    /// Whether a stream for `workspace` (None = global only) receives this event.
    pub fn visible_to(&self, workspace: Option<&str>) -> bool {
        match &self.scope {
            Scope::Global => true,
            Scope::Workspace(ws) => workspace == Some(ws.as_str()),
        }
    }
}

#[derive(Default)]
struct Ring {
    buf: VecDeque<Arc<Stored>>,
    /// Highest id pushed out of this ring; replay from below it is impossible.
    evicted_upto: u64,
}

struct HubState {
    base: u64,
    last_id: u64,
    global: Ring,
    workspaces: HashMap<String, Ring>,
}

/// What a new stream gets before live events.
#[derive(Debug, PartialEq, Eq)]
pub enum Replay {
    /// Missed events in id order (empty for a fresh connection or a caught-up one).
    Events(Vec<Arc<Stored>>),
    /// The missed events are gone; the client must refetch everything. `id` is the latest id
    /// so a later reconnect replays from here.
    Resync { id: u64 },
}

/// A stream's starting point plus its live feed. Live events are exactly those published after
/// the replay snapshot, unfiltered; use [`Stored::visible_to`].
pub struct Subscription {
    /// The latest id when the stream opened; every live event has a higher id.
    pub cursor: u64,
    pub replay: Replay,
    pub live: broadcast::Receiver<Arc<Stored>>,
}

/// Process-wide event fan-out with per-stream ring buffers.
pub struct EventHub {
    capacity: usize,
    state: Mutex<HubState>,
    tx: broadcast::Sender<Arc<Stored>>,
}

impl Default for EventHub {
    fn default() -> Self {
        Self::new()
    }
}

impl EventHub {
    /// A hub whose ids start after the current epoch ms.
    pub fn new() -> Self {
        Self::with_options(RING_CAPACITY, u64::try_from(crate::now_ms()).unwrap_or(0))
    }

    /// `capacity` events per ring; the first id is `base + 1`.
    pub fn with_options(capacity: usize, base: u64) -> Self {
        let capacity = capacity.max(1);
        let (tx, _) = broadcast::channel(capacity);
        Self {
            capacity,
            state: Mutex::new(HubState {
                base,
                last_id: base,
                global: Ring::default(),
                workspaces: HashMap::new(),
            }),
            tx,
        }
    }

    /// The id of the latest published event (`base` before the first one).
    pub fn last_id(&self) -> u64 {
        self.state.lock().last_id
    }

    /// Stores and broadcasts `event`; returns its id.
    pub fn publish(&self, scope: Scope, event: Event) -> u64 {
        let mut st = self.state.lock();
        st.last_id += 1;
        let stored = Arc::new(Stored {
            id: st.last_id,
            scope,
            event,
        });
        let capacity = self.capacity;
        let ring = match &stored.scope {
            Scope::Global => &mut st.global,
            Scope::Workspace(ws) => st.workspaces.entry(ws.clone()).or_default(),
        };
        if ring.buf.len() == capacity {
            if let Some(old) = ring.buf.pop_front() {
                ring.evicted_upto = old.id;
            }
        }
        ring.buf.push_back(stored.clone());
        // No receivers is fine: the ring still holds the event for later replay.
        let _ = self.tx.send(stored.clone());
        stored.id
    }

    /// Opens a stream for `workspace` (None = global events only) resuming after `last_event_id`.
    pub fn subscribe(&self, workspace: Option<&str>, last_event_id: Option<u64>) -> Subscription {
        let st = self.state.lock();
        let live = self.tx.subscribe();
        let replay = match last_event_id {
            None => Replay::Events(Vec::new()),
            Some(last) if last < st.base || last > st.last_id => Replay::Resync { id: st.last_id },
            Some(last) => {
                let rings: Vec<&Ring> = std::iter::once(&st.global)
                    .chain(workspace.and_then(|ws| st.workspaces.get(ws)))
                    .collect();
                if rings.iter().any(|r| r.evicted_upto > last) {
                    Replay::Resync { id: st.last_id }
                } else {
                    let mut missed: Vec<Arc<Stored>> = rings
                        .iter()
                        .flat_map(|r| r.buf.iter().filter(|e| e.id > last).cloned())
                        .collect();
                    missed.sort_by_key(|e| e.id);
                    Replay::Events(missed)
                }
            }
        };
        Subscription {
            cursor: st.last_id,
            replay,
            live,
        }
    }
}

struct ChatBuf {
    workspace: String,
    turn_id: String,
    kind: DeltaKind,
    text: String,
    next_seq: u64,
    last_emit: Option<Instant>,
}

impl ChatBuf {
    fn take(&mut self, chat_id: &str, now: Instant) -> Option<(Scope, Event)> {
        if self.text.is_empty() {
            return None;
        }
        self.next_seq += 1;
        self.last_emit = Some(now);
        Some((
            Scope::Workspace(self.workspace.clone()),
            Event::ChatDelta {
                chat_id: chat_id.to_owned(),
                turn_id: self.turn_id.clone(),
                seq: self.next_seq,
                kind: self.kind,
                text: std::mem::take(&mut self.text),
            },
        ))
    }
}

/// Merges streamed text so each chat emits at most one `chat.delta` per interval. Pure: the
/// caller supplies the clock. `seq` counts from 1 per turn with no gaps.
pub struct DeltaCoalescer {
    interval: Duration,
    chats: HashMap<String, ChatBuf>,
}

impl DeltaCoalescer {
    pub fn new(interval: Duration) -> Self {
        Self {
            interval,
            chats: HashMap::new(),
        }
    }

    /// Adds streamed text; returns the events to publish now, in order. A switch of turn or of
    /// text/thinking kind emits the pending text first.
    pub fn push(
        &mut self,
        now: Instant,
        workspace: &str,
        chat_id: &str,
        turn_id: &str,
        kind: DeltaKind,
        text: &str,
    ) -> Vec<(Scope, Event)> {
        let mut out = Vec::new();
        let buf = self
            .chats
            .entry(chat_id.to_owned())
            .or_insert_with(|| ChatBuf {
                workspace: workspace.to_owned(),
                turn_id: turn_id.to_owned(),
                kind,
                text: String::new(),
                next_seq: 0,
                last_emit: None,
            });
        if buf.turn_id != turn_id {
            out.extend(buf.take(chat_id, now));
            buf.turn_id = turn_id.to_owned();
            buf.next_seq = 0;
            buf.last_emit = None;
        } else if buf.kind != kind {
            out.extend(buf.take(chat_id, now));
        }
        buf.workspace = workspace.to_owned();
        buf.kind = kind;
        buf.text.push_str(text);
        let due = buf
            .last_emit
            .is_none_or(|t| now.saturating_duration_since(t) >= self.interval);
        if due {
            out.extend(buf.take(chat_id, now));
        }
        out
    }

    /// Emits pending text whose chat has waited a full interval.
    pub fn flush_due(&mut self, now: Instant) -> Vec<(Scope, Event)> {
        let interval = self.interval;
        let mut out: Vec<(Scope, Event)> = self
            .chats
            .iter_mut()
            .filter(|(_, b)| {
                b.last_emit
                    .is_none_or(|t| now.saturating_duration_since(t) >= interval)
            })
            .filter_map(|(id, b)| b.take(id, now))
            .collect();
        out.sort_by(|a, b| a.1.chat_id().cmp(&b.1.chat_id()));
        out
    }

    /// Emits a chat's pending text immediately, e.g. before its `chat.rest`.
    pub fn flush_chat(&mut self, chat_id: &str, now: Instant) -> Option<(Scope, Event)> {
        self.chats
            .get_mut(chat_id)
            .and_then(|b| b.take(chat_id, now))
    }

    /// Whether any text is waiting.
    pub fn is_idle(&self) -> bool {
        self.chats.values().all(|b| b.text.is_empty())
    }
}

/// Publishes through a [`DeltaCoalescer`] on a timer. All `chat.*` events of a streaming chat
/// must go through [`DeltaPump::publish`] so pending deltas are flushed ahead of them.
pub struct DeltaPump {
    hub: Arc<EventHub>,
    coalescer: Mutex<DeltaCoalescer>,
}

impl DeltaPump {
    pub fn new(hub: Arc<EventHub>) -> Self {
        Self::with_interval(hub, delta_interval())
    }

    pub fn with_interval(hub: Arc<EventHub>, interval: Duration) -> Self {
        Self {
            hub,
            coalescer: Mutex::new(DeltaCoalescer::new(interval)),
        }
    }

    pub fn hub(&self) -> &Arc<EventHub> {
        &self.hub
    }

    /// Streams text of a running turn.
    pub fn delta(
        &self,
        workspace: &str,
        chat_id: &str,
        turn_id: &str,
        kind: DeltaKind,
        text: &str,
    ) {
        let mut co = self.coalescer.lock();
        for (scope, ev) in co.push(Instant::now(), workspace, chat_id, turn_id, kind, text) {
            self.hub.publish(scope, ev);
        }
    }

    /// Publishes `event`, first flushing pending deltas of its chat.
    pub fn publish(&self, scope: Scope, event: Event) -> u64 {
        let mut co = self.coalescer.lock();
        if let Some(chat) = event.chat_id() {
            if let Some((s, ev)) = co.flush_chat(chat, Instant::now()) {
                self.hub.publish(s, ev);
            }
        }
        self.hub.publish(scope, event)
    }

    /// Emits trailing text once its interval has passed.
    pub fn tick(&self) {
        let mut co = self.coalescer.lock();
        if co.is_idle() {
            return;
        }
        for (scope, ev) in co.flush_due(Instant::now()) {
            self.hub.publish(scope, ev);
        }
    }

    /// Ticks until `stop` is cancelled.
    pub fn spawn(self: &Arc<Self>, stop: CancellationToken) -> tokio::task::JoinHandle<()> {
        let pump = self.clone();
        tokio::spawn(async move {
            let interval = pump.coalescer.lock().interval;
            let mut timer = tokio::time::interval(interval);
            timer.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                tokio::select! {
                    () = stop.cancelled() => {
                        pump.tick_all();
                        break;
                    }
                    _ = timer.tick() => pump.tick(),
                }
            }
        })
    }

    /// Emits every pending delta (shutdown).
    fn tick_all(&self) {
        let mut co = self.coalescer.lock();
        let ids: Vec<String> = co.chats.keys().cloned().collect();
        let now = Instant::now();
        for id in ids {
            if let Some((scope, ev)) = co.flush_chat(&id, now) {
                self.hub.publish(scope, ev);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ws(id: &str) -> Scope {
        Scope::Workspace(id.into())
    }

    fn graph(v: u64) -> Event {
        Event::GraphChanged { version: v }
    }

    fn ids(r: &Replay) -> Vec<u64> {
        match r {
            Replay::Events(evs) => evs.iter().map(|e| e.id).collect(),
            Replay::Resync { .. } => panic!("unexpected resync"),
        }
    }

    #[test]
    fn ids_are_monotonic_from_base() {
        let hub = EventHub::with_options(8, 100);
        assert_eq!(hub.last_id(), 100);
        assert_eq!(hub.publish(Scope::Global, Event::AccountsChanged), 101);
        assert_eq!(hub.publish(ws("ws_a"), graph(1)), 102);
        assert_eq!(hub.publish(ws("ws_b"), graph(1)), 103);
        assert_eq!(hub.last_id(), 103);
    }

    #[test]
    fn replay_merges_global_and_own_workspace_in_order() {
        let hub = EventHub::with_options(8, 0);
        hub.publish(Scope::Global, Event::AccountsChanged); // 1
        hub.publish(ws("ws_a"), graph(1)); // 2
        hub.publish(ws("ws_b"), graph(1)); // 3
        hub.publish(Scope::Global, Event::ReviewChanged); // 4
        hub.publish(ws("ws_a"), graph(2)); // 5
        assert_eq!(ids(&hub.subscribe(Some("ws_a"), Some(1)).replay), [2, 4, 5]);
        assert_eq!(ids(&hub.subscribe(Some("ws_b"), Some(0)).replay), [1, 3, 4]);
        assert_eq!(ids(&hub.subscribe(None, Some(0)).replay), [1, 4]);
        assert_eq!(
            ids(&hub.subscribe(Some("ws_a"), Some(5)).replay),
            [] as [u64; 0]
        );
        assert_eq!(
            ids(&hub.subscribe(Some("ws_a"), None).replay),
            [] as [u64; 0]
        );
    }

    #[test]
    fn overflow_and_foreign_ids_resync() {
        let hub = EventHub::with_options(3, 10);
        for v in 0..5 {
            hub.publish(ws("ws_a"), graph(v)); // 11..=15, ring keeps 13..=15
        }
        assert_eq!(
            hub.subscribe(Some("ws_a"), Some(11)).replay,
            Replay::Resync { id: 15 }
        );
        // 12 was evicted but the client already has it: nothing is lost.
        assert_eq!(
            ids(&hub.subscribe(Some("ws_a"), Some(12)).replay),
            [13, 14, 15]
        );
        // Another workspace's overflow does not affect this stream.
        assert_eq!(
            ids(&hub.subscribe(Some("ws_b"), Some(11)).replay),
            [] as [u64; 0]
        );
        // From a previous server run (below base) or from the future.
        assert_eq!(
            hub.subscribe(Some("ws_a"), Some(3)).replay,
            Replay::Resync { id: 15 }
        );
        assert_eq!(
            hub.subscribe(Some("ws_a"), Some(99)).replay,
            Replay::Resync { id: 15 }
        );
    }

    #[test]
    fn live_feed_starts_right_after_the_snapshot() {
        let hub = EventHub::with_options(8, 0);
        hub.publish(Scope::Global, Event::AccountsChanged);
        let mut sub = hub.subscribe(Some("ws_a"), Some(0));
        assert_eq!(ids(&sub.replay), [1]);
        assert_eq!(sub.cursor, 1);
        hub.publish(ws("ws_b"), graph(1));
        hub.publish(ws("ws_a"), graph(2));
        let a = sub.live.try_recv().unwrap();
        let b = sub.live.try_recv().unwrap();
        assert_eq!((a.id, b.id), (2, 3));
        assert!(!a.visible_to(Some("ws_a")) && b.visible_to(Some("ws_a")));
        assert!(sub.live.try_recv().is_err());
    }

    #[test]
    fn event_names_match_the_protocol() {
        let e = Event::ChatToolCall {
            chat_id: "c".into(),
            turn_id: "t".into(),
            call: ToolCallInfo {
                id: "1".into(),
                name: "quiz".into(),
            },
        };
        assert_eq!((e.name(), e.chat_id()), ("chat.toolCall", Some("c")));
        assert_eq!(Event::Resync.name(), "resync");
        assert_eq!(Event::AccountsChanged.chat_id(), None);
    }

    fn text(evs: &[(Scope, Event)]) -> Vec<(u64, String)> {
        evs.iter()
            .map(|(_, e)| match e {
                Event::ChatDelta { seq, text, .. } => (*seq, text.clone()),
                other => panic!("{other:?}"),
            })
            .collect()
    }

    #[test]
    fn coalescer_caps_rate_and_keeps_seq_gapless() {
        let t0 = Instant::now();
        let ms = |n| t0 + Duration::from_millis(n);
        let mut co = DeltaCoalescer::new(Duration::from_millis(34));
        let k = DeltaKind::Text;
        // The first delta goes out at once.
        assert_eq!(
            text(&co.push(t0, "ws", "c", "t1", k, "a")),
            [(1, "a".into())]
        );
        // Within the interval text accumulates.
        assert!(co.push(ms(5), "ws", "c", "t1", k, "b").is_empty());
        assert!(co.push(ms(10), "ws", "c", "t1", k, "c").is_empty());
        assert!(co.flush_due(ms(20)).is_empty());
        assert_eq!(text(&co.flush_due(ms(34))), [(2, "bc".into())]);
        // 1000 tokens in one second become at most 30 events.
        let mut n = 0;
        for i in 0..1000 {
            n += co.push(ms(100 + i), "ws", "c", "t1", k, "x").len();
            n += co.flush_due(ms(100 + i)).len();
        }
        assert!(n <= 30, "{n} events in one second");
        assert!(co.flush_chat("c", ms(1100)).is_some());
        assert!(co.is_idle());
    }

    #[test]
    fn coalescer_flushes_on_kind_or_turn_change() {
        let t0 = Instant::now();
        let mut co = DeltaCoalescer::new(Duration::from_millis(34));
        co.push(t0, "ws", "c", "t1", DeltaKind::Thinking, "hm");
        co.push(t0, "ws", "c", "t1", DeltaKind::Thinking, "m");
        let out = co.push(t0, "ws", "c", "t1", DeltaKind::Text, "Hi");
        assert!(matches!(
            &out[..],
            [(_, Event::ChatDelta { seq: 2, kind: DeltaKind::Thinking, text, .. })] if text == "m"
        ));
        let out = co.push(t0, "ws", "c", "t2", DeltaKind::Text, "new");
        // Pending "Hi" of t1 first, then t2 restarts its seq at 1.
        assert!(matches!(
            &out[..],
            [
                (_, Event::ChatDelta { turn_id: a, seq: 3, .. }),
                (Scope::Workspace(w), Event::ChatDelta { turn_id: b, seq: 1, .. }),
            ] if a == "t1" && b == "t2" && w == "ws"
        ));
    }

    #[tokio::test]
    async fn pump_flushes_deltas_before_rest_and_on_timer() {
        let hub = Arc::new(EventHub::with_options(64, 0));
        let pump = Arc::new(DeltaPump::with_interval(
            hub.clone(),
            Duration::from_millis(20),
        ));
        let mut sub = hub.subscribe(Some("ws"), None);
        pump.delta("ws", "c", "t", DeltaKind::Text, "a");
        pump.delta("ws", "c", "t", DeltaKind::Text, "b");
        pump.publish(
            ws("ws"),
            Event::ChatRest {
                chat_id: "c".into(),
                view_version: 2,
            },
        );
        let names: Vec<_> = std::iter::from_fn(|| sub.live.try_recv().ok())
            .map(|e| e.event.clone())
            .collect();
        assert!(matches!(
            &names[..],
            [
                Event::ChatDelta { seq: 1, .. },
                Event::ChatDelta { seq: 2, text, .. },
                Event::ChatRest { .. },
            ] if text == "b"
        ));

        let stop = CancellationToken::new();
        let task = pump.spawn(stop.clone());
        pump.delta("ws", "c", "t", DeltaKind::Text, "c");
        pump.delta("ws", "c", "t", DeltaKind::Text, "d");
        // "c" and "d" arrive as one or two deltas depending on timing; the timer delivers the
        // trailing text and seq continues without gaps.
        let (seqs, text) = tokio::time::timeout(Duration::from_secs(2), async {
            let (mut seqs, mut all) = (Vec::new(), String::new());
            while !all.ends_with('d') {
                if let Event::ChatDelta { seq, text, .. } = &sub.live.recv().await.unwrap().event {
                    seqs.push(*seq);
                    all.push_str(text);
                }
            }
            (seqs, all)
        })
        .await
        .unwrap();
        assert_eq!(text, "cd");
        assert_eq!(seqs, (3..3 + seqs.len() as u64).collect::<Vec<_>>());
        stop.cancel();
        task.await.unwrap();
    }
}
