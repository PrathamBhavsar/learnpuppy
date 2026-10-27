//! App state, key map and the 60fps event loop.
//!
//! The render loop only ever reads an `ArcSwap<ViewSnapshot>`; a worker task owns the
//! `DataSource`, applies `Command`s received over a bounded channel and swaps in a new
//! snapshot. Real stores/agents are wired behind `DataSource` in a later task.

use std::io;
use std::sync::Arc;
use std::time::Duration;

use arc_swap::ArcSwap;
use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::DefaultTerminal;
use tokio::sync::mpsc;

use crate::ui;
use crate::views::classroom::{Answer, Classroom, Dock, DockKey};
use crate::views::graph::{self, GraphData, GraphUi};
use crate::views::quiz::{DigestData, DigestUi, Rating};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Health {
    Healthy,
    Cooldown { secs_left: u64 },
    AuthExpired,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountRow {
    pub id: String,
    pub health: Health,
    pub turns: u32,
    pub tokens: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SlotPin {
    pub role: String,
    pub pin: String,
}

/// Immutable view data read lock-free by the renderer.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct ViewSnapshot {
    pub accounts: Vec<AccountRow>,
    pub slots: Vec<SlotPin>,
    pub graph: GraphData,
    pub classroom: Classroom,
    pub digest: DigestData,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    PinSlot {
        role: String,
        pin: String,
    },
    Refresh,
    Practice {
        chains: Vec<String>,
    },
    DeleteNodes {
        nodes: Vec<String>,
    },
    Answer(Answer),
    Grade {
        card: String,
        rating: Rating,
    },
    FoldDecision {
        id: String,
    },
    /// Stops the in-flight model turn.
    Cancel,
    /// Starts a Teach session on `topic`.
    StartTeach {
        topic: String,
    },
}

pub trait DataSource: Send + Sync {
    fn snapshot(&self) -> ViewSnapshot;
    fn apply(&self, cmd: Command);
    /// Called once when the UI exits: cancel whatever is still running.
    fn shutdown(&self) {}
}

pub type Shared = Arc<ArcSwap<ViewSnapshot>>;

pub async fn run_worker(src: Arc<dyn DataSource>, shared: Shared, mut rx: mpsc::Receiver<Command>) {
    while let Some(cmd) = rx.recv().await {
        src.apply(cmd);
        shared.store(Arc::new(src.snapshot()));
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum View {
    Classroom,
    Graph,
    Digest,
}

impl View {
    pub const ALL: [View; 3] = [View::Classroom, View::Graph, View::Digest];

    pub fn title(self) -> &'static str {
        match self {
            View::Classroom => "Classroom",
            View::Graph => "Graph",
            View::Digest => "Digest",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Modal {
    Palette {
        query: String,
        selected: usize,
    },
    Accounts {
        selected: usize,
    },
    /// Topic prompt for a new Teach session.
    Teach {
        topic: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Quit,
    SwitchView(View),
    NextView,
    ToggleSidebar,
    TogglePlan,
    OpenPalette,
    OpenAccounts,
    OpenTeach,
    CloseModal,
    Char(char),
    Backspace,
    Up,
    Down,
    Enter,
    Dock(DockKey),
}

/// Commands offered by the `Ctrl+K` palette.
pub const PALETTE: [(&str, Action); 8] = [
    ("Go to Classroom", Action::SwitchView(View::Classroom)),
    ("Go to Graph", Action::SwitchView(View::Graph)),
    ("Go to Digest", Action::SwitchView(View::Digest)),
    ("Toggle sidebar", Action::ToggleSidebar),
    ("Toggle plan pane", Action::TogglePlan),
    ("Open accounts and model slots", Action::OpenAccounts),
    ("Start teaching a topic", Action::OpenTeach),
    ("Quit", Action::Quit),
];

fn is_subsequence(needle: &str, hay: &str) -> bool {
    let mut hay = hay.chars().flat_map(char::to_lowercase);
    needle
        .chars()
        .flat_map(char::to_lowercase)
        .all(|n| hay.any(|h| h == n))
}

pub fn palette_matches(query: &str) -> Vec<(&'static str, Action)> {
    PALETTE
        .iter()
        .copied()
        .filter(|(label, _)| is_subsequence(query, label))
        .collect()
}

#[derive(Debug)]
pub struct App {
    pub view: View,
    pub sidebar: bool,
    pub plan_pane: bool,
    pub modal: Option<Modal>,
    pub graph: GraphUi,
    pub dock: Dock,
    pub digest: DigestUi,
    pub quit: bool,
}

impl Default for App {
    fn default() -> Self {
        Self {
            view: View::Classroom,
            sidebar: true,
            plan_pane: true,
            modal: None,
            graph: GraphUi::default(),
            dock: Dock::default(),
            digest: DigestUi::default(),
            quit: false,
        }
    }
}

impl App {
    pub fn map_key(&self, key: KeyEvent) -> Option<Action> {
        if self.modal.is_none() && self.view == View::Classroom {
            if let Some(a) = self.dock.map_key(key) {
                return Some(a);
            }
        }
        if self.modal.is_none() && self.view == View::Digest {
            if let Some(a) = self.digest.map_key(key) {
                return Some(a);
            }
        }
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        if ctrl {
            return match key.code {
                KeyCode::Char('c') => Some(Action::Quit),
                KeyCode::Char('k') => Some(Action::OpenPalette),
                KeyCode::Char('a') => Some(Action::OpenAccounts),
                KeyCode::Char('b') => Some(Action::ToggleSidebar),
                KeyCode::Char('p') => Some(Action::TogglePlan),
                _ => None,
            };
        }
        match key.code {
            KeyCode::Esc => Some(Action::CloseModal),
            KeyCode::Up => Some(Action::Up),
            KeyCode::Down => Some(Action::Down),
            KeyCode::Enter => Some(Action::Enter),
            KeyCode::Backspace => Some(Action::Backspace),
            KeyCode::Char(c)
                if matches!(
                    self.modal,
                    Some(Modal::Palette { .. } | Modal::Teach { .. })
                ) =>
            {
                Some(Action::Char(c))
            }
            KeyCode::Char(c)
                if self.modal.is_none() && self.view == View::Graph && graph::KEYS.contains(c) =>
            {
                Some(Action::Char(c))
            }
            _ if self.modal.is_some() => None,
            KeyCode::Tab => Some(Action::NextView),
            KeyCode::Char('q') => Some(Action::Quit),
            KeyCode::Char('1') => Some(Action::SwitchView(View::Classroom)),
            KeyCode::Char('2') => Some(Action::SwitchView(View::Graph)),
            KeyCode::Char('3') => Some(Action::SwitchView(View::Digest)),
            _ => None,
        }
    }

    /// Applies an action; returns a worker command when the action changes data.
    pub fn update(&mut self, action: Action, snap: &ViewSnapshot) -> Option<Command> {
        let dock_action = matches!(
            action,
            Action::Char(_) | Action::Backspace | Action::Enter | Action::Dock(_)
        );
        if dock_action && self.modal.is_none() && self.view == View::Classroom {
            return self.dock.update(action, &snap.classroom);
        }
        match (&mut self.modal, action) {
            (_, Action::Quit) => self.quit = true,
            (_, Action::OpenPalette) => {
                self.modal = Some(Modal::Palette {
                    query: String::new(),
                    selected: 0,
                });
            }
            (_, Action::OpenAccounts) => self.modal = Some(Modal::Accounts { selected: 0 }),
            (_, Action::OpenTeach) => {
                self.modal = Some(Modal::Teach {
                    topic: String::new(),
                });
            }
            (Some(Modal::Teach { topic }), Action::Char(c)) => topic.push(c),
            (Some(Modal::Teach { topic }), Action::Backspace) => {
                topic.pop();
            }
            (Some(Modal::Teach { topic }), Action::Enter) => {
                let topic = topic.trim().to_owned();
                if topic.is_empty() {
                    return None;
                }
                self.modal = None;
                self.view = View::Classroom;
                return Some(Command::StartTeach { topic });
            }
            (Some(_), Action::CloseModal) => self.modal = None,
            (None, Action::CloseModal) => return Some(Command::Cancel),
            (Some(Modal::Palette { query, selected }), Action::Char(c)) => {
                query.push(c);
                *selected = 0;
            }
            (Some(Modal::Palette { query, selected }), Action::Backspace) => {
                query.pop();
                *selected = 0;
            }
            (Some(Modal::Palette { query, selected }), Action::Up | Action::Down) => {
                let last = palette_matches(query).len().saturating_sub(1);
                *selected = step(*selected, action == Action::Down, last);
            }
            (Some(Modal::Palette { query, selected }), Action::Enter) => {
                let picked = palette_matches(query).get(*selected).map(|(_, a)| *a);
                self.modal = None;
                return picked.and_then(|a| self.update(a, snap));
            }
            (Some(Modal::Accounts { selected }), Action::Up | Action::Down) => {
                let last = snap.slots.len().saturating_sub(1);
                *selected = step(*selected, action == Action::Down, last);
            }
            (Some(Modal::Accounts { selected }), Action::Enter) => {
                let slot = snap.slots.get(*selected)?;
                return Some(Command::PinSlot {
                    role: slot.role.clone(),
                    pin: next_pin(snap, &slot.pin),
                });
            }
            (Some(_), _) => {}
            (None, Action::SwitchView(v)) => self.view = v,
            (None, Action::NextView) => {
                let i = View::ALL.iter().position(|v| *v == self.view).unwrap_or(0);
                self.view = View::ALL[(i + 1) % View::ALL.len()];
            }
            (None, Action::ToggleSidebar) => self.sidebar = !self.sidebar,
            (None, Action::TogglePlan) => self.plan_pane = !self.plan_pane,
            (None, a) if self.view == View::Graph => return self.graph.update(a, &snap.graph),
            (None, a) if self.view == View::Digest => return self.digest.update(a, &snap.digest),
            (None, _) => {}
        }
        None
    }
}

fn step(i: usize, down: bool, last: usize) -> usize {
    if down {
        (i + 1).min(last)
    } else {
        i.saturating_sub(1)
    }
}

/// Cycles `claude:auto` then each account id, wrapping.
fn next_pin(snap: &ViewSnapshot, current: &str) -> String {
    let choices: Vec<&str> = std::iter::once("claude:auto")
        .chain(snap.accounts.iter().map(|a| a.id.as_str()))
        .collect();
    let i = choices.iter().position(|c| *c == current);
    choices[i.map_or(0, |i| (i + 1) % choices.len())].to_string()
}

pub async fn run(src: Arc<dyn DataSource>) -> io::Result<()> {
    let shared: Shared = Arc::new(ArcSwap::from_pointee(src.snapshot()));
    let (tx, rx) = mpsc::channel(32);
    let worker = tokio::spawn(run_worker(src.clone(), shared.clone(), rx));
    let mut terminal = ratatui::init();
    let res = event_loop(&mut terminal, &shared, &tx).await;
    ratatui::restore();
    src.shutdown();
    drop(tx);
    let _ = worker.await;
    res
}

/// Queues a user command, waiting for room: only the periodic `Refresh` may be dropped.
/// False once the worker is gone.
async fn submit(tx: &mpsc::Sender<Command>, cmd: Command) -> bool {
    tx.send(cmd).await.is_ok()
}

async fn event_loop(
    terminal: &mut DefaultTerminal,
    shared: &Shared,
    tx: &mpsc::Sender<Command>,
) -> io::Result<()> {
    let mut app = App::default();
    let mut tick = tokio::time::interval(Duration::from_millis(16));
    let mut frames = 0u32;
    while !app.quit {
        tick.tick().await;
        let snap = shared.load_full();
        app.dock.sync(&snap.classroom);
        while event::poll(Duration::ZERO)? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }
                if let Some(cmd) = app
                    .map_key(key)
                    .and_then(|action| app.update(action, &snap))
                {
                    // A user command must not be lost; only the periodic Refresh may be dropped.
                    if !submit(tx, cmd).await {
                        break;
                    }
                }
            }
        }
        frames = frames.wrapping_add(1);
        if frames.is_multiple_of(120) {
            let _ = tx.try_send(Command::Refresh);
        }
        terminal.draw(|f| ui::render(f, &app, &snap))?;
    }
    Ok(())
}

#[cfg(test)]
pub(crate) fn sample_snapshot() -> ViewSnapshot {
    let acct = |id: &str, health| AccountRow {
        id: id.into(),
        health,
        turns: 12,
        tokens: 48_000,
    };
    ViewSnapshot {
        classroom: Default::default(),
        digest: crate::views::quiz::sample_digest(),
        accounts: vec![
            acct("claude:a@example.com", Health::Healthy),
            acct("claude:b@example.com", Health::Cooldown { secs_left: 840 }),
            acct("agy:default", Health::AuthExpired),
        ],
        slots: ["prober", "teacher", "extractor"]
            .iter()
            .map(|r| SlotPin {
                role: (*r).into(),
                pin: "claude:auto".into(),
            })
            .collect(),
        graph: crate::views::graph::sample_graph(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, mods: KeyModifiers) -> KeyEvent {
        KeyEvent::new(code, mods)
    }

    #[test]
    fn fuzzy_matches_subsequence_case_insensitively() {
        let labels: Vec<_> = palette_matches("TGL").iter().map(|(l, _)| *l).collect();
        assert_eq!(labels, ["Toggle sidebar", "Toggle plan pane"]);
        assert_eq!(palette_matches("").len(), PALETTE.len());
        assert!(palette_matches("zzz").is_empty());
    }

    #[test]
    fn global_chords_map_and_modal_swallows_plain_keys() {
        let mut app = App::default();
        let ctrl = |c| key(KeyCode::Char(c), KeyModifiers::CONTROL);
        assert_eq!(app.map_key(ctrl('k')), Some(Action::OpenPalette));
        assert_eq!(app.map_key(ctrl('a')), Some(Action::OpenAccounts));
        assert_eq!(
            app.map_key(key(KeyCode::Char('2'), KeyModifiers::NONE)),
            Some(Action::SwitchView(View::Graph))
        );
        app.modal = Some(Modal::Accounts { selected: 0 });
        assert_eq!(
            app.map_key(key(KeyCode::Char('q'), KeyModifiers::NONE)),
            None
        );
        assert_eq!(app.map_key(ctrl('c')), Some(Action::Quit));
    }

    #[test]
    fn palette_enter_runs_selected_command() {
        let s = sample_snapshot();
        let mut app = App::default();
        app.update(Action::OpenPalette, &s);
        for c in "graph".chars() {
            app.update(Action::Char(c), &s);
        }
        assert_eq!(app.update(Action::Enter, &s), None);
        assert_eq!((app.view, app.modal.clone()), (View::Graph, None));
    }

    #[test]
    fn palette_teach_prompts_for_a_topic_and_starts_a_teach_session() {
        let s = sample_snapshot();
        let mut app = App::default();
        app.update(Action::OpenPalette, &s);
        for c in "teach".chars() {
            app.update(Action::Char(c), &s);
        }
        assert_eq!(app.update(Action::Enter, &s), None);
        assert!(matches!(app.modal, Some(Modal::Teach { .. })));
        assert_eq!(
            app.update(Action::Enter, &s),
            None,
            "empty topic is ignored"
        );
        for c in "monads".chars() {
            app.update(Action::Char(c), &s);
        }
        assert_eq!(
            app.update(Action::Enter, &s),
            Some(Command::StartTeach {
                topic: "monads".into()
            })
        );
        assert_eq!(app.modal, None);
    }

    #[test]
    fn accounts_modal_enter_cycles_slot_pin() {
        let s = sample_snapshot();
        let mut app = App::default();
        app.update(Action::OpenAccounts, &s);
        app.update(Action::Down, &s);
        let cmd = app.update(Action::Enter, &s);
        assert_eq!(
            cmd,
            Some(Command::PinSlot {
                role: "teacher".into(),
                pin: "claude:a@example.com".into()
            })
        );
    }

    #[test]
    fn view_toggles_and_tab_cycle() {
        let s = ViewSnapshot::default();
        let mut app = App::default();
        app.update(Action::ToggleSidebar, &s);
        app.update(Action::TogglePlan, &s);
        assert!(!app.sidebar && !app.plan_pane);
        for _ in 0..3 {
            app.update(Action::NextView, &s);
        }
        assert_eq!(app.view, View::Classroom);
    }

    #[test]
    fn esc_without_modal_cancels_the_turn() {
        let mut app = App::default();
        let s = ViewSnapshot::default();
        assert_eq!(app.update(Action::CloseModal, &s), Some(Command::Cancel));
    }

    #[tokio::test]
    async fn submit_waits_for_room_instead_of_dropping() {
        let (tx, mut rx) = mpsc::channel(1);
        tx.send(Command::Refresh).await.unwrap();
        let sender = tokio::spawn(async move {
            submit(
                &tx,
                Command::Grade {
                    card: "c".into(),
                    rating: Rating::Good,
                },
            )
            .await
        });
        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(rx.recv().await, Some(Command::Refresh));
        assert!(sender.await.unwrap());
        assert!(matches!(rx.recv().await, Some(Command::Grade { .. })));
    }

    #[tokio::test]
    async fn worker_applies_commands_and_swaps_snapshot() {
        struct Src(std::sync::Mutex<u32>);
        impl DataSource for Src {
            fn snapshot(&self) -> ViewSnapshot {
                let n = *self.0.lock().unwrap();
                ViewSnapshot {
                    classroom: Default::default(),
                    accounts: vec![],
                    slots: vec![SlotPin {
                        role: "r".into(),
                        pin: n.to_string(),
                    }],
                    ..Default::default()
                }
            }
            fn apply(&self, _: Command) {
                *self.0.lock().unwrap() += 1;
            }
        }
        let src = Arc::new(Src(Default::default()));
        let shared: Shared = Arc::new(ArcSwap::from_pointee(src.snapshot()));
        let (tx, rx) = mpsc::channel(4);
        tx.send(Command::Refresh).await.unwrap();
        drop(tx);
        run_worker(src, shared.clone(), rx).await;
        assert_eq!(shared.load().slots[0].pin, "1");
    }
}
