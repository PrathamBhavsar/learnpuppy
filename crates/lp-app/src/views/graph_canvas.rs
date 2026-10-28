use std::collections::{BTreeMap, BTreeSet};
use std::rc::Rc;

use dioxus::html::input_data::MouseButton;
use dioxus::prelude::*;
use lp_core::graph::{GraphChain, GraphNode, GraphSnapshot, NodeId};
use lp_ui::overlays::{Checkbox, ScrollArea};
use lp_ui::primitives::{Button, ButtonSize, ButtonVariant, Card, MasteryRing};

use crate::backend::BackendHandle;
use crate::state::{AppState, QuickAction};

#[derive(Clone, PartialEq)]
struct CanvasData {
    graph: GraphSnapshot,
    positions: BTreeMap<NodeId, [f64; 2]>,
}

#[derive(Clone, Copy, PartialEq)]
struct Camera {
    x: f64,
    y: f64,
    zoom: f64,
}

impl Default for Camera {
    fn default() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            zoom: 1.0,
        }
    }
}

impl Camera {
    fn zoom_by(&mut self, factor: f64) {
        if factor.is_finite() && factor > 0.0 {
            self.zoom = (self.zoom * factor).clamp(0.2, 5.0);
        }
    }
}

fn toggle_selection(mut selected: Signal<BTreeSet<NodeId>>, id: NodeId) {
    let mut set = selected.write();
    if !set.remove(&id) {
        set.insert(id);
    }
}

fn curve(from: [f64; 2], to: [f64; 2]) -> String {
    let mid = (from[0] + to[0]) / 2.0;
    format!(
        "M {} {} C {} {}, {} {}, {} {}",
        from[0], from[1], mid, from[1], mid, to[1], to[0], to[1]
    )
}

fn node_boundary(from: [f64; 2], to: [f64; 2]) -> [f64; 2] {
    let dx = to[0] - from[0];
    let dy = to[1] - from[1];
    let distance = dx.hypot(dy).max(1.0);
    [to[0] - dx / distance * 27.0, to[1] - dy / distance * 27.0]
}

fn fitted_positions(raw: &BTreeMap<NodeId, [f64; 2]>) -> BTreeMap<NodeId, [f64; 2]> {
    let bounds = raw
        .values()
        .filter(|p| p[0].is_finite() && p[1].is_finite())
        .fold(
            [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ],
            |b, p| {
                [
                    b[0].min(p[0]),
                    b[1].min(p[1]),
                    b[2].max(p[0]),
                    b[3].max(p[1]),
                ]
            },
        );
    let scale =
        (760.0 / (bounds[2] - bounds[0]).max(1.0)).min(400.0 / (bounds[3] - bounds[1]).max(1.0));
    raw.iter()
        .filter(|(_, p)| p[0].is_finite() && p[1].is_finite())
        .map(|(id, p)| {
            (
                id.clone(),
                [
                    500.0 + (p[0] - (bounds[0] + bounds[2]) / 2.0) * scale,
                    320.0 + (p[1] - (bounds[1] + bounds[3]) / 2.0) * scale,
                ],
            )
        })
        .collect()
}

#[component]
pub fn GraphCanvasView() -> Element {
    let _state = use_context::<AppState>();
    let backend = use_context::<BackendHandle>();
    let mut graph = use_resource(move || {
        let backend = backend.clone();
        async move {
            let snapshot = backend.graph().await?;
            // ForceAtlas2 is CPU work; settle it away from the UI thread.
            tokio::task::spawn_blocking(move || {
                let positions = fitted_positions(&snapshot.layout(160));
                CanvasData {
                    graph: snapshot,
                    positions,
                }
            })
            .await
            .map(Rc::new)
            .map_err(|error| error.to_string())
        }
    });
    rsx! {
        section { "aria-label": "Knowledge graph", "data-testid": "view-graph",
            style: "height: 100%; min-height: 0; display: flex; flex-direction: column; overflow: hidden;",
            match graph() {
                Some(Ok(data)) => rsx! { GraphWorkspace { data } },
                Some(Err(error)) => rsx! {
                    Card { aria_label: "Graph unavailable", test_id: "graph-error",
                        h1 { "Knowledge graph" }
                        p { role: "alert", "{error}" }
                        Button { aria_label: "Retry loading graph", test_id: "graph-retry", onclick: move |_| graph.restart(), "Try again" }
                    }
                },
                None => rsx! { p { role: "status", "data-testid": "graph-loading", "Loading knowledge graph…" } },
            }
        }
    }
}

