//! Knowledge graph contracts, deterministic merger, simplify_chains, clustering, and layout (T10).
use std::collections::{BTreeMap, BTreeSet, HashMap};

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use smol_str::SmolStr;

pub const MIN_CLUSTER_SIZE: usize = 3;

pub type NodeId = SmolStr;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provenance {
    pub chat_id: SmolStr,
    pub message_id: SmolStr,
}

/// One `nodes.jsonl` line. Later lines for the same `id` are link-appends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: NodeId,
    pub name: String,
    pub statement: String,
    pub aliases: Vec<String>,
    pub created_at: DateTime<Utc>,
    pub provenance: Vec<Provenance>,
}

/// One `chains.jsonl` line: `premises` jointly entail `conclusion`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphChain {
    pub id: SmolStr,
    pub premises: Vec<NodeId>,
    pub conclusion: NodeId,
    pub provenance: Vec<Provenance>,
    pub created_at: DateTime<Utc>,
}

impl GraphChain {
    /// Canonical merge key: `sorted(premises).join("+") + ">" + conclusion`.
    pub fn key(&self) -> String {
        let mut p: Vec<&str> = self.premises.iter().map(|s| s.as_str()).collect();
        p.sort_unstable();
        format!("{}>{}", p.join("+"), self.conclusion)
    }
}

/// One `coverage.jsonl` line: fingerprint of an extracted message view.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoverageEntry {
    pub chat_id: SmolStr,
    pub fp: String,
    pub tokens: u32,
    pub folded_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExtractedProposition {
    pub id: String,
    pub name: String,
    pub statement: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct ExtractedChain {
    pub premises: Vec<String>,
    pub conclusion: String,
}

/// Extractor output for one batch; `message_ids` are the passages it covered.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SessionFindings {
    pub chat_id: String,
    pub message_ids: Vec<String>,
    pub propositions: Vec<ExtractedProposition>,
    pub chains: Vec<ExtractedChain>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuditLink {
    pub id: String,
    pub link_to: String,
}

/// Auditor output: identity verdicts only.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct AuditVerdicts {
    pub links: Vec<AuditLink>,
}

/// Nodes born in the same session, at least `MIN_CLUSTER_SIZE` of them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Cluster {
    pub chat_id: SmolStr,
    pub nodes: Vec<NodeId>,
}

/// Immutable folded view of the ledgers.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct GraphSnapshot {
    pub nodes: BTreeMap<NodeId, GraphNode>,
    pub chains: Vec<GraphChain>,
}

fn union_provenance(into: &mut Vec<Provenance>, more: &[Provenance]) {
    for p in more {
        if !into.contains(p) {
            into.push(p.clone());
        }
    }
}

impl GraphSnapshot {
    /// Fold raw ledger lines: node link-appends join aliases, chains merge by canonical key.
    pub fn from_ledgers(node_lines: &[GraphNode], chain_lines: &[GraphChain]) -> Self {
        let mut nodes: BTreeMap<NodeId, GraphNode> = BTreeMap::new();
        for line in node_lines {
            match nodes.get_mut(&line.id) {
                None => {
                    nodes.insert(line.id.clone(), line.clone());
                }
                Some(n) => {
                    for a in std::iter::once(&line.statement).chain(&line.aliases) {
                        if *a != n.statement && !n.aliases.contains(a) {
                            n.aliases.push(a.clone());
                        }
                    }
                    union_provenance(&mut n.provenance, &line.provenance);
                }
            }
        }
        Self {
            nodes,
            chains: merge_chain_lines(chain_lines),
        }
    }

    /// Nodes grouped by birth session (first provenance); small groups stay unclustered.
    pub fn birth_session_partition(&self) -> Vec<Cluster> {
        let mut groups: BTreeMap<SmolStr, Vec<NodeId>> = BTreeMap::new();
        for n in self.nodes.values() {
            if let Some(p) = n.provenance.first() {
                groups
                    .entry(p.chat_id.clone())
                    .or_default()
                    .push(n.id.clone());
            }
        }
        groups
            .into_iter()
            .filter(|(_, v)| v.len() >= MIN_CLUSTER_SIZE)
            .map(|(chat_id, nodes)| Cluster { chat_id, nodes })
            .collect()
    }

