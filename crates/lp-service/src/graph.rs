//! Knowledge graph reads and edits over a workspace's ledgers.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::SystemTime;

use lp_core::graph::{GraphChain, GraphNode, GraphSnapshot, NodeId};
use lp_core::ports::{ChatStore, FlashCard, GraphStore, StoreError, StoreResult, TrackerStore};
use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use smol_str::SmolStr;

/// The merged graph as of the ledgers on disk.
pub fn snapshot(g: &dyn GraphStore) -> StoreResult<GraphSnapshot> {
    Ok(GraphSnapshot::from_ledgers(&g.nodes()?, &g.chains()?))
}

/// The chains a deletion of `ids` would also remove.
pub fn delete_preview(g: &dyn GraphStore, ids: &[NodeId]) -> StoreResult<Vec<SmolStr>> {
    Ok(snapshot(g)?.deletion_blast_radius(ids))
}

/// Deletes `ids` and every chain that touches them; returns the removed chain ids.
/// Any read failure aborts before a write, so the chain ledger is never rewritten from a partial read.
pub fn delete_nodes(g: &dyn GraphStore, ids: &[NodeId]) -> StoreResult<Vec<SmolStr>> {
    let chains = g.chains()?;
    let doomed = GraphSnapshot::from_ledgers(&g.nodes()?, &chains).deletion_blast_radius(ids);
    let keep: Vec<_> = chains
        .into_iter()
        .filter(|c| !doomed.contains(&c.id))
        .collect();
    g.rewrite_chains(&keep)?;
    g.delete_nodes(ids)?;
    Ok(doomed)
}

/// The opening learner message of a practice session over the chains `chain_ids`.
pub fn practice_prompt(snap: &GraphSnapshot, chain_ids: &[SmolStr]) -> String {
    let name = |id: &str| snap.nodes.get(id).map_or(id.to_owned(), |n| n.name.clone());
    let lines: Vec<String> = snap
        .chains
        .iter()
        .filter(|c| chain_ids.contains(&c.id))
        .map(|c| {
            let premises: Vec<String> = c.premises.iter().map(|p| name(p)).collect();
            format!("- {} ⟹ {}", premises.join(" ∧ "), name(&c.conclusion))
        })
        .collect();
    format!("Practice these reasoning chains:\n{}", lines.join("\n"))
}

/// ForceAtlas2 iterations for the cached layout.
pub const LAYOUT_ITERATIONS: usize = 120;

/// A merged graph with the content hash the API uses as its version and ETag.
#[derive(Debug)]
pub struct Loaded {
    pub snap: GraphSnapshot,
    pub version: u64,
}

impl Loaded {
    pub fn new(snap: GraphSnapshot) -> Self {
        let version = version(&snap);
        Self { snap, version }
    }
}

/// A hash of everything the graph API shows: changes whenever a node, alias, provenance or chain
/// does. Stable across runs and machines.
pub fn version(snap: &GraphSnapshot) -> u64 {
    let mut h = Sha256::new();
    for n in snap.nodes.values() {
        h.update(n.id.as_bytes());
        h.update([0]);
        h.update(n.name.as_bytes());
        h.update([0]);
        h.update(n.statement.as_bytes());
        for a in &n.aliases {
            h.update([1]);
            h.update(a.as_bytes());
        }
        for p in &n.provenance {
            h.update([2]);
            h.update(p.chat_id.as_bytes());
            h.update([0]);
            h.update(p.message_id.as_bytes());
        }
        h.update(n.created_at.timestamp_millis().to_le_bytes());
    }
    h.update([3]);
    let mut chains: Vec<&GraphChain> = snap.chains.iter().collect();
    chains.sort_by(|a, b| a.id.cmp(&b.id));
    for c in chains {
        h.update(c.id.as_bytes());
        for p in &c.premises {
            h.update([4]);
            h.update(p.as_bytes());
        }
        h.update([5]);
        h.update(c.conclusion.as_bytes());
    }
    let d = h.finalize();
    // 53 bits so the number survives a JSON round trip through JavaScript.
    u64::from_le_bytes([d[0], d[1], d[2], d[3], d[4], d[5], d[6] & 0x1f, 0])
}

