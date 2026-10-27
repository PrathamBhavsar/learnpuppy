//! Knowledge Graph view: Braille force-directed canvas plus a 3-level collapsible chain table
//! (`Cluster -> A ∧ B ⟹ C -> Statements`) with multi-select practice and cascade-delete preview.

use std::collections::{BTreeMap, BTreeSet};

use lp_core::graph::{GraphChain, GraphSnapshot, NodeId};
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::symbols::Marker;
use ratatui::text::Line;
use ratatui::widgets::canvas::{Canvas, Line as Edge, Points, Rectangle};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Frame;

use crate::app::{Action, Command};

const LAYOUT_ITERATIONS: usize = 200;
/// Plain keys the graph view reacts to (besides arrows, Enter and Esc).
pub const KEYS: &str = "hjkl+-nNpdy ";

/// Graph plus its settled layout; built once per snapshot so rendering never re-simulates.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GraphData {
    pub snap: GraphSnapshot,
    pub layout: BTreeMap<NodeId, [f64; 2]>,
}

impl GraphData {
    pub fn new(snap: GraphSnapshot) -> Self {
        let layout = snap.layout(LAYOUT_ITERATIONS);
        Self { snap, layout }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum Key {
    Cluster(String),
    Chain(String),
    Node(NodeId),
}

struct Group {
    label: String,
    nodes: Vec<NodeId>,
    chains: Vec<usize>,
}

struct Row {
    key: Key,
    depth: usize,
    text: String,
    nodes: Vec<NodeId>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct GraphUi {
    pub pan: [f64; 2],
    pub zoom: f64,
    pub cursor: usize,
    pub focus: usize,
    pub selected: BTreeSet<NodeId>,
    pub confirming_delete: bool,
    expanded: BTreeSet<Key>,
}

impl Default for GraphUi {
    fn default() -> Self {
        Self {
            pan: [0.0; 2],
            zoom: 1.0,
            cursor: 0,
            focus: 0,
            selected: BTreeSet::new(),
            confirming_delete: false,
            expanded: BTreeSet::new(),
        }
    }
}

fn groups(snap: &GraphSnapshot) -> Vec<Group> {
    let clusters = snap.birth_session_partition();
    let mut of_node: BTreeMap<&NodeId, usize> = BTreeMap::new();
    for (i, c) in clusters.iter().enumerate() {
        of_node.extend(c.nodes.iter().map(|n| (n, i)));
    }
    let mut out: Vec<Group> = clusters
        .iter()
        .map(|c| Group {
            label: c.chat_id.to_string(),
            nodes: c.nodes.clone(),
            chains: vec![],
        })
        .collect();
    let mut loose = Group {
        label: "ungrouped".into(),
        nodes: snap
            .nodes
            .keys()
            .filter(|n| !of_node.contains_key(n))
            .cloned()
            .collect(),
        chains: vec![],
    };
    for (i, c) in snap.chains.iter().enumerate() {
        match of_node.get(&c.conclusion) {
            Some(&g) => out[g].chains.push(i),
            None => loose.chains.push(i),
        }
    }
    if !loose.nodes.is_empty() || !loose.chains.is_empty() {
        out.push(loose);
    }
    out
}

fn name<'a>(snap: &'a GraphSnapshot, id: &'a NodeId) -> &'a str {
    snap.nodes.get(id).map_or(id.as_str(), |n| n.name.as_str())
}

fn chain_label(snap: &GraphSnapshot, c: &GraphChain) -> String {
    let premises: Vec<&str> = c.premises.iter().map(|p| name(snap, p)).collect();
    format!("{} ⟹ {}", premises.join(" ∧ "), name(snap, &c.conclusion))
}

impl GraphUi {
    fn rows(&self, snap: &GraphSnapshot) -> Vec<Row> {
        let mut rows = Vec::new();
        for g in groups(snap) {
            let key = Key::Cluster(g.label.clone());
            let open = self.expanded.contains(&key);
            rows.push(Row {
                text: format!(
                    "{} {} ({} nodes, {} chains)",
                    if open { '▾' } else { '▸' },
                    g.label,
                    g.nodes.len(),
                    g.chains.len()
                ),
                key,
                depth: 0,
                nodes: g.nodes,
            });
            if !open {
                continue;
            }
            for &i in &g.chains {
                let c = &snap.chains[i];
                let key = Key::Chain(c.id.to_string());
                let open = self.expanded.contains(&key);
                let nodes: Vec<NodeId> = c
                    .premises
                    .iter()
                    .chain(std::iter::once(&c.conclusion))
                    .cloned()
                    .collect();
                rows.push(Row {
                    text: format!("{} {}", if open { '▾' } else { '▸' }, chain_label(snap, c)),
                    key,
                    depth: 1,
                    nodes: nodes.clone(),
                });
                if open {
                    rows.extend(nodes.into_iter().map(|n| {
                        Row {
                            text: snap
                                .nodes
                                .get(&n)
                                .map_or(n.to_string(), |x| x.statement.clone()),
                            key: Key::Node(n.clone()),
                            depth: 2,
                            nodes: vec![n],
                        }
                    }));
                }
            }
        }
        rows
    }

    /// Applies a graph-view action; returns a worker command for practice or confirmed delete.
    pub fn update(&mut self, action: Action, data: &GraphData) -> Option<Command> {
        let rows = self.rows(&data.snap);
        let last = rows.len().saturating_sub(1);
        let step = |v: f64| v / self.zoom * 0.1;
        let span = extent(&data.layout);
        match action {
            Action::CloseModal => self.confirming_delete = false,
            Action::Up => self.cursor = self.cursor.saturating_sub(1),
            Action::Down => self.cursor = (self.cursor + 1).min(last),
            Action::Enter => {
                if let Some(row) = rows.get(self.cursor) {
                    if matches!(row.key, Key::Node(_)) {
                        return None;
                    }
                    if !self.expanded.remove(&row.key) {
                        self.expanded.insert(row.key.clone());
                    }
                }
            }
            Action::Char(' ') => {
                if let Some(row) = rows.get(self.cursor) {
                    if row.nodes.iter().all(|n| self.selected.contains(n)) {
                        row.nodes.iter().for_each(|n| {
                            self.selected.remove(n);
                        });
                    } else {
                        self.selected.extend(row.nodes.iter().cloned());
                    }
                }
            }
            Action::Char('h') => self.pan[0] -= step(span[0]),
            Action::Char('l') => self.pan[0] += step(span[0]),
            Action::Char('j') => self.pan[1] -= step(span[1]),
            Action::Char('k') => self.pan[1] += step(span[1]),
            Action::Char('+') => self.zoom = (self.zoom * 1.25).min(16.0),
            Action::Char('-') => self.zoom = (self.zoom / 1.25).max(0.25),
            Action::Char('n') | Action::Char('N') => {
                let n = data.snap.nodes.len().max(1);
                self.focus = if action == Action::Char('n') {
                    (self.focus + 1) % n
                } else {
                    (self.focus + n - 1) % n
                };
            }
            Action::Char('p') => {
                let sel: Vec<NodeId> = self.selected.iter().cloned().collect();
                let chains: Vec<String> = data
                    .snap
                    .chains_for_selection(&sel)
                    .iter()
                    .map(|c| c.id.to_string())
                    .collect();
                return (!chains.is_empty()).then_some(Command::Practice { chains });
            }
            Action::Char('d') => self.confirming_delete = !self.selected.is_empty(),
            Action::Char('y') if self.confirming_delete => {
                self.confirming_delete = false;
                let nodes = std::mem::take(&mut self.selected)
                    .iter()
                    .map(|n| n.to_string())
                    .collect();
                return Some(Command::DeleteNodes { nodes });
            }
            _ => {}
        }
        self.cursor = self.cursor.min(rows.len().saturating_sub(1));
        None
    }
}

fn extent(layout: &BTreeMap<NodeId, [f64; 2]>) -> [f64; 2] {
    let (lo, hi) = bounds(layout);
    [(hi[0] - lo[0]).max(1.0), (hi[1] - lo[1]).max(1.0)]
}

fn bounds(layout: &BTreeMap<NodeId, [f64; 2]>) -> ([f64; 2], [f64; 2]) {
    let mut lo = [f64::MAX; 2];
    let mut hi = [f64::MIN; 2];
    for p in layout.values() {
        for i in 0..2 {
            lo[i] = lo[i].min(p[i]);
            hi[i] = hi[i].max(p[i]);
        }
    }
    if layout.is_empty() {
        return ([0.0; 2], [1.0; 2]);
    }
    (lo, hi)
}

pub fn render(f: &mut Frame, area: Rect, ui: &GraphUi, data: &GraphData) {
    let [left, right] =
        Layout::horizontal([Constraint::Percentage(55), Constraint::Percentage(45)]).areas(area);
    let [canvas, drawer] =
        Layout::vertical([Constraint::Min(5), Constraint::Length(6)]).areas(left);
    render_canvas(f, canvas, ui, data);
    render_drawer(f, drawer, ui, data);
    render_table(f, right, ui, data);
}

fn render_canvas(f: &mut Frame, area: Rect, ui: &GraphUi, data: &GraphData) {
    let (lo, hi) = bounds(&data.layout);
    let ext = extent(&data.layout);
    let view = |i: usize| {
        let half = ext[i] * 0.55 / ui.zoom;
        let c = (lo[i] + hi[i]) / 2.0 + ui.pan[i];
        [c - half, c + half]
    };
    let focus = data.snap.nodes.keys().nth(ui.focus);
    let pos = |n: &NodeId| data.layout.get(n).copied();
    let widget = Canvas::default()
        .block(Block::new().borders(Borders::ALL).title("Graph"))
        .marker(Marker::Braille)
        .x_bounds(view(0))
        .y_bounds(view(1))
        .paint(|ctx| {
            for g in groups(&data.snap) {
                let pts: Vec<[f64; 2]> = g.nodes.iter().filter_map(pos).collect();
                if g.label != "ungrouped" && !pts.is_empty() {
                    let (x0, x1) = pts
                        .iter()
                        .fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p[0]), a.1.max(p[0])));
                    let (y0, y1) = pts
                        .iter()
                        .fold((f64::MAX, f64::MIN), |a, p| (a.0.min(p[1]), a.1.max(p[1])));
                    let pad = ext[0] * 0.03;
                    ctx.draw(&Rectangle {
                        x: x0 - pad,
                        y: y0 - pad,
                        width: x1 - x0 + 2.0 * pad,
                        height: y1 - y0 + 2.0 * pad,
                        color: Color::DarkGray,
                    });
                }
            }
            for c in &data.snap.chains {
                if let Some(to) = pos(&c.conclusion) {
                    for from in c.premises.iter().filter_map(pos) {
                        ctx.draw(&Edge {
                            x1: from[0],
                            y1: from[1],
                            x2: to[0],
                            y2: to[1],
                            color: Color::Blue,
                        });
                    }
                }
            }
            ctx.layer();
            for (id, p) in &data.layout {
                let color = if Some(id) == focus {
                    Color::Yellow
                } else if ui.selected.contains(id) {
                    Color::Green
                } else {
                    Color::White
                };
                ctx.draw(&Points {
                    coords: &[(p[0], p[1])],
                    color,
                });
            }
            if let Some(p) = focus.and_then(pos) {
                let label = name(&data.snap, focus.unwrap_or(&NodeId::default())).to_string();
                ctx.print(
                    p[0],
                    p[1],
                    Line::styled(label, Style::new().fg(Color::Yellow)),
                );
            }
        });
    f.render_widget(widget, area);
}