    /// Chains that touch any selected node (as premise or conclusion).
    pub fn chains_for_selection(&self, selected: &[NodeId]) -> Vec<&GraphChain> {
        self.chains
            .iter()
            .filter(|c| touches(c, selected))
            .collect()
    }

    /// Accept a node id or chain id that exists in the graph.
    pub fn resolve_target_id(&self, target: &str) -> Option<SmolStr> {
        if self.nodes.contains_key(target) {
            return Some(SmolStr::new(target));
        }
        self.chains
            .iter()
            .find(|c| c.id == target)
            .map(|c| c.id.clone())
    }

    /// Ids of chains that die if `deleted` nodes are removed.
    pub fn deletion_blast_radius(&self, deleted: &[NodeId]) -> Vec<SmolStr> {
        self.chains
            .iter()
            .filter(|c| touches(c, deleted))
            .map(|c| c.id.clone())
            .collect()
    }

    /// Settle node positions with a deterministic ForceAtlas2 run over premise->conclusion edges.
    pub fn layout(&self, iterations: usize) -> BTreeMap<NodeId, [f64; 2]> {
        let ids: Vec<&NodeId> = self.nodes.keys().collect();
        let index: HashMap<&str, usize> = ids
            .iter()
            .enumerate()
            .map(|(i, id)| (id.as_str(), i))
            .collect();
        let mut edges = BTreeSet::new();
        for c in &self.chains {
            for p in &c.premises {
                if let (Some(&a), Some(&b)) =
                    (index.get(p.as_str()), index.get(c.conclusion.as_str()))
                {
                    edges.insert((a, b));
                }
            }
        }
        let pos = forceatlas2(
            ids.len(),
            &edges.into_iter().collect::<Vec<_>>(),
            iterations,
        );
        ids.into_iter().cloned().zip(pos).collect()
    }
}

fn touches(c: &GraphChain, nodes: &[NodeId]) -> bool {
    nodes.contains(&c.conclusion) || c.premises.iter().any(|p| nodes.contains(p))
}

fn merge_chain_lines(lines: &[GraphChain]) -> Vec<GraphChain> {
    let mut out: Vec<GraphChain> = Vec::new();
    let mut by_key: HashMap<String, usize> = HashMap::new();
    for line in lines {
        match by_key.get(&line.key()) {
            Some(&i) => union_provenance(&mut out[i].provenance, &line.provenance),
            None => {
                by_key.insert(line.key(), out.len());
                out.push(line.clone());
            }
        }
    }
    out
}

/// Drop redundant chains: prefer fewest premises, then transitive ancestry cover.
/// Iterates to a fixpoint so the result is idempotent.
pub fn simplify_chains(chains: &[GraphChain]) -> Vec<GraphChain> {
    let mut cur = chains.to_vec();
    loop {
        let next = simplify_once(&cur);
        if next.len() == cur.len() {
            return next;
        }
        cur = next;
    }
}

fn simplify_once(chains: &[GraphChain]) -> Vec<GraphChain> {
    // direct[x] = premises of chains concluding x
    let mut direct: HashMap<&str, Vec<&str>> = HashMap::new();
    for c in chains {
        direct
            .entry(c.conclusion.as_str())
            .or_default()
            .extend(c.premises.iter().map(|p| p.as_str()));
    }
    // cycle-guarded transitive ancestors of a node set
    let ancestors = |start: &[NodeId]| -> BTreeSet<&str> {
        let mut seen = BTreeSet::new();
        let mut stack: Vec<&str> = start.iter().map(|s| s.as_str()).collect();
        while let Some(n) = stack.pop() {
            for &p in direct.get(n).into_iter().flatten() {
                if seen.insert(p) {
                    stack.push(p);
                }
            }
        }
        seen
    };

    let mut order: Vec<usize> = (0..chains.len()).collect();
    order.sort_by_cached_key(|&i| {
        (
            chains[i].premises.len(),
            chains[i].created_at,
            chains[i].key(),
        )
    });

    let mut kept: Vec<usize> = Vec::new();
    for i in order {
        let cand = &chains[i];
        let covered = kept
            .iter()
            .map(|&k| &chains[k])
            .filter(|k| k.conclusion == cand.conclusion)
            .any(|k| {
                // rule 1: kept premises are a subset of the candidate's
                if k.premises.iter().all(|p| cand.premises.contains(p)) {
                    return true;
                }
                // rule 2: candidate premises are covered by kept premises or their ancestors
                let anc = ancestors(&k.premises);
                cand.premises
                    .iter()
                    .all(|p| k.premises.contains(p) || anc.contains(p.as_str()))
            });
        if !covered {
            kept.push(i);
        }
    }
    kept.sort_unstable();
    kept.into_iter().map(|i| chains[i].clone()).collect()
}

