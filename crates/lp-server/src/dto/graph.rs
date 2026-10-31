//! Knowledge graph wire types (docs/web/03 §Graph).

use std::collections::{BTreeMap, HashMap};

use lp_core::graph::{GraphChain, GraphNode, GraphSnapshot};
use lp_service::graph::{self, NodeDetail, Positions, ProvenanceView};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::review::CardBriefDto;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceDto {
    pub chat_id: String,
    pub message_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphNodeDto {
    pub id: String,
    /// Short name.
    pub name: String,
    /// The full proposition.
    pub text: String,
    /// Where it came from: `claude`, `codex`, `agy`, `git` or `chat`.
    pub kind: String,
    /// The session or chat that first produced it.
    pub session_id: Option<String>,
    pub provenance: Vec<ProvenanceDto>,
    pub created_at: i64,
    /// Chains touching the node.
    pub degree: usize,
}

impl GraphNodeDto {
    fn new(n: &GraphNode, degree: usize) -> Self {
        Self {
            id: n.id.to_string(),
            name: n.name.clone(),
            text: n.statement.clone(),
            kind: graph::kind_of(n).to_owned(),
            session_id: n.provenance.first().map(|p| p.chat_id.to_string()),
            provenance: n
                .provenance
                .iter()
                .map(|p| ProvenanceDto {
                    chat_id: p.chat_id.to_string(),
                    message_id: p.message_id.to_string(),
                })
                .collect(),
            created_at: n.created_at.timestamp_millis(),
            degree,
        }
    }
}

/// A reasoning chain: `premises` together imply `conclusion`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphChainDto {
    pub id: String,
    /// The first premise; with `to` this is the drawn edge.
    pub from: String,
    /// The conclusion.
    pub to: String,
    /// `implication` for one premise, `conjunction` for several.
    pub kind: String,
    pub premises: Vec<String>,
    pub conclusion: String,
    /// `A ∧ B ⟹ C` with node names.
    pub text: Option<String>,
}

