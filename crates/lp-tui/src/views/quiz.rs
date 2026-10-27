//! Tracker digest explorer (decision records) and the FSRS 4-choice spaced-repetition quiz.
//!
//! Scheduling lives behind `DataSource`: the view only emits `Command::Grade` and reads the
//! remaining due cards from the next snapshot.

use std::collections::BTreeSet;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph, Wrap};
use ratatui::Frame;

use crate::app::{Action, Command};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rating {
    Again,
    Hard,
    Good,
    Easy,
}

impl Rating {
    pub const ALL: [Rating; 4] = [Rating::Again, Rating::Hard, Rating::Good, Rating::Easy];

    fn label(self) -> &'static str {
        match self {
            Rating::Again => "Again",
            Rating::Hard => "Hard",
            Rating::Good => "Good",
            Rating::Easy => "Easy",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub id: String,
    pub project: String,
    pub tool: String,
    pub what: String,
    pub why: String,
    pub stated: bool,
    pub alternatives: Vec<String>,
    pub risk: String,
    pub commit: String,
    pub folded: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct QuizCard {
    pub id: String,
    pub deck: String,
    pub kind: String,
    pub question: String,
    pub options: [String; 4],
    pub correct: usize,
    pub rationale: String,
    pub source: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DigestData {
    pub decisions: Vec<Decision>,
    /// Cards due now, in review order.
    pub due: Vec<QuizCard>,
    pub streak: u32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Digest,
    Quiz,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DigestUi {
    tab: Tab,
    tool: Option<String>,
    cursor: usize,
    picked: Option<usize>,
    /// Cards graded locally whose removal the snapshot has not caught up with yet.
    graded: BTreeSet<String>,
}

fn visible<'a>(ui: &DigestUi, data: &'a DigestData) -> Vec<&'a Decision> {
    data.decisions
        .iter()
        .filter(|d| ui.tool.as_ref().is_none_or(|t| *t == d.tool))
        .collect()
}

fn current<'a>(ui: &DigestUi, data: &'a DigestData) -> Option<&'a QuizCard> {
    data.due.iter().find(|c| !ui.graded.contains(&c.id))
}

impl DigestUi {
    /// Claims the keys this view owns; digits are only taken while quizzing.
    pub fn map_key(&self, key: KeyEvent) -> Option<Action> {
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return None;
        }
        match key.code {
            KeyCode::Char(c @ ('d' | 'r')) => Some(Action::Char(c)),
            KeyCode::Char(c @ ('f' | 'g')) if self.tab == Tab::Digest => Some(Action::Char(c)),
            KeyCode::Char(c @ '1'..='4') if self.tab == Tab::Quiz => Some(Action::Char(c)),
            _ => None,
        }
    }

    pub fn update(&mut self, action: Action, data: &DigestData) -> Option<Command> {
        match action {
            Action::Char('d') => self.tab = Tab::Digest,
            Action::Char('r') => self.tab = Tab::Quiz,
            Action::Up => self.cursor = self.cursor.saturating_sub(1),
            Action::Down => {
                self.cursor = (self.cursor + 1).min(visible(self, data).len().saturating_sub(1));
            }
            Action::Char('f') => {
                let mut tools: Vec<&str> = data.decisions.iter().map(|d| d.tool.as_str()).collect();
                tools.sort_unstable();
                tools.dedup();
                let next = match &self.tool {
                    None => 0,
                    Some(t) => tools.iter().position(|x| x == t).map_or(0, |i| i + 1),
                };
                self.tool = tools.get(next).map(|t| (*t).to_owned());
                self.cursor = 0;
            }
            Action::Char('g') => {
                let d = *visible(self, data).get(self.cursor)?;
                return (!d.folded).then(|| Command::FoldDecision { id: d.id.clone() });
            }
            Action::Char(c @ '1'..='4') if self.tab == Tab::Quiz => {
                let n = usize::from(c as u8 - b'1');
                let card = current(self, data)?;
                if self.picked.is_none() {
                    self.picked = Some(n);
                } else {
                    let id = card.id.clone();
                    self.graded.insert(id.clone());
                    self.picked = None;
                    return Some(Command::Grade {
                        card: id,
                        rating: Rating::ALL[n],
                    });
                }
            }
            _ => {}
        }
        None
    }
}

fn bold(s: &str) -> Span<'_> {
    Span::styled(s, Style::new().add_modifier(Modifier::BOLD))
}

