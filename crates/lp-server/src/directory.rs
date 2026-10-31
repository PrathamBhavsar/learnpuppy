//! What every handler needs besides `AppState`'s own fields: the workspace registry
//! (`lp_service::workspace`), the server clock, caches, and the bridge from registry events to
//! the SSE hub. `/bootstrap` reads workspaces through [`WorkspaceDirectory`].

use std::collections::{HashSet, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::Arc;

use lp_service::events::{Event, EventHub, Scope};
use lp_service::graph::{self, Loaded, SnapshotCache};
use lp_service::paths;
use lp_service::workspace::migrate::{self, MarkerState, MigrationOutcome};
use lp_service::workspace::{
    EventSink, ServerLock, WorkspaceEvent, WorkspaceHandle, WorkspaceMeta, WorkspaceRegistry,
};
use parking_lot::Mutex;
use serde::Deserialize;

/// One workspace as the switcher lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkspaceSummary {
    pub id: String,
    pub name: String,
    pub slug: String,
    pub icon: String,
    pub hue: String,
    pub archived: bool,
    pub last_opened_at: Option<i64>,
}

impl From<WorkspaceMeta> for WorkspaceSummary {
    fn from(m: WorkspaceMeta) -> Self {
        Self {
            id: m.id,
            name: m.name,
            slug: m.slug,
            icon: m.icon,
            hue: m.hue,
            archived: m.archived,
            last_opened_at: Some(m.last_opened_at),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BannerLevel {
    Info,
    Warning,
}

/// A one-time notice shown at app start (e.g. "your data moved into the Default workspace").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Banner {
    pub id: String,
    pub level: BannerLevel,
    pub message: String,
}

/// Workspace listing for bootstrap plus access to the shared [`Core`]. Methods block
/// (filesystem); call from `spawn_blocking`.
pub trait WorkspaceDirectory: Send + Sync {
    /// Every healthy workspace, archived included, most recently opened first.
    fn list(&self) -> anyhow::Result<Vec<WorkspaceSummary>>;
    /// The workspace the user had open last, if it still exists.
    fn last_workspace(&self) -> anyhow::Result<Option<String>>;
    fn banners(&self) -> anyhow::Result<Vec<Banner>>;
    fn core(&self) -> &Arc<Core>;
}

/// How long the "your data moved" banner stays after a migration.
const MIGRATION_BANNER_MS: i64 = 7 * 24 * 3_600_000;
const MIGRATION_BANNER_ID: &str = "migrated-v1";

/// Server-authoritative time (docs/web/00 D20). Real time unless a test pinned it.
#[derive(Debug)]
pub struct Clock {
    fixed: AtomicI64,
}

const UNPINNED: i64 = i64::MIN;

impl Default for Clock {
    fn default() -> Self {
        Self {
            fixed: AtomicI64::new(UNPINNED),
        }
    }
}

impl Clock {
    pub fn now(&self) -> i64 {
        match self.fixed.load(Ordering::Acquire) {
            UNPINNED => lp_service::now_ms(),
            ms => ms,
        }
    }

    /// Pin the clock to `ms`, or release it. Only test builds can reach this.
    #[cfg(feature = "test-hooks")]
    pub fn pin(&self, ms: Option<i64>) {
        self.fixed.store(ms.unwrap_or(UNPINNED), Ordering::Release);
    }
}

/// Replies to retried mutations, by `clientId` (docs/web/03 §Conventions). Bounded; the oldest
/// entries go first.
pub struct Replies {
    cap: usize,
    inner: Mutex<(
        std::collections::HashMap<String, serde_json::Value>,
        VecDeque<String>,
    )>,
}

impl Replies {
    pub fn new(cap: usize) -> Self {
        Self {
            cap,
            inner: Mutex::new(Default::default()),
        }
    }

    pub fn get(&self, key: &str) -> Option<serde_json::Value> {
        self.inner.lock().0.get(key).cloned()
    }

    pub fn put(&self, key: String, reply: serde_json::Value) {
        let mut g = self.inner.lock();
        if g.0.insert(key.clone(), reply).is_none() {
            g.1.push_back(key);
        }
        while g.1.len() > self.cap {
            if let Some(old) = g.1.pop_front() {
                g.0.remove(&old);
            }
        }
    }
}

/// Shared server services behind the HTTP handlers.
pub struct Core {
    pub registry: Arc<WorkspaceRegistry>,
    pub clock: Clock,
    pub graphs: SnapshotCache,
    pub replies: Replies,
    pub hub: Arc<EventHub>,
    /// Home for CLI account discovery (`~/.claude`, `~/.codex`, …).
    pub home: PathBuf,
    pub claude_config_dir: Option<PathBuf>,
    /// Serialises read-modify-write of the global `settings.json`.
    pub settings_lock: Mutex<()>,
    /// Serialises grading so a card is never replayed from a half-written log.
    pub review_lock: Mutex<()>,
    layouts: Mutex<HashSet<String>>,
    startup_banners: Mutex<Vec<Banner>>,
}

/// Registry events → SSE events.
struct HubSink(Arc<EventHub>);

impl EventSink for HubSink {
    fn emit(&self, e: WorkspaceEvent) {
        let str_field = |k: &str| {
            e.data
                .get(k)
                .and_then(|v| v.as_str())
                .map(str::to_owned)
                .unwrap_or_default()
        };
        let event = match e.kind.as_str() {
            "workspace.created" => Event::WorkspaceCreated {
                workspace_id: str_field("id"),
            },
            "workspace.updated" => Event::WorkspaceUpdated {
                workspace_id: str_field("id"),
            },
            "workspace.deleted" => Event::WorkspaceDeleted {
                workspace_id: str_field("id"),
            },
            "graph.changed" => Event::GraphChanged {
                version: e.data.get("version").and_then(|v| v.as_u64()).unwrap_or(0),
            },
            "chat.created" => Event::ChatCreated {
                chat_id: str_field("chatId"),
            },
            "chat.updated" => Event::ChatUpdated {
                chat_id: str_field("chatId"),
            },
            "review.changed" => Event::ReviewChanged,
            other => {
                tracing::debug!(kind = other, "workspace event has no SSE mapping");
                return;
            }
        };
        let scope = e.workspace.map_or(Scope::Global, Scope::Workspace);
        self.0.publish(scope, event);
    }
}

impl Core {
    /// Opens the registry over `root`, wired to `hub`.
    pub fn open(
        root: PathBuf,
        hub: Arc<EventHub>,
        home: PathBuf,
        claude_config_dir: Option<PathBuf>,
    ) -> anyhow::Result<Arc<Self>> {
        let registry = WorkspaceRegistry::new(root, Arc::new(HubSink(hub.clone())))?;
        Ok(Arc::new(Self {
            registry: Arc::new(registry),
            clock: Clock::default(),
            graphs: SnapshotCache::default(),
            replies: Replies::new(2048),
            hub,
            home,
            claude_config_dir,
            settings_lock: Mutex::new(()),
            review_lock: Mutex::new(()),
            layouts: Mutex::new(HashSet::new()),
            startup_banners: Mutex::new(Vec::new()),
        }))
    }

    pub fn root(&self) -> &std::path::Path {
        self.registry.root()
    }

    pub fn push_banner(&self, b: Banner) {
        self.startup_banners.lock().push(b);
    }

    /// Computes and caches the layout of `loaded` in the background unless that is already
    /// running for this workspace. Emits `graph.changed` when the positions are on disk.
    pub fn start_layout(self: &Arc<Self>, handle: &Arc<WorkspaceHandle>, loaded: Arc<Loaded>) {
        if !self.layouts.lock().insert(handle.id().to_owned()) {
            return;
        }
        let (core, handle) = (self.clone(), handle.clone());
        std::mem::drop(tokio::task::spawn_blocking(move || {
            let dir = handle.root().join("graph");
            let positions = graph::compute_positions(&loaded);
            match graph::write_positions(&dir, &positions) {
                Ok(()) => handle.emit(
                    "graph.changed",
                    serde_json::json!({ "version": loaded.version }),
                ),
                Err(e) => {
                    tracing::warn!(workspace = handle.id(), "could not cache the layout: {e:#}")
                }
            }
            core.layouts.lock().remove(handle.id());
        }));
    }

    /// Whether a layout for `ws` is being computed.
    pub fn layout_running(&self, ws: &str) -> bool {
        self.layouts.lock().contains(ws)
    }

    /// Moves a legacy single-store data root into the "Default" workspace (docs/web/02
    /// §Migration). A failure is kept as a warning banner rather than stopping the server.
    /// Blocking.
    pub fn migrate_on_start(&self) {
        let root = self.root().to_owned();
        let outcome =
            ServerLock::acquire(&root, None, None).and_then(|lock| migrate::migrate(&root, &lock));
        match outcome {
            Ok(MigrationOutcome::Migrated { workspace_id, .. }) => {
                tracing::info!(workspace = workspace_id, "migrated the legacy store");
            }
            Ok(_) => {}
            Err(e) => {
                tracing::error!("legacy migration failed: {e}");
                self.push_banner(Banner {
                    id: "migration-failed".into(),
                    level: BannerLevel::Warning,
                    message: format!("Moving your existing data into a workspace failed: {e}"),
                });
            }
        }
    }
}

/// [`WorkspaceDirectory`] over the real registry.
pub struct RegistryDirectory {
    core: Arc<Core>,
}

impl RegistryDirectory {
    pub fn new(core: Arc<Core>) -> Self {
        Self { core }
    }

    /// Opens everything under `root` (tests and the CLI).
    pub fn open(
        root: PathBuf,
        hub: Arc<EventHub>,
        home: PathBuf,
        claude_config_dir: Option<PathBuf>,
    ) -> anyhow::Result<Self> {
        Ok(Self::new(Core::open(root, hub, home, claude_config_dir)?))
    }
}

#[derive(Deserialize, Default)]
#[serde(default)]
struct StateFile {
    last_workspace: Option<String>,
}

impl WorkspaceDirectory for RegistryDirectory {
    fn list(&self) -> anyhow::Result<Vec<WorkspaceSummary>> {
        let list = self.core.registry.list(true)?;
        Ok(list
            .workspaces
            .into_iter()
            .map(|w| WorkspaceSummary::from(w.meta))
            .collect())
    }

    fn last_workspace(&self) -> anyhow::Result<Option<String>> {
        let root = self.core.root();
        let state: StateFile = match std::fs::read(paths::state_json(root)) {
            Ok(b) => serde_json::from_slice(&b).unwrap_or_default(),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => StateFile::default(),
            Err(e) => return Err(e.into()),
        };
        Ok(state
            .last_workspace
            .filter(|id| self.core.registry.get(id).is_ok()))
    }

    fn banners(&self) -> anyhow::Result<Vec<Banner>> {
        let mut out = self.core.startup_banners.lock().clone();
        let root = self.core.root();
        if let Some(m) = migrate::marker(root)? {
            let at = m.finished_at.unwrap_or(m.started_at);
            if m.state == MarkerState::Done && self.core.clock.now() - at < MIGRATION_BANNER_MS {
                let name = self
                    .core
                    .registry
                    .get(&m.workspace_id)
                    .map_or_else(|_| migrate::DEFAULT_NAME.to_owned(), |w| w.name);
                out.push(Banner {
                    id: MIGRATION_BANNER_ID.into(),
                    level: BannerLevel::Info,
                    message: format!(
                        "Your existing data moved into the \"{name}\" workspace. Nothing was copied or lost."
                    ),
                });
            }
        }
        Ok(out)
    }

    fn core(&self) -> &Arc<Core> {
        &self.core
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use lp_service::workspace::NewWorkspace;

    pub(crate) fn directory(root: &std::path::Path) -> RegistryDirectory {
        RegistryDirectory::open(
            root.to_owned(),
            Arc::new(EventHub::new()),
            root.join("home"),
            None,
        )
        .unwrap()
    }

    #[test]
    fn lists_workspaces_newest_first_with_archived_and_skips_damaged_ones() {
        let d = tempfile::tempdir().unwrap();
        let dir = directory(d.path());
        assert!(dir.list().unwrap().is_empty());
        assert_eq!(dir.last_workspace().unwrap(), None);

        let reg = &dir.core().registry;
        let old = reg
            .create(NewWorkspace {
                name: "Old".into(),
                ..Default::default()
            })
            .unwrap();
        let new = reg
            .create(NewWorkspace {
                name: "New".into(),
                ..Default::default()
            })
            .unwrap();
        reg.archive(old.id()).unwrap();
        reg.touch(new.id()).unwrap();
        let broken = paths::workspace_dir(d.path(), "ws_bad");
        std::fs::create_dir_all(&broken).unwrap();
        std::fs::write(broken.join("workspace.json"), "{").unwrap();

        let list = dir.list().unwrap();
        let names: Vec<_> = list.iter().map(|w| (w.name.as_str(), w.archived)).collect();
        assert_eq!(names, [("New", false), ("Old", true)]);
        assert_eq!(dir.last_workspace().unwrap().as_deref(), Some(new.id()));
        reg.delete(new.id(), "New").unwrap();
        assert_eq!(dir.last_workspace().unwrap(), None);
    }

    #[test]
    fn migration_runs_once_and_leaves_a_banner() {
        let d = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(d.path().join("graph")).unwrap();
        std::fs::write(d.path().join("graph/nodes.jsonl"), "").unwrap();
        let dir = directory(d.path());
        dir.core().migrate_on_start();
        let list = dir.list().unwrap();
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].name, "Default");
        assert!(d
            .path()
            .join("workspaces")
            .join(&list[0].id)
            .join("graph")
            .is_dir());
        assert!(!d.path().join("graph").exists());
        let banners = dir.banners().unwrap();
        assert_eq!(banners.len(), 1);
        assert_eq!(banners[0].id, "migrated-v1");
        assert!(banners[0].message.contains("Default"));

        dir.core().migrate_on_start();
        assert_eq!(dir.list().unwrap().len(), 1, "second run is a no-op");
        assert!(migrate::marker(d.path()).unwrap().is_some());
    }

    #[test]
    fn replies_are_bounded_and_clock_defaults_to_real_time() {
        let r = Replies::new(2);
        r.put("a".into(), serde_json::json!(1));
        r.put("b".into(), serde_json::json!(2));
        r.put("c".into(), serde_json::json!(3));
        assert_eq!((r.get("a"), r.get("c")), (None, Some(serde_json::json!(3))));
        let before = lp_service::now_ms();
        assert!(Clock::default().now() >= before);
    }
}