impl GraphChainDto {
    fn new(c: &GraphChain, names: &dyn Fn(&str) -> String) -> Self {
        let premises: Vec<String> = c.premises.iter().map(ToString::to_string).collect();
        let text = format!(
            "{} ⟹ {}",
            premises
                .iter()
                .map(|p| names(p))
                .collect::<Vec<_>>()
                .join(" ∧ "),
            names(&c.conclusion)
        );
        Self {
            id: c.id.to_string(),
            from: premises.first().cloned().unwrap_or_default(),
            to: c.conclusion.to_string(),
            kind: if premises.len() > 1 {
                "conjunction"
            } else {
                "implication"
            }
            .into(),
            premises,
            conclusion: c.conclusion.to_string(),
            text: Some(text),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ClusterDto {
    pub id: String,
    pub label: String,
    pub node_ids: Vec<String>,
}

/// The whole graph, as one snapshot.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphDto {
    /// Content hash; changes whenever a node or chain does. Same as `graph.changed.version`.
    pub version: u64,
    pub nodes: Vec<GraphNodeDto>,
    pub chains: Vec<GraphChainDto>,
    pub clusters: Vec<ClusterDto>,
    /// Node id → `[x, y]`; `null` while the layout is computed.
    pub positions: Option<BTreeMap<String, [f64; 2]>>,
    /// Cached layout revision; advances when graph mutations or explicit relayout replace positions.
    pub layout_version: Option<u64>,
    /// The layout is being computed; a `graph.changed` event follows when it is ready.
    pub layout_pending: bool,
}

impl GraphDto {
    pub fn new(
        snap: &GraphSnapshot,
        version: u64,
        positions: Option<Positions>,
        labels: &HashMap<smol_str::SmolStr, String>,
        layout_pending: bool,
    ) -> Self {
        let deg = graph::degrees(snap);
        let names = |id: &str| {
            snap.nodes
                .get(id)
                .map_or_else(|| id.to_owned(), |n| n.name.clone())
        };
        let (layout_version, positions) = match positions {
            Some(p) => (Some(p.layout_version.max(p.version)), Some(p.positions)),
            None => (None, None),
        };
        Self {
            version,
            nodes: snap
                .nodes
                .values()
                .map(|n| GraphNodeDto::new(n, deg.get(n.id.as_str()).copied().unwrap_or(0)))
                .collect(),
            chains: snap
                .chains
                .iter()
                .map(|c| GraphChainDto::new(c, &names))
                .collect(),
            clusters: snap
                .birth_session_partition()
                .into_iter()
                .map(|c| ClusterDto {
                    label: labels
                        .get(&c.chat_id)
                        .cloned()
                        .unwrap_or_else(|| c.chat_id.to_string()),
                    id: c.chat_id.to_string(),
                    node_ids: c.nodes.iter().map(ToString::to_string).collect(),
                })
                .collect(),
            layout_pending: layout_pending && positions.is_none(),
            positions,
            layout_version,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodeBriefDto {
    pub id: String,
    pub name: String,
    pub text: String,
    pub kind: String,
}

impl From<&GraphNode> for NodeBriefDto {
    fn from(n: &GraphNode) -> Self {
        Self {
            id: n.id.to_string(),
            name: n.name.clone(),
            text: n.statement.clone(),
            kind: graph::kind_of(n).to_owned(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProvenanceDetailDto {
    pub chat_id: String,
    pub message_id: String,
    /// Position in the chat transcript, when the message is still there.
    pub message_index: Option<usize>,
    pub chat_title: Option<String>,
    pub snippet: String,
}

impl From<ProvenanceView> for ProvenanceDetailDto {
    fn from(p: ProvenanceView) -> Self {
        Self {
            chat_id: p.chat_id,
            message_id: p.message_id,
            message_index: p.message_index,
            chat_title: p.chat_title,
            snippet: p.snippet,
        }
    }
}

/// One node with everything the drawer shows.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodeDetailDto {
    #[serde(flatten)]
    pub node: GraphNodeDto,
    pub aliases: Vec<String>,
    pub provenance_detail: Vec<ProvenanceDetailDto>,
    pub incoming: Vec<GraphChainDto>,
    pub outgoing: Vec<GraphChainDto>,
    pub related: Vec<NodeBriefDto>,
    /// Cards generated from the decisions this node was distilled from.
    pub cards: Vec<CardBriefDto>,
    /// Mean mastery of `cards`, `0..=1`.
    pub mastery: Option<f64>,
}

impl NodeDetailDto {
    pub fn new(d: NodeDetail, snap: &GraphSnapshot, now: i64) -> Self {
        let names = |id: &str| {
            snap.nodes
                .get(id)
                .map_or_else(|| id.to_owned(), |n| n.name.clone())
        };
        Self {
            node: GraphNodeDto::new(&d.node, d.degree),
            aliases: d.node.aliases.clone(),
            provenance_detail: d.provenance.into_iter().map(Into::into).collect(),
            incoming: d
                .incoming
                .iter()
                .map(|c| GraphChainDto::new(c, &names))
                .collect(),
            outgoing: d
                .outgoing
                .iter()
                .map(|c| GraphChainDto::new(c, &names))
                .collect(),
            related: d.related.iter().map(Into::into).collect(),
            cards: d
                .cards
                .iter()
                .map(|c| CardBriefDto::new(c, None, now))
                .collect(),
            mastery: d.mastery,
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct GraphSearchQuery {
    pub q: String,
    pub limit: Option<usize>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct GraphSearchDto {
    /// Best match first.
    pub items: Vec<NodeBriefDto>,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ChainsQuery {
    /// Comma-separated node ids.
    pub selection: Option<String>,
}

/// Chains touching the selection, grouped for the 3-level chain table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChainGroupDto {
    /// Birth session of the chain's conclusion.
    pub session_id: String,
    pub label: String,
    pub chains: Vec<ChainRowDto>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChainRowDto {
    pub chain: GraphChainDto,
    /// Premises then conclusion, as propositions.
    pub nodes: Vec<NodeBriefDto>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChainTableDto {
    pub selection: Vec<String>,
    pub groups: Vec<ChainGroupDto>,
    pub total: usize,
}

impl ChainTableDto {
    pub fn new(
        snap: &GraphSnapshot,
        selection: Vec<String>,
        labels: &HashMap<smol_str::SmolStr, String>,
    ) -> Self {
        let ids: Vec<smol_str::SmolStr> = selection.iter().map(|s| s.as_str().into()).collect();
        let names = |id: &str| {
            snap.nodes
                .get(id)
                .map_or_else(|| id.to_owned(), |n| n.name.clone())
        };
        let mut groups: BTreeMap<String, Vec<ChainRowDto>> = BTreeMap::new();
        let mut total = 0;
        for c in snap.chains_for_selection(&ids) {
            total += 1;
            let session = snap
                .nodes
                .get(&c.conclusion)
                .and_then(|n| n.provenance.first())
                .map(|p| p.chat_id.to_string())
                .unwrap_or_default();
            let nodes = c
                .premises
                .iter()
                .chain(std::iter::once(&c.conclusion))
                .filter_map(|id| snap.nodes.get(id))
                .map(NodeBriefDto::from)
                .collect();
            groups.entry(session).or_default().push(ChainRowDto {
                chain: GraphChainDto::new(c, &names),
                nodes,
            });
        }
        Self {
            selection,
            groups: groups
                .into_iter()
                .map(|(session_id, chains)| ChainGroupDto {
                    label: labels
                        .get(session_id.as_str())
                        .cloned()
                        .unwrap_or_else(|| session_id.clone()),
                    session_id,
                    chains,
                })
                .collect(),
            total,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_core::graph::Provenance;

    fn node(id: &str, chat: &str) -> GraphNode {
        GraphNode {
            id: id.into(),
            name: id.to_uppercase(),
            statement: format!("{id} holds"),
            aliases: vec![],
            created_at: chrono::DateTime::UNIX_EPOCH,
            provenance: vec![Provenance {
                chat_id: chat.into(),
                message_id: "m0".into(),
            }],
        }
    }

    fn chain(id: &str, p: &[&str], c: &str) -> GraphChain {
        GraphChain {
            id: id.into(),
            premises: p.iter().map(|s| (*s).into()).collect(),
            conclusion: c.into(),
            provenance: vec![],
            created_at: chrono::DateTime::UNIX_EPOCH,
        }
    }

    fn snap() -> GraphSnapshot {
        let nodes: Vec<GraphNode> = ["a", "b", "c", "d"]
            .iter()
            .map(|i| node(i, "claude:s1"))
            .collect();
        GraphSnapshot::from_ledgers(
            &nodes,
            &[chain("x", &["a"], "b"), chain("y", &["a", "b"], "c")],
        )
    }

    #[test]
    fn graph_dto_derives_degree_kind_chain_text_and_clusters() {
        let s = snap();
        let labels = HashMap::from([(smol_str::SmolStr::from("claude:s1"), "Session".to_owned())]);
        let g = GraphDto::new(&s, 7, None, &labels, true);
        assert!(g.layout_pending && g.positions.is_none() && g.layout_version.is_none());
        let a = &g.nodes[0];
        assert_eq!(
            (a.id.as_str(), a.degree, a.kind.as_str()),
            ("a", 2, "claude")
        );
        assert_eq!(a.session_id.as_deref(), Some("claude:s1"));
        assert_eq!(g.chains[0].kind, "implication");
        assert_eq!(g.chains[1].kind, "conjunction");
        assert_eq!(g.chains[1].text.as_deref(), Some("A ∧ B ⟹ C"));
        assert_eq!(
            (g.chains[1].from.as_str(), g.chains[1].to.as_str()),
            ("a", "c")
        );
        assert_eq!(g.clusters.len(), 1);
        assert_eq!(g.clusters[0].label, "Session");

        let pos = Positions {
            layout_version: 7,
            version: 7,
            positions: BTreeMap::from([("a".to_owned(), [1.0, 2.0])]),
        };
        let g = GraphDto::new(&s, 7, Some(pos), &labels, true);
        assert!(!g.layout_pending, "ready positions end the pending state");
        assert_eq!(g.layout_version, Some(7));
        let j = serde_json::to_value(&g).unwrap();
        assert_eq!(j["positions"]["a"][1], 2.0);
        assert_eq!(j["nodes"][0]["createdAt"], 0);
        assert_eq!(serde_json::from_value::<GraphDto>(j).unwrap(), g);
    }

    #[test]
    fn chain_table_groups_by_conclusion_session() {
        let s = snap();
        let labels = HashMap::new();
        let t = ChainTableDto::new(&s, vec!["b".into()], &labels);
        assert_eq!(t.total, 2);
        assert_eq!(t.groups.len(), 1);
        assert_eq!(t.groups[0].label, "claude:s1");
        let rows: Vec<_> = t.groups[0]
            .chains
            .iter()
            .map(|r| r.chain.id.as_str())
            .collect();
        assert_eq!(rows, ["x", "y"]);
        assert_eq!(t.groups[0].chains[1].nodes.len(), 3);
        assert_eq!(ChainTableDto::new(&s, vec![], &labels).total, 0);
    }
}