/// The tool a node came from: the prefix of its birth session id for ingested sessions
/// (`claude:…`, `codex:…`, `agy:…`, `git:…`), else `chat`.
pub fn kind_of(n: &GraphNode) -> &'static str {
    let Some(p) = n.provenance.first() else {
        return "chat";
    };
    match p.chat_id.split_once(':').map(|(src, _)| src) {
        Some("claude") => "claude",
        Some("codex") => "codex",
        Some("agy") => "agy",
        Some("git") => "git",
        _ => "chat",
    }
}

/// Chains touching each node, as premise or conclusion.
pub fn degrees(snap: &GraphSnapshot) -> HashMap<&str, usize> {
    let mut deg: HashMap<&str, usize> = HashMap::new();
    for c in &snap.chains {
        for id in c.premises.iter().chain(std::iter::once(&c.conclusion)) {
            *deg.entry(id.as_str()).or_default() += 1;
        }
    }
    deg
}

/// File stamp of the two ledgers; changes whenever either is appended to or rewritten.
type Stamp = [Option<(SystemTime, u64)>; 2];

fn stamp(dir: &Path) -> Stamp {
    ["nodes.jsonl", "chains.jsonl"].map(|f| {
        let m = std::fs::metadata(dir.join(f)).ok()?;
        Some((m.modified().ok()?, m.len()))
    })
}

/// Merged snapshots by graph directory, reloaded only when a ledger file changed. Graph reads
/// are the hottest endpoint and the ledgers are re-read in full otherwise.
#[derive(Default)]
pub struct SnapshotCache {
    entries: Mutex<HashMap<PathBuf, (Stamp, Arc<Loaded>)>>,
}

impl SnapshotCache {
    /// The merged graph of `g`, whose ledgers live in `dir`.
    pub fn get(&self, dir: &Path, g: &dyn GraphStore) -> StoreResult<Arc<Loaded>> {
        let now = stamp(dir);
        if let Some((s, l)) = self.entries.lock().get(dir) {
            if *s == now {
                return Ok(l.clone());
            }
        }
        let loaded = Arc::new(Loaded::new(snapshot(g)?));
        self.entries
            .lock()
            .insert(dir.to_owned(), (now, loaded.clone()));
        Ok(loaded)
    }

    pub fn forget(&self, dir: &Path) {
        self.entries.lock().remove(dir);
    }
}

/// `graph/positions.json`: node positions for one graph version.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Positions {
    pub version: u64,
    pub positions: BTreeMap<String, [f64; 2]>,
}

pub const POSITIONS_FILE: &str = "positions.json";

/// The cached layout, if it is readable and was computed for `version`.
pub fn read_positions(dir: &Path, version: u64) -> Option<Positions> {
    let bytes = std::fs::read(dir.join(POSITIONS_FILE)).ok()?;
    serde_json::from_slice::<Positions>(&bytes)
        .ok()
        .filter(|p| p.version == version)
}

/// Lays the graph out (ForceAtlas2, deterministic) and rounds to 2 decimals.
pub fn compute_positions(l: &Loaded) -> Positions {
    let positions = l
        .snap
        .layout(LAYOUT_ITERATIONS)
        .into_iter()
        .map(|(id, [x, y])| (id.to_string(), [round2(x), round2(y)]))
        .collect();
    Positions {
        version: l.version,
        positions,
    }
}

fn round2(v: f64) -> f64 {
    (v * 100.0).round() / 100.0
}

/// Atomic write of `positions.json`.
pub fn write_positions(dir: &Path, p: &Positions) -> anyhow::Result<()> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(POSITIONS_FILE);
    let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
    std::fs::write(&tmp, serde_json::to_vec(p)?)?;
    std::fs::rename(&tmp, &path)?;
    Ok(())
}

/// Birth-session groups with a display label: the chat title when the session is a chat, else
/// the session id.
pub fn cluster_labels(chats: &dyn ChatStore, sessions: &[SmolStr]) -> HashMap<SmolStr, String> {
    let titles: HashMap<SmolStr, String> = chats
        .list_chats()
        .unwrap_or_default()
        .into_iter()
        .map(|c| (c.id, c.title))
        .collect();
    sessions
        .iter()
        .map(|s| {
            let label = titles.get(s).cloned().unwrap_or_else(|| s.to_string());
            (s.clone(), label)
        })
        .collect()
}

