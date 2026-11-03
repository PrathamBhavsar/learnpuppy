//! The stores of one workspace, all opened under that workspace's root directory.

use std::path::Path;
use std::sync::Arc;

use lp_store::{FsChatStore, FsCourseStore, FsGraphStore, FsJobStore, SqliteTrackerStore};

pub struct Stores {
    pub graph: Arc<FsGraphStore>,
    pub chats: Arc<FsChatStore>,
    pub tracker: Arc<SqliteTrackerStore>,
    pub jobs: Arc<FsJobStore>,
    pub courses: Arc<FsCourseStore>,
}

impl Stores {
    /// Opens (creating if needed) `graph/`, `chats/`, `tracker/` and `jobs/` under `root`.
    pub fn open(root: &Path) -> anyhow::Result<Self> {
        Ok(Self {
            graph: Arc::new(FsGraphStore::open(root.join("graph"))?),
            chats: Arc::new(FsChatStore::open(root.join("chats"))?),
            tracker: Arc::new(SqliteTrackerStore::open(root.join("tracker"))?),
            jobs: Arc::new(FsJobStore::open(root.join("jobs"))?),
            courses: Arc::new(FsCourseStore::new(root)),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::paths::workspace_dir;
    use crate::review::tests::card;
    use lp_core::graph::GraphNode;
    use lp_core::ports::{GraphStore, TrackerStore};

    fn node(id: &str) -> GraphNode {
        GraphNode {
            id: id.into(),
            name: id.into(),
            statement: id.into(),
            aliases: vec![],
            created_at: chrono::DateTime::UNIX_EPOCH,
            provenance: vec![],
        }
    }

    #[test]
    fn two_workspace_roots_never_share_data() {
        let d = tempfile::tempdir().unwrap();
        let (ra, rb) = (
            workspace_dir(d.path(), "ws_aaaaaaaa"),
            workspace_dir(d.path(), "ws_bbbbbbbb"),
        );
        let (a, b) = (Stores::open(&ra).unwrap(), Stores::open(&rb).unwrap());
        a.graph.append_node(&node("n_a")).unwrap();
        a.tracker.upsert_card(&card("c1", 0)).unwrap();
        assert_eq!(a.graph.nodes().unwrap().len(), 1);
        assert!(b.graph.nodes().unwrap().is_empty());
        assert!(b.tracker.cards().unwrap().is_empty());
        for sub in ["graph", "chats", "tracker", "jobs"] {
            assert!(ra.join(sub).is_dir(), "{sub}");
        }
        // Nothing lands at the data root itself.
        assert!(!d.path().join("graph").exists());
        // Reopening sees the same data.
        assert_eq!(Stores::open(&ra).unwrap().graph.nodes().unwrap().len(), 1);
    }
}