/// What the caller must persist after a merge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MergeOutput {
    /// New `nodes.jsonl` lines (births and link-appends).
    pub node_lines: Vec<GraphNode>,
    /// New `chains.jsonl` lines that survive simplification.
    pub chain_lines: Vec<GraphChain>,
    /// Full simplified chain ledger.
    pub chains: Vec<GraphChain>,
    /// True when an existing chain was dropped, so `chains` must replace the ledger atomically.
    pub rewrite: bool,
}

fn mint(prefix: &str, seed: &str) -> SmolStr {
    let h = hex::encode(Sha256::digest(seed.as_bytes()));
    SmolStr::new(format!("{prefix}_{}", &h[..12]))
}

/// Deterministically fold extractor findings and auditor verdicts into the graph.
pub fn merge_findings(
    snap: &GraphSnapshot,
    findings: &SessionFindings,
    verdicts: &AuditVerdicts,
    now: DateTime<Utc>,
) -> MergeOutput {
    let provenance: Vec<Provenance> = findings
        .message_ids
        .iter()
        .map(|m| Provenance {
            chat_id: SmolStr::new(&findings.chat_id),
            message_id: SmolStr::new(m),
        })
        .collect();

    let mut local: HashMap<&str, NodeId> = HashMap::new();
    let mut node_lines = Vec::new();
    for p in &findings.propositions {
        if local.contains_key(p.id.as_str()) {
            continue;
        }
        let link = verdicts
            .links
            .iter()
            .find(|l| l.id == p.id && snap.nodes.contains_key(l.link_to.as_str()));
        let (id, aliases) = match link {
            Some(l) => (SmolStr::new(&l.link_to), vec![p.name.clone()]),
            None => (
                mint("n", &format!("{}/{}", findings.chat_id, p.id)),
                Vec::new(),
            ),
        };
        local.insert(&p.id, id.clone());
        node_lines.push(GraphNode {
            id,
            name: p.name.clone(),
            statement: p.statement.clone(),
            aliases,
            created_at: now,
            provenance: provenance.clone(),
        });
    }

    let resolve = |s: &String| -> Option<NodeId> {
        local
            .get(s.as_str())
            .cloned()
            .or_else(|| snap.nodes.contains_key(s.as_str()).then(|| SmolStr::new(s)))
    };
    let mut lines = snap.chains.clone();
    for c in &findings.chains {
        let (Some(conclusion), Some(mut premises)) = (
            resolve(&c.conclusion),
            c.premises.iter().map(resolve).collect::<Option<Vec<_>>>(),
        ) else {
            continue;
        };
        premises.sort_unstable();
        premises.dedup();
        if premises.is_empty() || premises.contains(&conclusion) {
            continue;
        }
        let mut chain = GraphChain {
            id: SmolStr::default(),
            premises,
            conclusion,
            provenance: provenance.clone(),
            created_at: now,
        };
        chain.id = mint("arg", &chain.key());
        lines.push(chain);
    }

    let new_lines = &lines[snap.chains.len()..];
    let chains = simplify_chains(&merge_chain_lines(&lines));
    let survives = |c: &GraphChain| chains.iter().any(|k| k.key() == c.key());
    let chain_lines = new_lines.iter().filter(|c| survives(c)).cloned().collect();
    let rewrite = snap.chains.iter().any(|c| !survives(c));
    MergeOutput {
        node_lines,
        chain_lines,
        chains,
        rewrite,
    }
}