/// Where a node came from, with the text it came from when that can still be found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProvenanceView {
    pub chat_id: String,
    pub message_id: String,
    /// Position in the chat transcript when `message_id` is an index.
    pub message_index: Option<usize>,
    pub chat_title: Option<String>,
    pub snippet: String,
}

/// Everything the node drawer shows.
#[derive(Clone, Debug)]
pub struct NodeDetail {
    pub node: GraphNode,
    pub degree: usize,
    pub incoming: Vec<GraphChain>,
    pub outgoing: Vec<GraphChain>,
    pub related: Vec<GraphNode>,
    pub provenance: Vec<ProvenanceView>,
    pub cards: Vec<FlashCard>,
    /// Mean mastery of the linked cards.
    pub mastery: Option<f64>,
}

const SNIPPET_CHARS: usize = 240;

fn snippet(s: &str) -> String {
    let flat = s.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.chars().count() <= SNIPPET_CHARS {
        return flat;
    }
    let cut: String = flat.chars().take(SNIPPET_CHARS).collect();
    format!("{}…", cut.trim_end())
}

/// The text of message `index` (or of a decision `message_id`) behind one provenance entry.
fn provenance_view(
    p: &lp_core::graph::Provenance,
    chats: &dyn ChatStore,
    t: &dyn TrackerStore,
    decisions: &[lp_core::ports::DecisionRecord],
) -> ProvenanceView {
    let mut view = ProvenanceView {
        chat_id: p.chat_id.to_string(),
        message_id: p.message_id.to_string(),
        message_index: None,
        chat_title: None,
        snippet: String::new(),
    };
    if let Some(d) = decisions.iter().find(|d| d.id == p.message_id) {
        view.snippet = snippet(&format!("{}: {}", d.title, d.what));
        view.chat_title = t.session(&d.session_id).ok().flatten().map(|s| s.cwd);
        return view;
    }
    if let Ok(rec) = chats.load_chat(&p.chat_id) {
        view.chat_title = Some(rec.title);
        let index = p.message_id.trim_start_matches('m').parse::<usize>().ok();
        if let (Some(i), Some(msgs)) = (index, rec.body.get("messages").and_then(|m| m.as_array()))
        {
            view.message_index = Some(i).filter(|i| *i < msgs.len());
            view.snippet = msgs.get(i).map(message_text).unwrap_or_default();
        }
    }
    view
}

/// The human text of one persisted `Msg` (`{"User": "…"}`, `{"Assistant": {"text": …}}`, …).
fn message_text(m: &serde_json::Value) -> String {
    let Some(obj) = m.as_object() else {
        return String::new();
    };
    let Some((_, v)) = obj.iter().next() else {
        return String::new();
    };
    let text = v
        .as_str()
        .or_else(|| v.get("text").and_then(|t| t.as_str()))
        .unwrap_or_default();
    snippet(text)
}

/// The node drawer data for `id`; `None` when it is not in the graph.
pub fn node_detail(
    snap: &GraphSnapshot,
    id: &str,
    chats: &dyn ChatStore,
    t: &dyn TrackerStore,
    links: &crate::digest::CardLinks,
) -> StoreResult<Option<NodeDetail>> {
    let Some(node) = snap.nodes.get(id) else {
        return Ok(None);
    };
    let incoming: Vec<GraphChain> = snap
        .chains
        .iter()
        .filter(|c| c.conclusion == id)
        .cloned()
        .collect();
    let outgoing: Vec<GraphChain> = snap
        .chains
        .iter()
        .filter(|c| c.premises.iter().any(|p| p == id))
        .cloned()
        .collect();
    let mut related: Vec<GraphNode> = Vec::new();
    for c in incoming.iter().chain(&outgoing) {
        for other in c.premises.iter().chain(std::iter::once(&c.conclusion)) {
            if other != id && !related.iter().any(|r| r.id == *other) {
                if let Some(n) = snap.nodes.get(other) {
                    related.push(n.clone());
                }
            }
        }
    }
    let decisions = t.decisions()?;
    let provenance: Vec<ProvenanceView> = node
        .provenance
        .iter()
        .map(|p| provenance_view(p, chats, t, &decisions))
        .collect();
    let mut cards = Vec::new();
    for p in &node.provenance {
        for cid in links.cards_of(&p.message_id) {
            if let Some(c) = t.card(cid)? {
                if !cards.iter().any(|x: &FlashCard| x.id == c.id) {
                    cards.push(c);
                }
            }
        }
    }
    let mastery = (!cards.is_empty()).then(|| {
        cards
            .iter()
            .map(|c| crate::review::mastery(&c.card))
            .sum::<f64>()
            / cards.len() as f64
    });
    Ok(Some(NodeDetail {
        degree: incoming.len() + outgoing.len(),
        node: node.clone(),
        incoming,
        outgoing,
        related,
        provenance,
        cards,
        mastery,
    }))
}

