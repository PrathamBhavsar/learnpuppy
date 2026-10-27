//! Stateless rendering of `App` + `ViewSnapshot`.

use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Clear, Paragraph};
use ratatui::Frame;

use crate::app::{palette_matches, App, Health, Modal, View, ViewSnapshot};
use crate::views::classroom;
use crate::views::graph;
use crate::views::quiz;

pub fn render(f: &mut Frame, app: &App, snap: &ViewSnapshot) {
    let [top, body, bottom] = Layout::vertical([
        Constraint::Length(1),
        Constraint::Min(0),
        Constraint::Length(1),
    ])
    .areas(f.area());
    f.render_widget(top_bar(app, snap), top);

    let mut cols = Vec::new();
    if app.sidebar {
        cols.push(Constraint::Length(22));
    }
    cols.push(Constraint::Min(10));
    let plan = app.plan_pane && app.view == View::Classroom;
    if plan {
        cols.push(Constraint::Length(28));
    }
    let areas = Layout::horizontal(cols).split(body);
    let mut areas = areas.iter().copied();
    if app.sidebar {
        let a = areas.next().unwrap_or_default();
        f.render_widget(pane("Sessions"), a);
    }
    let main = areas.next().unwrap_or_default();
    if app.view == View::Graph {
        graph::render(f, main, &app.graph, &snap.graph);
    } else if app.view == View::Classroom {
        classroom::render(f, main, &snap.classroom, &app.dock);
    } else {
        quiz::render(f, main, &app.digest, &snap.digest);
    }
    if plan {
        classroom::render_plan(f, areas.next().unwrap_or_default(), &snap.classroom);
    }

    f.render_widget(
        Paragraph::new(
            "Ctrl+K palette  Ctrl+A accounts  Ctrl+B sidebar  Ctrl+P plan  Tab view  q quit",
        )
        .style(Style::new().fg(Color::DarkGray)),
        bottom,
    );

    match &app.modal {
        Some(Modal::Palette { query, selected }) => palette(f, query, *selected),
        Some(Modal::Accounts { selected }) => accounts(f, snap, *selected),
        Some(Modal::Teach { topic }) => {
            let inner = popup(f, "Teach a topic", 50, 5);
            f.render_widget(Paragraph::new(format!("> {topic}_")), inner);
        }
        None => {}
    }
}

fn pane(title: &str) -> Block<'_> {
    Block::new().borders(Borders::ALL).title(title)
}

fn top_bar<'a>(app: &App, snap: &ViewSnapshot) -> Paragraph<'a> {
    let mut spans = vec![Span::styled(
        " LearnPuppy ",
        Style::new().add_modifier(Modifier::BOLD),
    )];
    for (i, v) in View::ALL.iter().enumerate() {
        let style = if *v == app.view {
            Style::new()
                .fg(Color::Cyan)
                .add_modifier(Modifier::REVERSED)
        } else {
            Style::new()
        };
        spans.push(Span::styled(format!(" {} {} ", i + 1, v.title()), style));
    }
    let healthy = snap
        .accounts
        .iter()
        .filter(|a| a.health == Health::Healthy)
        .count();
    spans.push(Span::raw(format!(
        "  accounts {healthy}/{}",
        snap.accounts.len()
    )));
    Paragraph::new(Line::from(spans))
}

fn health_label(h: &Health) -> String {
    match h {
        Health::Healthy => "Healthy".into(),
        Health::Cooldown { secs_left } => format!("Cooldown ({}m left)", secs_left.div_ceil(60)),
        Health::AuthExpired => "Auth expired".into(),
    }
}

fn popup(f: &mut Frame, title: &str, w: u16, h: u16) -> Rect {
    let area = f.area();
    let w = w.min(area.width);
    let h = h.min(area.height);
    let rect = Rect::new(
        area.x + (area.width - w) / 2,
        area.y + (area.height - h) / 2,
        w,
        h,
    );
    f.render_widget(Clear, rect);
    let block = pane(title);
    let inner = block.inner(rect);
    f.render_widget(block, rect);
    inner
}

fn palette(f: &mut Frame, query: &str, selected: usize) {
    let inner = popup(f, "Command palette", 50, 12);
    let mut lines = vec![Line::from(format!("> {query}_"))];
    for (i, (label, _)) in palette_matches(query).iter().enumerate() {
        let (mark, style) = if i == selected {
            ("> ", Style::new().fg(Color::Cyan))
        } else {
            ("  ", Style::new())
        };
        lines.push(Line::styled(format!("{mark}{label}"), style));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

fn accounts(f: &mut Frame, snap: &ViewSnapshot, selected: usize) {
    let inner = popup(f, "Accounts and model slots", 76, 20);
    let mut lines = vec![Line::styled(
        "Accounts",
        Style::new().add_modifier(Modifier::BOLD),
    )];
    for a in &snap.accounts {
        let color = match a.health {
            Health::Healthy => Color::Green,
            Health::Cooldown { .. } => Color::Yellow,
            Health::AuthExpired => Color::Red,
        };
        lines.push(Line::from(vec![
            Span::styled("● ", Style::new().fg(color)),
            Span::raw(format!(
                "{:<24} {:<20} {} turns {} tok",
                a.id,
                health_label(&a.health),
                a.turns,
                a.tokens
            )),
        ]));
    }
    lines.push(Line::raw(""));
    lines.push(Line::styled(
        "Slots (Enter cycles pin)",
        Style::new().add_modifier(Modifier::BOLD),
    ));
    for (i, s) in snap.slots.iter().enumerate() {
        let (mark, style) = if i == selected {
            ("> ", Style::new().fg(Color::Cyan))
        } else {
            ("  ", Style::new())
        };
        lines.push(Line::styled(
            format!("{mark}{:<12} {}", s.role, s.pin),
            style,
        ));
    }
    f.render_widget(Paragraph::new(lines), inner);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{sample_snapshot, Action};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn draw(app: &App, w: u16, h: u16) -> String {
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| render(f, app, &sample_snapshot())).unwrap();
        t.backend().to_string()
    }

    fn with_modal(action: Action) -> App {
        let mut app = App::default();
        app.update(action, &sample_snapshot());
        app
    }

    #[test]
    fn shell_80x24() {
        insta::assert_snapshot!(draw(&App::default(), 80, 24));
    }

    #[test]
    fn shell_140x40() {
        insta::assert_snapshot!(draw(&App::default(), 140, 40));
    }

    #[test]
    fn palette_80x24() {
        insta::assert_snapshot!(draw(&with_modal(Action::OpenPalette), 80, 24));
    }

    #[test]
    fn accounts_80x24() {
        insta::assert_snapshot!(draw(&with_modal(Action::OpenAccounts), 80, 24));
    }

    #[test]
    fn accounts_140x40() {
        insta::assert_snapshot!(draw(&with_modal(Action::OpenAccounts), 140, 40));
    }
}