fn render_drawer(f: &mut Frame, area: Rect, ui: &GraphUi, data: &GraphData) {
    let mut lines = Vec::new();
    if ui.confirming_delete {
        let sel: Vec<NodeId> = ui.selected.iter().cloned().collect();
        let dead = data.snap.deletion_blast_radius(&sel);
        lines.push(Line::styled(
            format!(
                "Delete {} nodes? cascades to {} chains",
                sel.len(),
                dead.len()
            ),
            Style::new().fg(Color::Red),
        ));
        lines.extend(
            dead.iter()
                .take(3)
                .map(|id| Line::raw(format!("  - chain {id}"))),
        );
        lines.push(Line::raw("y confirm  Esc cancel"));
    } else if let Some(n) = data.snap.nodes.values().nth(ui.focus) {
        lines.push(Line::styled(
            n.name.clone(),
            Style::new().add_modifier(Modifier::BOLD),
        ));
        lines.push(Line::raw(n.statement.clone()));
        lines.extend(n.provenance.iter().take(3).map(|p| {
            Line::styled(
                format!("from {} / {}", p.chat_id, p.message_id),
                Style::new().fg(Color::DarkGray),
            )
        }));
    }
    let title = "Provenance  hjkl pan +/- zoom n/N node";
    f.render_widget(
        Paragraph::new(lines).block(Block::new().borders(Borders::ALL).title(title)),
        area,
    );
}

