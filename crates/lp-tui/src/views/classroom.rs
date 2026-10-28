//! Classroom view: transcript, question dock and plan DAG pane.

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lp_core::plan::{shuffle_options, Phase, PlanStatuses, PlanTree, QuizInput, QuizOption};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Padding, Paragraph};
use ratatui::Frame;

use crate::app::{Action, Command};

/// Value sent as the answer when the learner picks "I don't know".
pub const IDK: &str = "__idk__";
const MAX_RUNG: u8 = 5;

/// What the teacher is waiting on. `Quiz` covers `probe` and `quiz`; `Free` covers `explain`,
/// `recall` and `problem`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Prompt {
    Quiz { call_id: String, input: QuizInput },
    Free { label: String, question: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Classroom {
    pub phase: Phase,
    pub practice: bool,
    /// One markdown string per message.
    pub transcript: Vec<String>,
    pub plan: Option<PlanTree>,
    pub statuses: PlanStatuses,
    pub prompt: Option<Prompt>,
}

impl Default for Classroom {
    fn default() -> Self {
        Self {
            phase: Phase::Probe,
            practice: false,
            transcript: Vec::new(),
            plan: None,
            statuses: PlanStatuses::default(),
            prompt: None,
        }
    }
}

impl Classroom {
    fn badge(&self) -> &'static str {
        match (self.practice, self.phase) {
            (true, _) => "PRACTICE",
            (_, Phase::Probe) => "PROBE",
            (_, Phase::Plan) => "PLAN",
            (_, Phase::Teach) => "TEACH",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Answer {
    /// Picked option value, `IDK`, or empty for free-form prompts.
    pub value: String,
    pub confidence: u8,
    pub reading_thinking: String,
    pub thinking: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DockKey {
    Tab,
    Hint,
    Submit,
    Leave,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Kind {
    #[default]
    None,
    Quiz(usize),
    Free,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum Tab {
    #[default]
    Reading,
    Answer,
}

/// Local input state for the active prompt; reset whenever the prompt changes.
#[derive(Debug)]
pub struct Dock {
    key: String,
    kind: Kind,
    /// Index into the shuffled options; `len` means "I don't know".
    choice: Option<usize>,
    confidence: u8,
    tab: Tab,
    notes: bool,
    reading: String,
    thinking: String,
    rung: u8,
    sent: bool,
}

impl Default for Dock {
    fn default() -> Self {
        Self {
            key: String::new(),
            kind: Kind::None,
            choice: None,
            confidence: 3,
            tab: Tab::Reading,
            notes: false,
            reading: String::new(),
            thinking: String::new(),
            rung: 0,
            sent: false,
        }
    }
}

impl Dock {
    pub fn sync(&mut self, cl: &Classroom) {
        let (kind, key) = match &cl.prompt {
            None => (Kind::None, ""),
            Some(Prompt::Quiz { call_id, input }) => {
                (Kind::Quiz(input.options.len()), call_id.as_str())
            }
            Some(Prompt::Free { question, .. }) => (Kind::Free, question.as_str()),
        };
        if self.key != key {
            *self = Self {
                key: key.to_owned(),
                ..Self::default()
            };
        }
        self.kind = kind;
    }

    fn typing(&self) -> bool {
        self.kind == Kind::Free || self.notes
    }

    /// Dock-specific bindings; `None` falls through to the global keymap.
    pub fn map_key(&self, key: KeyEvent) -> Option<Action> {
        if self.kind == Kind::None {
            return None;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            return match key.code {
                KeyCode::Char('t') => Some(Action::Dock(DockKey::Tab)),
                KeyCode::Char('h') => Some(Action::Dock(DockKey::Hint)),
                KeyCode::Char('s') => Some(Action::Dock(DockKey::Submit)),
                _ => None,
            };
        }
        match key.code {
            KeyCode::Enter => Some(Action::Enter),
            KeyCode::Backspace => Some(Action::Backspace),
            KeyCode::Esc if self.notes => Some(Action::Dock(DockKey::Leave)),
            KeyCode::Char(c) if self.typing() => Some(Action::Char(c)),
            KeyCode::Char(c) if self.kind != Kind::Free => match c.to_ascii_lowercase() {
                'a'..='d' | 'i' | 't' | '1'..='5' => Some(Action::Char(c)),
                _ => None,
            },
            _ => None,
        }
    }

    pub fn update(&mut self, action: Action, cl: &Classroom) -> Option<Command> {
        match (action, self.kind) {
            (_, Kind::None) => {}
            (Action::Dock(DockKey::Tab), Kind::Quiz(_)) => {
                self.tab = if self.tab == Tab::Reading {
                    Tab::Answer
                } else {
                    Tab::Reading
                };
            }
            (Action::Dock(DockKey::Hint), _) => self.rung = (self.rung + 1).min(MAX_RUNG),
            (Action::Dock(DockKey::Leave), _) => self.notes = false,
            (Action::Dock(DockKey::Submit), Kind::Free) | (Action::Enter, Kind::Quiz(_)) => {
                return self.submit(cl);
            }
            (Action::Char(c), _) if self.typing() => self.text().push(c),
            (Action::Backspace, _) if self.typing() => {
                self.text().pop();
            }
            (Action::Enter, Kind::Free) => self.thinking.push('\n'),
            (Action::Char(c), Kind::Quiz(n)) => match c.to_ascii_lowercase() {
                l @ 'a'..='d' if usize::from(l as u8 - b'a') < n => {
                    self.choice = Some(usize::from(l as u8 - b'a'));
                }
                'i' => self.choice = Some(n),
                't' => self.notes = true,
                d @ '1'..='5' => self.confidence = d as u8 - b'0',
                _ => {}
            },
            _ => {}
        }
        None
    }

    fn text(&mut self) -> &mut String {
        if self.kind != Kind::Free && self.tab == Tab::Reading {
            &mut self.reading
        } else {
            &mut self.thinking
        }
    }

    fn submit(&mut self, cl: &Classroom) -> Option<Command> {
        if self.sent {
            return None;
        }
        let value = match &cl.prompt {
            Some(Prompt::Quiz { call_id, input }) => {
                let i = self.choice?;
                shuffle_options(&input.options, call_id)
                    .get(i)
                    .map_or(IDK.to_owned(), |o| o.value.clone())
            }
            _ => String::new(),
        };
        self.sent = true;
        Some(Command::Answer(Answer {
            value,
            confidence: self.confidence,
            reading_thinking: self.reading.clone(),
            thinking: self.thinking.clone(),
        }))
    }
}

// ---------------------------------------------------------------- rendering

fn wrap(s: &str, width: usize) -> Vec<String> {
    let chars: Vec<char> = s.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars
        .chunks(width.max(1))
        .map(|c| c.iter().collect())
        .collect()
}

fn transcript_lines(msgs: &[String], width: usize) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    for msg in msgs {
        out.extend(crate::markdown::render(msg, width));
        out.push(Line::raw(""));
    }
    out
}

fn tail<T: Clone>(v: &[T], n: usize) -> Vec<T> {
    v[v.len().saturating_sub(n)..].to_vec()
}

pub fn render(f: &mut Frame, area: Rect, cl: &Classroom, dock: &Dock) {
    let block = Block::new()
        .borders(Borders::ALL)
        .padding(Padding::horizontal(1))
        .title(format!("Classroom [{}]", cl.badge()));
    let inner = block.inner(area);
    f.render_widget(block, area);
    let lines = dock_lines(cl, dock, usize::from(inner.width.saturating_sub(4)));
    let dock_h = if lines.is_empty() {
        0
    } else {
        (lines.len() as u16 + 2).min(inner.height * 2 / 3)
    };
    let [text, bottom] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(dock_h)]).areas(inner);
    let t = transcript_lines(&cl.transcript, usize::from(text.width));
    f.render_widget(Paragraph::new(tail(&t, usize::from(text.height))), text);
    if dock_h > 0 {
        let b = Block::new()
            .borders(Borders::ALL)
            .padding(Padding::horizontal(1))
            .title("Question");
        let body = b.inner(bottom);
        f.render_widget(b, bottom);
        f.render_widget(Paragraph::new(tail(&lines, usize::from(body.height))), body);
    }
}

fn dock_lines(cl: &Classroom, dock: &Dock, width: usize) -> Vec<Line<'static>> {
    let dim = Style::new().fg(Color::DarkGray);
    let sel = Style::new().fg(Color::Cyan);
    let mut out = Vec::new();
    match &cl.prompt {
        None => {}
        Some(Prompt::Quiz { call_id, input }) => {
            out.extend(crate::markdown::render(&input.question, width));
            let opts: Vec<QuizOption> = shuffle_options(&input.options, call_id);
            let n = opts.len();
            let rows = opts
                .iter()
                .enumerate()
                .map(|(i, o)| (i, format!("[{}] {}", char::from(b'A' + i as u8), o.label)))
                .chain([(n, "[I] I don't know".to_owned())]);
            for (i, text) in rows {
                let (mark, style) = if dock.choice == Some(i) {
                    ("> ", sel)
                } else {
                    ("  ", Style::new())
                };
                out.push(Line::styled(format!("{mark}{text}"), style));
            }
            let mut conf = vec![Span::raw("Confidence ")];
            for c in 1..=5u8 {
                let style = if c == dock.confidence { sel } else { dim };
                conf.push(Span::styled(format!(" {c} "), style));
            }
            conf.push(Span::styled(" 1=Guess .. 5=Certain", dim));
            out.push(Line::from(conf));
            let tab = |t, label: &str| {
                let style = if dock.tab == t { sel } else { dim };
                Span::styled(format!(" {label} "), style)
            };
            out.push(Line::from(vec![
                tab(Tab::Reading, "Reading Notes"),
                tab(Tab::Answer, "Answer Reasoning"),
                Span::styled(" t type  Ctrl+T tab  Esc back  Enter submit", dim),
            ]));
            let text = if dock.tab == Tab::Reading {
                &dock.reading
            } else {
                &dock.thinking
            };
            out.push(Line::from(format!(
                "{text}{}",
                if dock.notes { "_" } else { "" }
            )));
        }
        Some(Prompt::Free { label, question }) => {
            out.push(Line::styled(
                format!("{label}:"),
                Style::new().add_modifier(Modifier::BOLD),
            ));
            out.extend(crate::markdown::render(question, width));
            out.push(Line::raw(""));
            for l in dock.thinking.split('\n') {
                out.extend(wrap(l, width).into_iter().map(Line::from));
            }
            if let Some(last) = out.last_mut() {
                last.spans.push(Span::raw("_"));
            }
            let hint = format!("Ctrl+H hint (R{})  Ctrl+S submit  Enter newline", dock.rung);
            out.extend(wrap(&hint, width).into_iter().map(|l| Line::styled(l, dim)));
        }
    }
    out
}

pub fn render_plan(f: &mut Frame, area: Rect, cl: &Classroom) {
    let block = Block::new().borders(Borders::ALL).title("Plan");
    let inner = block.inner(area);
    f.render_widget(block, area);
    let Some(tree) = &cl.plan else {
        f.render_widget(
            Paragraph::new(Line::styled(
                "no plan yet",
                Style::new().fg(Color::DarkGray),
            )),
            inner,
        );
        return;
    };
    let mut lines = Vec::new();
    for n in &tree.nodes {
        let (badge, style) = if cl.statuses.current.as_ref() == Some(&n.id) {
            (
                "●",
                Style::new().fg(Color::Cyan).add_modifier(Modifier::BOLD),
            )
        } else if cl.statuses.visited.contains(&n.id) {
            ("✓", Style::new().fg(Color::Green))
        } else {
            ("○", Style::new().fg(Color::DarkGray))
        };
        lines.push(Line::styled(format!("{badge} {}", n.title), style));
        let after: Vec<&str> = tree
            .edges
            .iter()
            .filter(|(_, to)| *to == n.id)
            .map(|(from, _)| from.as_str())
            .collect();
        if !after.is_empty() {
            lines.push(Line::styled(
                format!("  after {}", after.join(", ")),
                Style::new().fg(Color::DarkGray),
            ));
        }
    }
    f.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{App, ViewSnapshot};
    use lp_core::plan::{parse_plan, CorrectAnswer};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn quiz() -> Prompt {
        let opt = |l: &str, v: &str| QuizOption {
            label: l.into(),
            value: v.into(),
        };
        Prompt::Quiz {
            call_id: "call-1".into(),
            input: QuizInput {
                question: "Which structure gives O(1) average lookup?".into(),
                options: vec![
                    opt("Linked list", "list"),
                    opt("Hash map", "hash"),
                    opt("Binary heap", "heap"),
                    opt("Sorted array", "array"),
                ],
                correct_answer: CorrectAnswer::One("hash".into()),
                explanation: "Hashing".into(),
                multi_select: false,
                kind: None,
                node_id: None,
            },
        }
    }

    fn teach(prompt: Option<Prompt>, practice: bool) -> Classroom {
        let plan = parse_plan(
            "graph TD\n a[\"Arrays\"]\n b[\"Hash maps\"]\n c[\"Tries\"]\n a --> b --> c\n",
            1,
        )
        .unwrap();
        Classroom {
            phase: Phase::Teach,
            practice,
            transcript: vec![
                "# Hash maps\nA map sends key \\to bucket, so $f \\leq n$.\n```rust\nlet m = HashMap::new();\n```"
                    .into(),
            ],
            statuses: PlanStatuses {
                current: Some("b".into()),
                visited: vec!["a".into()],
                upcoming: vec!["c".into()],
            },
            plan: Some(plan),
            prompt,
        }
    }

    fn draw(cl: Classroom, w: u16, h: u16) -> String {
        let mut app = App::default();
        app.dock.sync(&cl);
        let snap = ViewSnapshot {
            classroom: cl,
            ..Default::default()
        };
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| crate::ui::render(f, &app, &snap)).unwrap();
        t.backend().to_string()
    }

    #[test]
    fn probe_80x24() {
        let cl = Classroom {
            transcript: vec!["Let's see what you already know.".into()],
            prompt: Some(quiz()),
            ..Classroom::default()
        };
        insta::assert_snapshot!(draw(cl, 80, 24));
    }

    #[test]
    fn plan_80x24() {
        let cl = Classroom {
            phase: Phase::Plan,
            transcript: vec!["Drafting your plan...".into()],
            ..Classroom::default()
        };
        insta::assert_snapshot!(draw(cl, 80, 24));
    }

    #[test]
    fn teach_140x40() {
        insta::assert_snapshot!(draw(teach(Some(quiz()), false), 140, 40));
    }

    #[test]
    fn practice_80x24() {
        let free = Prompt::Free {
            label: "problem".into(),
            question: "Insert 3 keys into an empty table.".into(),
        };
        insta::assert_snapshot!(draw(teach(Some(free), true), 80, 24));
    }

    fn press(
        dock: &mut Dock,
        cl: &Classroom,
        code: KeyCode,
        mods: KeyModifiers,
    ) -> Option<Command> {
        let action = dock.map_key(KeyEvent::new(code, mods))?;
        dock.update(action, cl)
    }

    fn typed(dock: &mut Dock, cl: &Classroom, s: &str) {
        for c in s.chars() {
            press(dock, cl, KeyCode::Char(c), KeyModifiers::NONE);
        }
    }

    #[test]
    fn quiz_keys_pick_option_confidence_notes_and_submit_once() {
        let cl = teach(Some(quiz()), false);
        let mut d = Dock::default();
        d.sync(&cl);
        let none = KeyModifiers::NONE;
        assert_eq!(
            press(&mut d, &cl, KeyCode::Enter, none),
            None,
            "needs a choice"
        );
        typed(&mut d, &cl, "d4");
        typed(&mut d, &cl, "t");
        typed(&mut d, &cl, "why");
        press(&mut d, &cl, KeyCode::Char('t'), KeyModifiers::CONTROL);
        typed(&mut d, &cl, "hash");
        press(&mut d, &cl, KeyCode::Esc, none);
        let expect = shuffle_options(
            match &cl.prompt {
                Some(Prompt::Quiz { input, .. }) => &input.options,
                _ => unreachable!(),
            },
            "call-1",
        )[3]
        .value
        .clone();
        assert_eq!(
            press(&mut d, &cl, KeyCode::Enter, none),
            Some(Command::Answer(Answer {
                value: expect,
                confidence: 4,
                reading_thinking: "why".into(),
                thinking: "hash".into(),
            }))
        );
        assert_eq!(
            press(&mut d, &cl, KeyCode::Enter, none),
            None,
            "already sent"
        );
    }

    #[test]
    fn idk_and_free_prompt_hint_ladder_caps_at_r5() {
        let mut d = Dock::default();
        let cl = teach(Some(quiz()), false);
        d.sync(&cl);
        typed(&mut d, &cl, "i");
        let Some(Command::Answer(a)) = press(&mut d, &cl, KeyCode::Enter, KeyModifiers::NONE)
        else {
            panic!("expected answer")
        };
        assert_eq!((a.value.as_str(), a.confidence), (IDK, 3));

        let free = teach(
            Some(Prompt::Free {
                label: "explain".into(),
                question: "q".into(),
            }),
            false,
        );
        d.sync(&free);
        for _ in 0..9 {
            press(&mut d, &free, KeyCode::Char('h'), KeyModifiers::CONTROL);
        }
        typed(&mut d, &free, "ab");
        press(&mut d, &free, KeyCode::Enter, KeyModifiers::NONE);
        typed(&mut d, &free, "c");
        assert_eq!((d.rung, d.thinking.as_str()), (5, "ab\nc"));
        let sent = press(&mut d, &free, KeyCode::Char('s'), KeyModifiers::CONTROL);
        assert!(matches!(sent, Some(Command::Answer(a)) if a.value.is_empty()));
    }
}