#[component]
fn GraphWorkspace(data: Rc<CanvasData>) -> Element {
    let mut state = use_context::<AppState>();
    let mut camera = use_signal(Camera::default);
    let mut drag = use_signal(|| None::<[f64; 2]>);
    let mut viewport_scale = use_signal(|| 1.0_f64);
    let mut mounted_canvas = use_signal(|| None::<Rc<MountedData>>);
    let mut selected = use_signal(BTreeSet::<NodeId>::new);
    let mut inspected = use_signal(|| None::<NodeId>);
    let mut table_open = use_signal(|| false);
    let expanded = use_signal(BTreeSet::<String>::new);
    let mut practice_targets = use_signal(Vec::<String>::new);
    let graph = &data.graph;
    let cam = camera();
    let transform = format!(
        "translate(500 320) scale({}) translate({} {})",
        cam.zoom,
        cam.x - 500.0,
        cam.y - 320.0
    );
    let clusters = graph.birth_session_partition();
    let drawer_node = inspected().and_then(|id| graph.nodes.get(&id).cloned());
    rsx! {
        header { style: "display: flex; flex-wrap: wrap; align-items: center; gap: 0.75rem; padding: 1rem; border-bottom: 1px solid var(--border);",
            div { style: "flex: 1; min-width: 12rem;",
                h1 { style: "margin: 0; font-size: 1.25rem;", "Knowledge graph" }
                p { style: "margin: 0.25rem 0 0; color: var(--muted-foreground);", "{graph.nodes.len()} propositions · {graph.chains.len()} reasoning chains" }
            }
            Button { variant: ButtonVariant::Outline, size: ButtonSize::Sm, aria_label: "Toggle reasoning chain table", test_id: "graph-toggle-table",
                "aria-expanded": table_open().to_string(), "aria-controls": "graph-chain-table", onclick: move |_| table_open.toggle(), "Chain table" }
            Button { disabled: selected().is_empty(), aria_label: "Launch adaptive practice with selected propositions", test_id: "graph-practice",
                onclick: move |_| {
                    practice_targets.set(selected().iter().map(ToString::to_string).collect());
                    state.select_action(QuickAction::Practice);
                }, "Practice selected ({selected().len()})" }
        }
        div { style: "display: flex; flex: 1; min-height: 0; overflow: hidden;",
            div { style: "flex: 1 1 18rem; min-width: 0; min-height: 12rem; position: relative; overflow: hidden;",
                svg { xmlns: "http://www.w3.org/2000/svg", view_box: "0 0 1000 640", role: "group",
                    "aria-label": "Knowledge graph canvas. Drag to pan, scroll to zoom, Enter to inspect, Space to select.",
                    "aria-describedby": "graph-help", "data-testid": "graph-canvas", tabindex: "0",
                    style: "position: absolute; inset: 0; display: block; width: 100%; height: 100%; touch-action: none;",
                    onmounted: move |event| async move {
                        mounted_canvas.set(Some(event.data()));
                        if let Ok(rect) = event.get_client_rect().await {
                            viewport_scale.set((rect.size.width / 1000.0).min(rect.size.height / 640.0).max(0.01));
                        }
                    },
                    onwheel: move |event| { event.prevent_default(); camera.write().zoom_by((-event.delta().strip_units().y * 0.0015).exp()); },
                    onmousedown: move |event| {
                        if event.trigger_button() == Some(MouseButton::Primary) {
                            event.prevent_default();
                            let p = event.client_coordinates(); drag.set(Some([p.x, p.y]));
                            if let Some(element) = mounted_canvas() {
                                spawn(async move {
                                    if let Ok(rect) = element.get_client_rect().await {
                                        viewport_scale.set((rect.size.width / 1000.0).min(rect.size.height / 640.0).max(0.01));
                                    }
                                });
                            }
                        }
                    },
                    onmousemove: move |event| {
                        if let Some(previous) = drag() {
                            let p = event.client_coordinates(); let scale = viewport_scale() * camera().zoom;
                            let mut cam = camera.write(); cam.x += (p.x - previous[0]) / scale; cam.y += (p.y - previous[1]) / scale;
                            drag.set(Some([p.x, p.y]));
                        }
                    },
                    onmouseup: move |_| drag.set(None), onmouseleave: move |_| drag.set(None),
                    onkeydown: move |event| {
                        match event.key() {
                            Key::Character(key) if key == "+" || key == "=" => { event.prevent_default(); camera.write().zoom_by(1.2); }
                            Key::Character(key) if key == "-" => { event.prevent_default(); camera.write().zoom_by(1.0 / 1.2); }
                            Key::Home => { event.prevent_default(); camera.set(Camera::default()); }
                            Key::ArrowLeft => { event.prevent_default(); camera.write().x += 30.0 / cam.zoom; }
                            Key::ArrowRight => { event.prevent_default(); camera.write().x -= 30.0 / cam.zoom; }
                            Key::ArrowUp => { event.prevent_default(); camera.write().y += 30.0 / cam.zoom; }
                            Key::ArrowDown => { event.prevent_default(); camera.write().y -= 30.0 / cam.zoom; }
                            Key::Escape => { inspected.set(None); drag.set(None); }
                            _ => {}
                        }
                    },
                    defs { marker { id: "graph-arrow", view_box: "0 0 10 10", ref_x: "9", ref_y: "5", marker_width: "6", marker_height: "6", orient: "auto-start-reverse",
                        path { d: "M 0 0 L 10 5 L 0 10 z", fill: "var(--muted-foreground)" }
                    } }
                    g { transform,
                        for cluster in &clusters {
                            if let Some(b) = hull_bounds(&cluster.nodes, &data.positions) {
                                rect { key: "hull-{cluster.chat_id}", x: b[0], y: b[1], width: b[2], height: b[3], rx: "36",
                                    fill: "var(--primary)", fill_opacity: "0.06", stroke: "var(--border)", stroke_dasharray: "6 6",
                                    "data-testid": "graph-cluster-hull", "aria-label": "Birth session {cluster.chat_id}" }
                                text { x: b[0] + 20.0, y: b[1] + 24.0, fill: "var(--muted-foreground)", font_size: "12", "Session {cluster.chat_id}" }
                            }
                        }
                        for chain in &graph.chains {
                            if let Some(to) = data.positions.get(&chain.conclusion) {
                                if let Some(junction) = chain_junction(chain, &data.positions) {
                                    for premise in &chain.premises {
                                        if let Some(from) = data.positions.get(premise) {
                                            path { key: "edge-{chain.id}-{premise}", d: curve(*from, junction), fill: "none", stroke: "var(--muted-foreground)", stroke_width: "1.5", "data-testid": "graph-premise-edge" }
                                        }
                                    }
                                    path { d: curve(junction, node_boundary(junction, *to)), fill: "none", stroke: "var(--muted-foreground)", stroke_width: "1.5", marker_end: "url(#graph-arrow)", "data-testid": "graph-conclusion-edge" }
                                    circle { cx: junction[0], cy: junction[1], r: "7", fill: "var(--accent)", stroke: "var(--primary)", "data-testid": "graph-chain-junction", title { "Joint implication {chain.id}" } }
                                }
                            }
                        }
                        for node in graph.nodes.values() {
                            if let Some(pos) = data.positions.get(&node.id) {
                                GraphProposition { key: "{node.id}", node: node.clone(), position: *pos, selected, inspected }
                            }
                        }
                    }
                }
                if graph.nodes.is_empty() {
                    div { "data-testid": "graph-empty", role: "status", style: "position: absolute; inset: 25% 15% auto; text-align: center; pointer-events: none;",
                        h2 { "Your graph starts with a conversation" }
                        p { "Learn in Classroom or fold a coding digest into the graph to connect your first propositions." }
                    }
                }
                div { role: "toolbar", "aria-label": "Graph camera controls", style: "position: absolute; bottom: 1rem; left: 1rem; display: flex; align-items: center; gap: 0.5rem;",
                    Button { variant: ButtonVariant::Outline, size: ButtonSize::Sm, aria_label: "Zoom out", test_id: "graph-zoom-out", onclick: move |_| camera.write().zoom_by(1.0 / 1.2), "−" }
                    output { "aria-label": "Graph zoom", "data-testid": "graph-zoom-level", "{(cam.zoom * 100.0).round()}%" }
                    Button { variant: ButtonVariant::Outline, size: ButtonSize::Sm, aria_label: "Zoom in", test_id: "graph-zoom-in", onclick: move |_| camera.write().zoom_by(1.2), "+" }
                    Button { variant: ButtonVariant::Outline, size: ButtonSize::Sm, aria_label: "Reset graph camera", test_id: "graph-camera-reset", onclick: move |_| camera.set(Camera::default()), "Fit graph" }
                }
            }
            if let Some(node) = drawer_node {
                aside { role: "complementary", "aria-label": "Proposition provenance", "data-testid": "graph-provenance-drawer",
                    style: "flex: 0 1 21rem; min-width: 15rem; max-height: 100%; overflow-y: auto; padding: 1rem; border-left: 1px solid var(--border); background: var(--card);",
                    onkeydown: move |event| {
                        if event.key() == Key::Escape { event.prevent_default(); inspected.set(None); }
                    },
                    Button { variant: ButtonVariant::Ghost, size: ButtonSize::Sm, aria_label: "Close proposition details", test_id: "graph-close-provenance", onclick: move |_| inspected.set(None), "Close" }
                    h2 { "{node.name}" } p { "{node.statement}" } code { "{node.id}" }
                    MasteryRing { score: 0.0, aria_label: "Mastery not yet assessed", test_id: "graph-provenance-mastery", show_label: false }
                    p { "Mastery not yet assessed" }
                    Checkbox { checked: selected().contains(&node.id), aria_label: format!("Select {} for practice", node.name), label: "Include in practice", test_id: "graph-drawer-select",
                        on_checked_change: { let id = node.id.clone(); move |_| toggle_selection(selected, id.clone()) } }
                    h3 { "Source messages" }
                    if node.provenance.is_empty() { p { "No source messages recorded." } }
                    for source in &node.provenance {
                        p { key: "{source.chat_id}:{source.message_id}", "data-testid": "graph-provenance-source", "Session {source.chat_id} · Message {source.message_id}" }
                    }
                    if !node.aliases.is_empty() { h3 { "Other statements" } for alias in &node.aliases { p { "{alias}" } } }
                }
            }
        }
        footer { style: "padding: 0.5rem 1rem; border-top: 1px solid var(--border); color: var(--muted-foreground);",
            p { id: "graph-help", style: "margin: 0;", "Drag to pan · Scroll to zoom · Enter for sources · Space to select · Home to fit" }
            if !selected().is_empty() {
                Button { variant: ButtonVariant::Ghost, size: ButtonSize::Sm, aria_label: "Clear selected propositions", test_id: "graph-clear-selection", onclick: move |_| selected.write().clear(), "Clear {selected().len()} selected" }
            }
            p { role: "status", "aria-live": "polite", "data-testid": "graph-practice-status",
                if !practice_targets().is_empty() { "Adaptive practice launched for {practice_targets().len()} propositions." }
            }
        }
        if table_open() {
            ScrollArea { aria_label: "Reasoning chain table", test_id: "graph-chain-table", max_height: "35%",
                div { id: "graph-chain-table", role: "tree", "aria-label": "Clusters, reasoning chains, and statements", style: "padding: 1rem;",
                    for group in chain_groups(graph) {
                        ChainGroup { key: "{group.0}", label: group.0, chains: group.1, graph: Rc::new(graph.clone()), expanded, selected, inspected }
                    }
                    if graph.chains.is_empty() { p { "No reasoning chains yet." } }
                }
            }
        }
    }
}