fn render_table(f: &mut Frame, area: Rect, ui: &GraphUi, data: &GraphData) {
    let lines: Vec<Line> = ui
        .rows(&data.snap)
        .iter()
        .enumerate()
        .map(|(i, r)| {
            let mark = if r.nodes.iter().all(|n| ui.selected.contains(n)) {
                "[x]"
            } else {
                "[ ]"
            };
            let style = if i == ui.cursor {
                Style::new()
                    .fg(Color::Cyan)
                    .add_modifier(Modifier::REVERSED)
            } else {
                Style::new()
            };
            Line::styled(format!("{}{mark} {}", "  ".repeat(r.depth), r.text), style)
        })
        .collect();
    let title = "Chains  Enter fold  Space select  p practice  d delete";
    f.render_widget(
        Paragraph::new(lines).block(Block::new().borders(Borders::ALL).title(title)),
        area,
    );
}

#[cfg(test)]
pub(crate) fn sample_graph() -> GraphData {
    use chrono::DateTime;
    use lp_core::graph::{GraphNode, Provenance};
    let node = |id: &str, chat: &str| GraphNode {
        id: id.into(),
        name: id.to_uppercase(),
        statement: format!("{id} holds"),
        aliases: vec![],
        created_at: DateTime::UNIX_EPOCH,
        provenance: vec![Provenance {
            chat_id: chat.into(),
            message_id: "m1".into(),
        }],
    };
    let chain = |id: &str, premises: &[&str], conclusion: &str| GraphChain {
        id: id.into(),
        premises: premises.iter().map(|p| (*p).into()).collect(),
        conclusion: conclusion.into(),
        provenance: vec![],
        created_at: DateTime::UNIX_EPOCH,
    };
    let nodes = [
        node("a", "chat1"),
        node("b", "chat1"),
        node("c", "chat1"),
        node("d", "chat2"),
        node("e", "chat2"),
    ];
    let chains = [chain("k1", &["a", "b"], "c"), chain("k2", &["c"], "d")];
    GraphData::new(GraphSnapshot::from_ledgers(&nodes, &chains))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    fn draw(ui: &GraphUi, w: u16, h: u16) -> String {
        let data = sample_graph();
        let mut t = Terminal::new(TestBackend::new(w, h)).unwrap();
        t.draw(|f| render(f, f.area(), ui, &data)).unwrap();
        t.backend().to_string()
    }

    fn press(ui: &mut GraphUi, keys: &str) -> Option<Command> {
        let data = sample_graph();
        let mut out = None;
        for c in keys.chars() {
            out = ui.update(Action::Char(c), &data);
        }
        out
    }

    #[test]
    fn graph_80x24() {
        insta::assert_snapshot!(draw(&GraphUi::default(), 80, 24));
    }

    #[test]
    fn graph_140x40_expanded_selected() {
        let data = sample_graph();
        let mut ui = GraphUi::default();
        ui.update(Action::Enter, &data); // open chat1
        ui.update(Action::Down, &data);
        ui.update(Action::Enter, &data); // open k1
        press(&mut ui, " ");
        insta::assert_snapshot!(draw(&ui, 140, 40));
    }

    #[test]
    fn delete_preview_80x24() {
        let mut ui = GraphUi::default();
        press(&mut ui, " d");
        insta::assert_snapshot!(draw(&ui, 80, 24));
    }

    #[test]
    fn fold_select_and_cursor_transitions() {
        let data = sample_graph();
        let mut ui = GraphUi::default();
        assert_eq!(ui.rows(&data.snap).len(), 2); // chat1 cluster, ungrouped
        ui.update(Action::Enter, &data);
        assert_eq!(ui.rows(&data.snap).len(), 3); // + k1
        ui.update(Action::Enter, &data);
        assert_eq!(ui.rows(&data.snap).len(), 2);
        ui.update(Action::Up, &data);
        assert_eq!(ui.cursor, 0);
        for _ in 0..9 {
            ui.update(Action::Down, &data);
        }
        assert_eq!(ui.cursor, 1);
    }

    #[test]
    fn select_launches_practice_and_delete_needs_confirm() {
        let mut ui = GraphUi::default();
        assert_eq!(press(&mut ui, "p"), None);
        press(&mut ui, " ");
        assert_eq!(ui.selected.len(), 3);
        assert_eq!(
            press(&mut ui, "p"),
            Some(Command::Practice {
                chains: vec!["k1".into(), "k2".into()]
            })
        );
        assert_eq!(press(&mut ui, "y"), None);
        press(&mut ui, "d");
        assert!(ui.confirming_delete);
        ui.update(Action::CloseModal, &sample_graph());
        assert!(!ui.confirming_delete && ui.selected.len() == 3);
        press(&mut ui, "d");
        let Some(Command::DeleteNodes { nodes }) = press(&mut ui, "y") else {
            panic!("expected delete")
        };
        assert_eq!(nodes, ["a", "b", "c"]);
        assert!(ui.selected.is_empty() && !ui.confirming_delete);
    }

    #[test]
    fn camera_and_focus_transitions() {
        let mut ui = GraphUi::default();
        press(&mut ui, "++l");
        assert!((ui.zoom - 1.5625).abs() < 1e-9 && ui.pan[0] > 0.0);
        press(&mut ui, "N");
        assert_eq!(ui.focus, 4);
        press(&mut ui, "n");
        assert_eq!(ui.focus, 0);
        press(&mut ui, "--------------");
        assert_eq!(ui.zoom, 0.25);
    }
}