pub fn render(f: &mut Frame, area: Rect, ui: &DigestUi, data: &DigestData) {
    let [tabs, body] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
    let tab = |t: Tab, label: &'static str| {
        let style = if ui.tab == t {
            Style::new()
                .fg(Color::Cyan)
                .add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        Span::styled(label, style)
    };
    let filter = ui.tool.as_deref().unwrap_or("all tools");
    f.render_widget(
        Paragraph::new(Line::from(vec![
            tab(Tab::Digest, " [d] Digest "),
            tab(Tab::Quiz, " [r] Quiz "),
            Span::styled(
                format!("  filter: {filter}  streak: {}", data.streak),
                Style::new().fg(Color::DarkGray),
            ),
        ])),
        tabs,
    );
    match ui.tab {
        Tab::Digest => render_digest(f, body, ui, data),
        Tab::Quiz => render_quiz(f, body, ui, data),
    }
}

fn render_digest(f: &mut Frame, area: Rect, ui: &DigestUi, data: &DigestData) {
    let [list, detail] =
        Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)]).areas(area);
    let rows = visible(ui, data);
    let mut lines = Vec::new();
    let mut project = "";
    for (i, d) in rows.iter().enumerate() {
        if d.project != project {
            project = &d.project;
            lines.push(Line::styled(
                project.to_owned(),
                Style::new().add_modifier(Modifier::BOLD),
            ));
        }
        let (mark, style) = if i == ui.cursor {
            ("> ", Style::new().fg(Color::Cyan))
        } else {
            ("  ", Style::new())
        };
        let fold = if d.folded { " ✓" } else { "" };
        lines.push(Line::styled(
            format!("{mark}[{}] {}{fold}", d.tool, d.what),
            style,
        ));
    }
    f.render_widget(
        Paragraph::new(lines).block(Block::new().borders(Borders::ALL).title("Decisions")),
        list,
    );
    let mut lines = Vec::new();
    if let Some(d) = rows.get(ui.cursor) {
        let basis = if d.stated { "stated" } else { "inferred" };
        lines.push(Line::from(vec![bold("What: "), Span::raw(d.what.clone())]));
        lines.push(Line::from(vec![
            bold("Why: "),
            Span::raw(format!("{} ({basis})", d.why)),
        ]));
        lines.push(bold("Alternatives rejected:").into());
        lines.extend(d.alternatives.iter().map(|a| Line::raw(format!("  - {a}"))));
        lines.push(Line::from(vec![bold("Risk: "), Span::raw(d.risk.clone())]));
        lines.push(Line::from(vec![
            bold("Commit: "),
            Span::raw(d.commit.clone()),
        ]));
        lines.push(Line::styled(
            "g fold into graph  f filter tool",
            Style::new().fg(Color::DarkGray),
        ));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(Block::new().borders(Borders::ALL).title("Decision")),
        detail,
    );
}

fn render_quiz(f: &mut Frame, area: Rect, ui: &DigestUi, data: &DigestData) {
    let block = Block::new().borders(Borders::ALL).title("Due Today");
    let Some(card) = current(ui, data) else {
        f.render_widget(Paragraph::new("No cards due.").block(block), area);
        return;
    };
    let left = data
        .due
        .iter()
        .filter(|c| !ui.graded.contains(&c.id))
        .count();
    let mut lines = vec![
        Line::styled(
            format!("{} · {} · {left} due", card.deck, card.kind),
            Style::new().fg(Color::DarkGray),
        ),
        Line::raw(card.question.clone()),
        Line::raw(""),
    ];
    for (i, opt) in card.options.iter().enumerate() {
        let style = match ui.picked {
            Some(_) if i == card.correct => Style::new().fg(Color::Green),
            Some(p) if i == p => Style::new().fg(Color::Red),
            _ => Style::new(),
        };
        lines.push(Line::styled(format!("[{}] {opt}", i + 1), style));
    }
    lines.push(Line::raw(""));
    if ui.picked.is_some() {
        lines.push(Line::from(vec![
            bold("Rationale: "),
            Span::raw(card.rationale.clone()),
        ]));
        lines.push(Line::from(vec![
            bold("Source: "),
            Span::raw(card.source.clone()),
        ]));
        lines.push(Line::raw(""));
        let bar: Vec<String> = Rating::ALL
            .iter()
            .enumerate()
            .map(|(i, r)| format!("[{}] {}", i + 1, r.label()))
            .collect();
        lines.push(Line::styled(bar.join("  "), Style::new().fg(Color::Yellow)));
    } else {
        lines.push(Line::styled(
            "1..4 pick an answer",
            Style::new().fg(Color::DarkGray),
        ));
    }
    f.render_widget(
        Paragraph::new(lines)
            .wrap(Wrap { trim: false })
            .block(block),
        area,
    );
}