fn hull_bounds(ids: &[NodeId], positions: &BTreeMap<NodeId, [f64; 2]>) -> Option<[f64; 4]> {
    let mut points = ids.iter().filter_map(|id| positions.get(id));
    let first = points.next()?;
    let b = points.fold([first[0], first[1], first[0], first[1]], |b, p| {
        [
            b[0].min(p[0]),
            b[1].min(p[1]),
            b[2].max(p[0]),
            b[3].max(p[1]),
        ]
    });
    Some([
        b[0] - 55.0,
        b[1] - 65.0,
        b[2] - b[0] + 110.0,
        b[3] - b[1] + 130.0,
    ])
}

fn chain_junction(chain: &GraphChain, positions: &BTreeMap<NodeId, [f64; 2]>) -> Option<[f64; 2]> {
    let to = positions.get(&chain.conclusion)?;
    let from: Vec<_> = chain
        .premises
        .iter()
        .filter_map(|id| positions.get(id))
        .collect();
    if from.is_empty() {
        return None;
    }
    let count = from.len() as f64;
    let junction = [
        (from.iter().map(|p| p[0]).sum::<f64>() / count + to[0]) / 2.0,
        (from.iter().map(|p| p[1]).sum::<f64>() / count + to[1]) / 2.0,
    ];
    // Opposing premises can average onto the conclusion; keep the implication visible.
    if (junction[0] - to[0]).hypot(junction[1] - to[1]) < 65.0 {
        Some([to[0] - 70.0, to[1] - 70.0])
    } else {
        Some(junction)
    }
}