/// ForceAtlas2 (linear attraction, degree-weighted repulsion, gravity) from a deterministic
/// golden-angle spiral start. O(n^2) per iteration, which is fine for TUI-sized graphs.
fn forceatlas2(n: usize, edges: &[(usize, usize)], iterations: usize) -> Vec<[f64; 2]> {
    const REPULSION: f64 = 2.0;
    const GRAVITY: f64 = 1.0;
    const STEP: f64 = 0.1;
    let mut deg = vec![1.0; n];
    for &(a, b) in edges {
        deg[a] += 1.0;
        deg[b] += 1.0;
    }
    let mut pos: Vec<[f64; 2]> = (0..n)
        .map(|i| {
            let r = (i as f64 + 1.0).sqrt();
            let t = i as f64 * 2.399_963_229_728_653;
            [r * t.cos(), r * t.sin()]
        })
        .collect();
    for _ in 0..iterations {
        let mut f = vec![[0.0; 2]; n];
        for i in 0..n {
            for j in i + 1..n {
                let (dx, dy) = (pos[i][0] - pos[j][0], pos[i][1] - pos[j][1]);
                let d2 = (dx * dx + dy * dy).max(1e-6);
                let k = REPULSION * deg[i] * deg[j] / d2;
                f[i][0] += dx * k;
                f[i][1] += dy * k;
                f[j][0] -= dx * k;
                f[j][1] -= dy * k;
            }
        }
        for &(a, b) in edges {
            let (dx, dy) = (pos[b][0] - pos[a][0], pos[b][1] - pos[a][1]);
            f[a][0] += dx;
            f[a][1] += dy;
            f[b][0] -= dx;
            f[b][1] -= dy;
        }
        for i in 0..n {
            let g = GRAVITY * deg[i];
            f[i][0] -= pos[i][0] * g / (pos[i][0].hypot(pos[i][1]).max(1e-6));
            f[i][1] -= pos[i][1] * g / (pos[i][0].hypot(pos[i][1]).max(1e-6));
            // cap displacement so the system cannot blow up
            let mag = f[i][0].hypot(f[i][1]);
            let s = if mag * STEP > 1.0 { 1.0 / mag } else { STEP };
            pos[i][0] += f[i][0] * s;
            pos[i][1] += f[i][1] * s;
        }
    }
    pos
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn t(secs: i64) -> DateTime<Utc> {
        DateTime::from_timestamp(secs, 0).unwrap()
    }

    fn chain(prem: &[&str], concl: &str, secs: i64) -> GraphChain {
        let mut premises: Vec<NodeId> = prem.iter().map(SmolStr::new).collect();
        premises.sort_unstable();
        let mut c = GraphChain {
            id: SmolStr::default(),
            premises,
            conclusion: SmolStr::new(concl),
            provenance: vec![],
            created_at: t(secs),
        };
        c.id = mint("arg", &c.key());
        c
    }

    fn node(id: &str, chat: &str) -> GraphNode {
        GraphNode {
            id: SmolStr::new(id),
            name: id.into(),
            statement: format!("{id} holds"),
            aliases: vec![],
            created_at: t(0),
            provenance: vec![Provenance {
                chat_id: SmolStr::new(chat),
                message_id: "m1".into(),
            }],
        }
    }

    fn keys(cs: &[GraphChain]) -> Vec<String> {
        cs.iter().map(|c| c.key()).collect()
    }

    #[test]
    fn rule1_drops_strict_superset() {
        let out = simplify_chains(&[chain(&["A", "B", "C"], "X", 1), chain(&["A", "B"], "X", 2)]);
        assert_eq!(keys(&out), ["A+B>X"]);
    }

    #[test]
    fn rule2_transitive_ancestry_cover() {
        let out = simplify_chains(&[
            chain(&["A", "B"], "X", 1),
            chain(&["X", "C"], "Y", 2),
            chain(&["A", "B", "C"], "Y", 3),
        ]);
        assert_eq!(keys(&out), ["A+B>X", "C+X>Y"]);
    }

    #[test]
    fn cycles_terminate() {
        let cs = [
            chain(&["A"], "B", 1),
            chain(&["B"], "A", 2),
            chain(&["A", "B"], "A", 3),
        ];
        assert_eq!(keys(&simplify_chains(&cs)), ["A>B", "B>A"]);
    }

    #[test]
    fn link_append_folds_into_aliases() {
        let mut link = node("n_1", "chat2");
        link.statement = "same thing, reworded".into();
        link.provenance[0].message_id = "m9".into();
        let snap = GraphSnapshot::from_ledgers(&[node("n_1", "chat1"), link], &[]);
        let n = &snap.nodes["n_1"];
        assert_eq!(n.statement, "n_1 holds");
        assert_eq!(n.aliases, ["same thing, reworded"]);
        assert_eq!(n.provenance.len(), 2);
    }

    #[test]
    fn chains_merge_by_canonical_key() {
        let mut a = chain(&["A", "B"], "X", 1);
        let mut b = chain(&["A", "B"], "X", 2);
        a.provenance = vec![Provenance {
            chat_id: "c1".into(),
            message_id: "m".into(),
        }];
        b.premises.reverse();
        b.provenance = vec![Provenance {
            chat_id: "c2".into(),
            message_id: "m".into(),
        }];
        let snap = GraphSnapshot::from_ledgers(&[], &[a, b]);
        assert_eq!(snap.chains.len(), 1);
        assert_eq!(snap.chains[0].provenance.len(), 2);
    }

    fn findings() -> SessionFindings {
        let p = |id: &str| ExtractedProposition {
            id: id.into(),
            name: id.into(),
            statement: format!("{id} is true"),
        };
        SessionFindings {
            chat_id: "chat1".into(),
            message_ids: vec!["m1".into()],
            propositions: vec![p("p1"), p("p2"), p("p3")],
            chains: vec![
                ExtractedChain {
                    premises: vec!["p1".into(), "p2".into()],
                    conclusion: "p3".into(),
                },
                ExtractedChain {
                    premises: vec!["p1".into()],
                    conclusion: "p1".into(),
                },
                ExtractedChain {
                    premises: vec!["nope".into()],
                    conclusion: "p3".into(),
                },
            ],
        }
    }

    #[test]
    fn merge_mints_resolves_and_drops_bad_chains() {
        let out = merge_findings(
            &GraphSnapshot::default(),
            &findings(),
            &AuditVerdicts::default(),
            t(5),
        );
        assert_eq!(out.node_lines.len(), 3);
        assert!(out.node_lines[0].id.starts_with("n_") && out.node_lines[0].id.len() == 14);
        assert_eq!(out.chains.len(), 1);
        assert!(out.chains[0].id.starts_with("arg_"));
        assert!(!out.rewrite);
        let again = merge_findings(
            &GraphSnapshot::default(),
            &findings(),
            &AuditVerdicts::default(),
            t(5),
        );
        assert_eq!(out, again);
    }

    #[test]
    fn merge_links_to_live_node_only() {
        let snap = GraphSnapshot::from_ledgers(&[node("n_old", "chat0")], &[]);
        let verdicts = AuditVerdicts {
            links: vec![
                AuditLink {
                    id: "p1".into(),
                    link_to: "n_old".into(),
                },
                AuditLink {
                    id: "p2".into(),
                    link_to: "n_ghost".into(),
                },
            ],
        };
        let out = merge_findings(&snap, &findings(), &verdicts, t(5));
        assert_eq!(out.node_lines[0].id, "n_old");
        assert!(out.node_lines[1].id.starts_with("n_") && out.node_lines[1].id != "n_ghost");
    }

    #[test]
    fn merge_rewrites_when_existing_chain_becomes_redundant() {
        let snap = GraphSnapshot::from_ledgers(
            &[node("A", "c"), node("B", "c"), node("X", "c")],
            &[chain(&["A", "B"], "X", 1)],
        );
        let f = SessionFindings {
            chat_id: "c".into(),
            message_ids: vec![],
            propositions: vec![],
            chains: vec![ExtractedChain {
                premises: vec!["A".into()],
                conclusion: "X".into(),
            }],
        };
        let out = merge_findings(&snap, &f, &AuditVerdicts::default(), t(9));
        assert!(out.rewrite);
        assert_eq!(keys(&out.chains), ["A>X"]);
        assert_eq!(out.chain_lines.len(), 1);
    }

    #[test]
    fn partition_requires_min_cluster_size() {
        let ns: Vec<GraphNode> = ["a", "b", "c"]
            .iter()
            .map(|i| node(i, "big"))
            .chain(["d", "e"].iter().map(|i| node(i, "small")))
            .collect();
        let parts = GraphSnapshot::from_ledgers(&ns, &[]).birth_session_partition();
        assert_eq!(parts.len(), 1);
        assert_eq!(parts[0].chat_id, "big");
        assert_eq!(parts[0].nodes, ["a", "b", "c"]);
    }

    #[test]
    fn selection_resolution_and_blast_radius() {
        let c1 = chain(&["A", "B"], "X", 1);
        let c2 = chain(&["X"], "Y", 2);
        let c3 = chain(&["P"], "Q", 3);
        let snap = GraphSnapshot::from_ledgers(
            &[node("A", "c"), node("X", "c")],
            &[c1.clone(), c2.clone(), c3],
        );
        let sel = [SmolStr::new("X")];
        assert_eq!(snap.chains_for_selection(&sel).len(), 2);
        assert_eq!(snap.deletion_blast_radius(&sel), [c1.id.clone(), c2.id]);
        assert_eq!(snap.resolve_target_id("A").as_deref(), Some("A"));
        assert_eq!(snap.resolve_target_id(&c1.id).as_ref(), Some(&c1.id));
        assert_eq!(snap.resolve_target_id("zzz"), None);
    }

    #[test]
    fn layout_is_deterministic_finite_and_pulls_linked_nodes_together() {
        let ns: Vec<GraphNode> = ["A", "B", "C"].iter().map(|i| node(i, "c")).collect();
        let snap = GraphSnapshot::from_ledgers(&ns, &[chain(&["A"], "B", 1)]);
        let l = snap.layout(300);
        assert_eq!(l, snap.layout(300));
        assert!(l.values().all(|p| p[0].is_finite() && p[1].is_finite()));
        let d = |a: &str, b: &str| (l[a][0] - l[b][0]).hypot(l[a][1] - l[b][1]);
        assert!(d("A", "B") < d("A", "C"));
    }

    fn arb_chains() -> impl Strategy<Value = Vec<GraphChain>> {
        let id = prop::sample::select(vec!["A", "B", "C", "D", "E", "F"]);
        prop::collection::vec(
            (prop::collection::btree_set(id.clone(), 1..4), id, 0i64..50),
            0..12,
        )
        .prop_map(|v| {
            v.into_iter()
                .filter(|(p, c, _)| !p.contains(c))
                .map(|(p, c, s)| chain(&p.into_iter().collect::<Vec<_>>(), c, s))
                .collect()
        })
    }

    proptest! {
        #[test]
        fn simplify_is_idempotent_subset_and_cycle_safe(cs in arb_chains()) {
            let once = simplify_chains(&cs);
            prop_assert_eq!(simplify_chains(&once), once.clone());
            prop_assert!(once.iter().all(|c| cs.contains(c)));
            // every conclusion keeps at least one chain
            for c in &cs {
                prop_assert!(once.iter().any(|k| k.conclusion == c.conclusion));
            }
        }

        #[test]
        fn extra_premise_on_transitive_chain_is_dropped(extra in prop::sample::select(vec!["A", "B", "C"])) {
            let out = simplify_chains(&[
                chain(&["A", "B"], "X", 1),
                chain(&["X", "C"], "Y", 2),
                chain(&["X", "C", extra], "Y", 3),
            ]);
            prop_assert_eq!(keys(&out), ["A+B>X", "C+X>Y"]);
        }
    }
}