#[cfg(test)]
pub(crate) fn sample_digest() -> DigestData {
    let dec = |id: &str, project: &str, tool: &str, what: &str| Decision {
        id: id.into(),
        project: project.into(),
        tool: tool.into(),
        what: what.into(),
        why: "Single writer keeps replay deterministic".into(),
        stated: id == "d1",
        alternatives: vec!["Per-device SQLite".into(), "Shared lock file".into()],
        risk: "Sync lag on a cold device".into(),
        commit: "a1b2c3d".into(),
        folded: false,
    };
    let card = |id: &str, kind: &str| QuizCard {
        id: id.into(),
        deck: "System Design".into(),
        kind: kind.into(),
        question: "Why replay reviews instead of storing only the latest card state?".into(),
        options: [
            "It is faster".into(),
            "State is a pure fold, so devices converge".into(),
            "SQLite forbids updates".into(),
            "Fuzz needs history".into(),
        ],
        correct: 1,
        rationale: "A pure fold over sorted reviews gives identical state on every device.".into(),
        source: "tracker/server/digest/fsrs.ts".into(),
    };
    DigestData {
        decisions: vec![
            dec(
                "d1",
                "learnpuppy",
                "claude",
                "Use jsonl sync files per device",
            ),
            dec(
                "d2",
                "learnpuppy",
                "codex",
                "Bounded channel between UI and worker",
            ),
            dec("d3", "tracker", "claude", "Disable FSRS fuzz"),
        ],
        due: vec![card("c1", "why"), card("c2", "trace")],
        streak: 7,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{run_worker, App, DataSource, Shared, View, ViewSnapshot};
    use arc_swap::ArcSwap;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use std::sync::{Arc, Mutex};
    use tokio::sync::mpsc;

    fn draw(ui: &DigestUi, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| render(f, f.area(), ui, &sample_digest()))
            .unwrap();
        t.backend().to_string()
    }

    fn press(ui: &mut DigestUi, keys: &str) -> Vec<Command> {
        let data = sample_digest();
        keys.chars()
            .filter_map(|c| ui.update(Action::Char(c), &data))
            .collect()
    }

    #[test]
    fn digest_80x24() {
        insta::assert_snapshot!(draw(&DigestUi::default(), 80, 24));
    }

    #[test]
    fn digest_filtered_140x40() {
        let mut ui = DigestUi::default();
        press(&mut ui, "ff");
        insta::assert_snapshot!(draw(&ui, 140, 40));
    }

    #[test]
    fn quiz_question_80x24() {
        let mut ui = DigestUi::default();
        press(&mut ui, "r");
        insta::assert_snapshot!(draw(&ui, 80, 24));
    }

    #[test]
    fn quiz_revealed_80x24() {
        let mut ui = DigestUi::default();
        press(&mut ui, "r1");
        insta::assert_snapshot!(draw(&ui, 80, 24));
    }

    #[test]
    fn filter_cycles_and_fold_emits_once_per_unfolded_decision() {
        let mut ui = DigestUi::default();
        press(&mut ui, "f");
        assert_eq!(ui.tool.as_deref(), Some("claude"));
        press(&mut ui, "ff");
        assert_eq!(ui.tool, None);
        assert_eq!(
            press(&mut ui, "g"),
            [Command::FoldDecision { id: "d1".into() }]
        );
    }

    #[test]
    fn digits_belong_to_quiz_only() {
        let mut app = App {
            view: View::Digest,
            ..App::default()
        };
        let key = |c| KeyEvent::new(KeyCode::Char(c), KeyModifiers::NONE);
        assert_eq!(app.map_key(key('2')), Some(Action::SwitchView(View::Graph)));
        app.digest.update(Action::Char('r'), &sample_digest());
        assert_eq!(app.map_key(key('2')), Some(Action::Char('2')));
    }

    struct Src(Mutex<DigestData>);
    impl DataSource for Src {
        fn snapshot(&self) -> ViewSnapshot {
            ViewSnapshot {
                digest: self.0.lock().unwrap().clone(),
                ..Default::default()
            }
        }
        fn apply(&self, cmd: Command) {
            if let Command::Grade { card, rating } = cmd {
                let mut d = self.0.lock().unwrap();
                d.due.retain(|c| c.id != card);
                d.streak += u32::from(rating != Rating::Again);
            }
        }
    }

    #[tokio::test]
    async fn review_grades_card_and_schedule_updates() {
        let src = Arc::new(Src(Mutex::new(sample_digest())));
        let shared: Shared = Arc::new(ArcSwap::from_pointee(src.snapshot()));
        let (tx, rx) = mpsc::channel(4);
        let mut ui = DigestUi::default();
        let cmds = press(&mut ui, "r23");
        assert_eq!(
            cmds,
            [Command::Grade {
                card: "c1".into(),
                rating: Rating::Good
            }]
        );
        for c in cmds {
            tx.send(c).await.unwrap();
        }
        drop(tx);
        run_worker(src, shared.clone(), rx).await;
        let snap = shared.load();
        assert_eq!(snap.digest.due.len(), 1);
        assert_eq!(snap.digest.streak, 8);
        assert_eq!(
            current(&ui, &snap.digest).map(|c| c.id.as_str()),
            Some("c2")
        );
    }
}