#[component]
fn GraphProposition(
    node: GraphNode,
    position: [f64; 2],
    selected: Signal<BTreeSet<NodeId>>,
    mut inspected: Signal<Option<NodeId>>,
) -> Element {
    let click_id = node.id.clone();
    let key_id = node.id.clone();
    let is_selected = selected().contains(&node.id);
    rsx! {
        g { transform: "translate({position[0]} {position[1]})", role: "button", tabindex: "0",
            "aria-label": format!("Inspect {}. Space toggles practice selection.", node.name), "aria-pressed": is_selected.to_string(), "data-testid": format!("graph-node-{}", node.id),
            onmousedown: move |event| event.stop_propagation(),
            onclick: move |event| { event.stop_propagation(); inspected.set(Some(click_id.clone())); },
            onkeydown: move |event| {
                match event.key() {
                    Key::Enter => { event.prevent_default(); event.stop_propagation(); inspected.set(Some(key_id.clone())); }
                    Key::Character(key) if key == " " => { event.prevent_default(); event.stop_propagation(); toggle_selection(selected, key_id.clone()); }
                    _ => {}
                }
            },
            circle { r: "25", fill: "none", stroke: "var(--mastery-novice)", stroke_width: "3", stroke_dasharray: "3 4", "data-testid": "graph-mastery-ring", title { "Mastery not yet assessed" } }
            circle { r: "19", fill: if is_selected { "var(--primary)" } else { "var(--card)" }, stroke: if is_selected { "var(--ring)" } else { "var(--border)" }, stroke_width: "2" }
            if is_selected { text { text_anchor: "middle", y: "5", fill: "var(--primary-foreground)", "✓" } }
            text { text_anchor: "middle", y: "45", fill: "var(--foreground)", font_size: "13", "{node.name}" }
            title { "{node.statement}" }
        }
    }
}