/// Error for an unknown node id.
pub fn not_found(id: &str) -> StoreError {
    StoreError::NotFound(id.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_core::graph::{GraphChain, GraphNode};
    use lp_store::FsGraphStore;

    fn seeded() -> (tempfile::TempDir, FsGraphStore) {
        let d = tempfile::tempdir().unwrap();
        let g = FsGraphStore::open(d.path()).unwrap();
        for (id, name) in [("n_a", "Alpha"), ("n_b", "Beta"), ("n_c", "Gamma")] {
            g.append_node(&GraphNode {
                id: id.into(),
                name: name.into(),
                statement: id.into(),
                aliases: vec![],
                created_at: chrono::DateTime::UNIX_EPOCH,
                provenance: vec![],
            })
            .unwrap();
        }
        let chain = |id: &str, p: &str, c: &str| GraphChain {
            id: id.into(),
            premises: vec![p.into()],
            conclusion: c.into(),
            provenance: vec![],
            created_at: chrono::DateTime::UNIX_EPOCH,
        };
        g.append_chain(&chain("ch1", "n_a", "n_b")).unwrap();
        g.append_chain(&chain("ch2", "n_b", "n_c")).unwrap();
        (d, g)
    }

    #[test]
    fn delete_removes_nodes_and_touching_chains() {
        let (_d, g) = seeded();
        let ids = [SmolStr::from("n_a")];
        assert_eq!(delete_preview(&g, &ids).unwrap(), ["ch1"]);
        assert_eq!(delete_nodes(&g, &ids).unwrap(), ["ch1"]);
        let nodes: Vec<_> = g.nodes().unwrap().into_iter().map(|n| n.id).collect();
        assert_eq!(nodes, ["n_b", "n_c"]);
        let chains: Vec<_> = g.chains().unwrap().into_iter().map(|c| c.id).collect();
        assert_eq!(chains, ["ch2"]);
        assert_eq!(snapshot(&g).unwrap().nodes.len(), 2);
    }

    #[test]
    fn practice_prompt_names_the_selected_chains() {
        let (_d, g) = seeded();
        let p = practice_prompt(&snapshot(&g).unwrap(), &["ch2".into()]);
        assert_eq!(p, "Practice these reasoning chains:\n- Beta ⟹ Gamma");
    }

    use lp_core::graph::Provenance;
    use lp_core::ports::{ChatRecord, DecisionBasis, DecisionRecord};

    fn with_prov(g: &FsGraphStore, id: &str, chat: &str, msg: &str) {
        g.append_node(&GraphNode {
            id: id.into(),
            name: id.into(),
            statement: format!("{id} statement"),
            aliases: vec![],
            created_at: chrono::DateTime::UNIX_EPOCH,
            provenance: vec![Provenance {
                chat_id: chat.into(),
                message_id: msg.into(),
            }],
        })
        .unwrap();
    }

    #[test]
    fn version_tracks_content_and_cache_follows_the_ledgers() {
        let (d, g) = seeded();
        let cache = SnapshotCache::default();
        let a = cache.get(d.path(), &g).unwrap();
        assert!(a.version < 1 << 53);
        assert!(Arc::ptr_eq(&a, &cache.get(d.path(), &g).unwrap()), "cached");
        assert_eq!(a.version, version(&snapshot(&g).unwrap()));

        with_prov(&g, "n_d", "claude:s1", "m0");
        let b = cache.get(d.path(), &g).unwrap();
        assert_eq!(b.snap.nodes.len(), 4);
        assert_ne!(a.version, b.version);
        delete_nodes(&g, &["n_d".into()]).unwrap();
        let c = cache.get(d.path(), &g).unwrap();
        assert_eq!(c.version, a.version, "same content, same version");
        cache.forget(d.path());
    }

    #[test]
    fn kind_degrees_and_positions_cache() {
        let (d, g) = seeded();
        with_prov(&g, "n_x", "codex:abc", "d1");
        with_prov(&g, "n_y", "chat7", "m1");
        let l = Loaded::new(snapshot(&g).unwrap());
        assert_eq!(kind_of(&l.snap.nodes["n_x"]), "codex");
        assert_eq!(kind_of(&l.snap.nodes["n_y"]), "chat");
        assert_eq!(kind_of(&l.snap.nodes["n_a"]), "chat");
        let deg = degrees(&l.snap);
        assert_eq!((deg["n_a"], deg["n_b"], deg["n_c"]), (1, 2, 1));
        assert!(!deg.contains_key("n_x"));

        let dir = d.path().join("cache");
        assert!(read_positions(&dir, l.version).is_none());
        let p = compute_positions(&l);
        assert_eq!(p, compute_positions(&l), "layout is deterministic");
        assert_eq!(p.positions.len(), 5);
        write_positions(&dir, &p).unwrap();
        assert_eq!(read_positions(&dir, l.version), Some(p));
        assert!(
            read_positions(&dir, l.version + 1).is_none(),
            "stale version"
        );
    }

    #[test]
    fn node_detail_gathers_chains_provenance_and_linked_cards() {
        let (d, g) = seeded();
        let stores = crate::stores::Stores::open(&d.path().join("ws")).unwrap();
        with_prov(&g, "n_p", "chat1", "m1");
        with_prov(&g, "n_q", "claude:s9", "dec1");
        let rec = ChatRecord {
            id: "chat1".into(),
            title: "Ownership".into(),
            created_at: 0,
            updated_at: 0,
            body: serde_json::json!({"messages": [{"User": "hi"}, {"Assistant": {"text": "Values   have\none owner.", "calls": []}}]}),
            turns: vec![],
            extractions: vec![],
        };
        stores.chats.save_chat(&rec).unwrap();
        stores
            .tracker
            .upsert_decision(&DecisionRecord {
                id: "dec1".into(),
                session_id: "claude:s9".into(),
                title: "Snapshot copy".into(),
                what: "copy the db".into(),
                why: "y".into(),
                basis: DecisionBasis::Stated,
                alternatives: vec![],
                risk: String::new(),
                files: vec![],
                commit_sha: None,
            })
            .unwrap();
        stores
            .tracker
            .upsert_card(&crate::review::tests::card("c1", 0))
            .unwrap();
        let mut links = crate::digest::CardLinks::default();
        links.link("dec1", ["c1".to_owned(), "gone".to_owned()]);

        let snap = snapshot(&g).unwrap();
        let det = node_detail(
            &snap,
            "n_b",
            stores.chats.as_ref(),
            stores.tracker.as_ref(),
            &links,
        )
        .unwrap()
        .unwrap();
        assert_eq!(det.degree, 2);
        assert_eq!(det.incoming[0].id, "ch1");
        assert_eq!(det.outgoing[0].id, "ch2");
        let rel: Vec<_> = det.related.iter().map(|n| n.id.as_str()).collect();
        assert_eq!(rel, ["n_a", "n_c"]);

        let p = node_detail(
            &snap,
            "n_p",
            stores.chats.as_ref(),
            stores.tracker.as_ref(),
            &links,
        )
        .unwrap()
        .unwrap();
        let pv = &p.provenance[0];
        assert_eq!(pv.chat_title.as_deref(), Some("Ownership"));
        assert_eq!(pv.message_index, Some(1));
        assert_eq!(pv.snippet, "Values have one owner.");

        let q = node_detail(
            &snap,
            "n_q",
            stores.chats.as_ref(),
            stores.tracker.as_ref(),
            &links,
        )
        .unwrap()
        .unwrap();
        assert_eq!(q.provenance[0].snippet, "Snapshot copy: copy the db");
        assert_eq!(q.cards.len(), 1, "dangling card ids are skipped");
        assert!(q.mastery.is_some());
        assert!(node_detail(
            &snap,
            "nope",
            stores.chats.as_ref(),
            stores.tracker.as_ref(),
            &links
        )
        .unwrap()
        .is_none());
    }
}