fn chain_groups(graph: &GraphSnapshot) -> Vec<(String, Vec<GraphChain>)> {
    let mut groups = BTreeMap::<String, Vec<GraphChain>>::new();
    for chain in &graph.chains {
        let label = graph
            .nodes
            .get(&chain.conclusion)
            .and_then(|node| node.provenance.first())
            .map(|source| format!("Session {}", source.chat_id))
            .unwrap_or_else(|| "Unclustered".into());
        groups.entry(label).or_default().push(chain.clone());
    }
    groups.into_iter().collect()
}

fn toggle_expanded(mut expanded: Signal<BTreeSet<String>>, id: String) {
    let mut set = expanded.write();
    if !set.remove(&id) {
        set.insert(id);
    }
}

#[component]
fn ChainGroup(
    label: String,
    chains: Vec<GraphChain>,
    graph: Rc<GraphSnapshot>,
    expanded: Signal<BTreeSet<String>>,
    selected: Signal<BTreeSet<NodeId>>,
    mut inspected: Signal<Option<NodeId>>,
) -> Element {
    let group_key = format!("cluster:{label}");
    let is_open = expanded().contains(&group_key);
    rsx! {
        div { role: "treeitem", "aria-level": "1", "aria-expanded": is_open.to_string(),
            Button { variant: ButtonVariant::Ghost, aria_label: format!("Expand cluster {label}"), test_id: format!("graph-cluster-toggle-{label}"),
                "aria-expanded": is_open.to_string(), onclick: move |_| toggle_expanded(expanded, group_key.clone()),
                if is_open { "▾ " } else { "▸ " } "{label} ({chains.len()})"
            }
            if is_open {
                div { role: "group", style: "padding-left: 1rem;",
                    for chain in chains {
                        {
                            let chain_key = format!("chain:{}", chain.id);
                            let chain_open = expanded().contains(&chain_key);
                            let names = chain.premises.iter().map(|id| graph.nodes.get(id).map_or(id.as_str(), |n| n.name.as_str())).collect::<Vec<_>>().join(" ∧ ");
                            let conclusion = graph.nodes.get(&chain.conclusion).map_or(chain.conclusion.as_str(), |n| n.name.as_str());
                            rsx! {
                                div { key: "{chain.id}", role: "treeitem", "aria-level": "2", "aria-expanded": chain_open.to_string(),
                                    Button { variant: ButtonVariant::Ghost, aria_label: format!("Expand chain {}", chain.id), test_id: format!("graph-chain-toggle-{}", chain.id),
                                        "aria-expanded": chain_open.to_string(), onclick: move |_| toggle_expanded(expanded, chain_key.clone()),
                                        if chain_open { "▾ " } else { "▸ " } "{names} ⟹ {conclusion}"
                                    }
                                    if chain_open {
                                        div { role: "group", style: "padding-left: 1rem;",
                                            for id in chain.premises.iter().chain(std::iter::once(&chain.conclusion)) {
                                                if let Some(node) = graph.nodes.get(id) {
                                                    div { key: "{node.id}", role: "treeitem", "aria-level": "3", style: "display: flex; gap: 0.5rem; align-items: center;",
                                                        Checkbox { checked: selected().contains(id), aria_label: format!("Select {} for practice", node.name), test_id: format!("graph-chain-select-{}-{}", chain.id, node.id),
                                                            on_checked_change: { let id = id.clone(); move |_| toggle_selection(selected, id.clone()) } }
                                                        Button { variant: ButtonVariant::Link, aria_label: format!("Inspect {}", node.name), test_id: format!("graph-statement-{}-{}", chain.id, node.id),
                                                            onclick: { let id = id.clone(); move |_| inspected.set(Some(id.clone())) }, "{node.statement}" }
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
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::backend::FauxBackend;
    use crate::state::View;
    use crate::{AppWithBackend, AppWithBackendProps};
    use lp_core::graph::Provenance;
    use std::sync::Arc;

    fn sample_graph() -> GraphSnapshot {
        let nodes = (1..=3)
            .map(|index| GraphNode {
                id: format!("n_{index}").into(),
                name: format!("Proposition {index}"),
                statement: format!("Statement {index}"),
                aliases: vec![],
                created_at: "2026-10-28T00:00:00Z".parse().unwrap(),
                provenance: vec![Provenance {
                    chat_id: "birth-session".into(),
                    message_id: format!("message-{index}").into(),
                }],
            })
            .collect::<Vec<_>>();
        let chain = GraphChain {
            id: "arg_joint".into(),
            premises: vec!["n_1".into(), "n_2".into()],
            conclusion: "n_3".into(),
            provenance: nodes[0].provenance.clone(),
            created_at: nodes[0].created_at,
        };
        GraphSnapshot::from_ledgers(&nodes, &[chain])
    }

    async fn render_backend(backend: FauxBackend, marker: &str) -> String {
        let mut dom = VirtualDom::new_with_props(
            AppWithBackend,
            AppWithBackendProps {
                backend: Arc::new(backend),
                initial_view: View::Graph,
                accounts_open: false,
            },
        );
        dom.rebuild_in_place();
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let html = dioxus::ssr::render(&dom);
                if html.contains(marker) {
                    return html;
                }
                dom.wait_for_work().await;
                dom.render_immediate(&mut dioxus::dioxus_core::NoOpMutations);
            }
        })
        .await
        .expect("graph backend should settle")
    }

    #[tokio::test]
    async fn faux_backend_empty_graph_keeps_camera_and_disabled_practice_available() {
        let html = render_backend(FauxBackend::default(), "data-testid=\"graph-empty\"").await;
        assert!(html.contains("Your graph starts with a conversation"));
        for control in [
            "graph-canvas",
            "graph-zoom-in",
            "graph-zoom-out",
            "graph-camera-reset",
            "graph-toggle-table",
        ] {
            assert!(html.contains(&format!("data-testid=\"{control}\"")));
        }
        assert!(html.contains("aria-label=\"Launch adaptive practice with selected propositions\""));
        assert!(html.contains("aria-disabled=\"true\""));
        assert!(!html.contains("graph-provenance-drawer"));
    }

    #[tokio::test]
    async fn faux_backend_errors_offer_retry_without_discarding_the_shell() {
        let html = render_backend(
            FauxBackend {
                error: Some("Graph ledger unavailable".into()),
                ..Default::default()
            },
            "data-testid=\"graph-error\"",
        )
        .await;
        assert!(html.contains("role=\"alert\""));
        assert!(html.contains("Graph ledger unavailable"));
        assert!(html.contains("data-testid=\"graph-retry\""));
        assert!(html.contains("data-testid=\"navigation-rail\""));
    }

    fn populated_workspace() -> Element {
        AppState::use_provider(View::Graph, false);
        let graph = sample_graph();
        let positions = fitted_positions(&graph.layout(20));
        rsx! { GraphWorkspace { data: Rc::new(CanvasData { graph, positions }) } }
    }

    #[test]
    fn populated_canvas_renders_joint_entailment_clusters_and_unassessed_rings() {
        let mut dom = VirtualDom::new(populated_workspace);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);
        assert_eq!(
            html.matches("data-testid=\"graph-premise-edge\"").count(),
            2
        );
        assert_eq!(
            html.matches("data-testid=\"graph-conclusion-edge\"")
                .count(),
            1
        );
        assert_eq!(
            html.matches("data-testid=\"graph-chain-junction\"").count(),
            1
        );
        assert_eq!(
            html.matches("data-testid=\"graph-cluster-hull\"").count(),
            1
        );
        assert_eq!(
            html.matches("data-testid=\"graph-mastery-ring\"").count(),
            3
        );
        assert!(html.contains("Mastery not yet assessed"));
        for index in 1..=3 {
            assert!(html.contains(&format!("data-testid=\"graph-node-n_{index}\"")));
        }
        assert!(html.contains(" C "));
    }

    fn expanded_chain_table() -> Element {
        let graph = sample_graph();
        let selected = use_signal(|| BTreeSet::from([NodeId::from("n_2")]));
        let inspected = use_signal(|| None);
        let expanded = use_signal(|| {
            BTreeSet::from([
                "cluster:Session birth-session".into(),
                "chain:arg_joint".into(),
            ])
        });
        rsx! { ChainGroup { label: "Session birth-session", chains: graph.chains.clone(), graph: Rc::new(graph), expanded, selected, inspected } }
    }

    #[test]
    fn chain_table_expands_all_three_levels_and_preserves_selection() {
        let mut dom = VirtualDom::new(expanded_chain_table);
        dom.rebuild_in_place();
        let html = dioxus::ssr::render(&dom);
        for level in 1..=3 {
            assert!(html.contains(&format!("aria-level=\"{level}\"")));
        }
        assert!(html.contains("Proposition 1 ∧ Proposition 2 ⟹ Proposition 3"));
        assert!(html.contains("aria-checked=\"true\""));
        for index in 1..=3 {
            assert!(html.contains(&format!("Statement {index}")));
        }
    }

    #[test]
    fn fitting_handles_single_nodes_and_rejects_nonfinite_coordinates() {
        let raw = BTreeMap::from([
            ("single".into(), [10.0, 20.0]),
            ("invalid".into(), [f64::NAN, 0.0]),
        ]);
        let fitted = fitted_positions(&raw);
        assert_eq!(fitted.len(), 1);
        assert_eq!(fitted["single"], [500.0, 320.0]);
        assert!(fitted_positions(&BTreeMap::new()).is_empty());
        let mut camera = Camera::default();
        camera.zoom_by(100.0);
        assert_eq!(camera.zoom, 5.0);
        camera.zoom_by(0.001);
        assert_eq!(camera.zoom, 0.2);
        camera.zoom_by(f64::NAN);
        assert_eq!(camera.zoom, 0.2);
    }
}
