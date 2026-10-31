//! `WorkspaceRegistry`: lists, creates, edits, archives and deletes workspaces, and hands out lazily
//! opened [`WorkspaceHandle`]s (idle-closed after 10 min, at most 8 open; docs/web/01 §Concurrency).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lp_core::ports::{ChatStore, TrackerStore};
use parking_lot::{Mutex, RwLock};
use serde_json::json;
use tokio_util::sync::CancellationToken;

use super::meta::{
    self, is_valid_id, name_key, unique_name, unique_slug, NewWorkspace, WorkspaceMeta,
    WorkspacePatch,
};
use super::{fsx, state::State, Result, WorkspaceError};
use crate::paths;
use crate::stores::Stores;

/// A workspace-scoped or global event for the SSE hub (docs/web/03 §Events).
#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceEvent {
    /// `None` for global events (`workspace.created|updated|deleted` are global so every tab sees them).
    pub workspace: Option<String>,
    /// Wire event type, e.g. `workspace.updated`, `graph.changed`.
    pub kind: String,
    pub data: serde_json::Value,
}

/// Where workspace events go. `lp-server`'s event bus implements this; tests use [`NoopSink`] or a
/// recording sink.
pub trait EventSink: Send + Sync {
    fn emit(&self, event: WorkspaceEvent);
}

/// Drops every event.
pub struct NoopSink;

impl EventSink for NoopSink {
    fn emit(&self, _: WorkspaceEvent) {}
}

/// Registry tuning; [`Default`] is the docs/web/02 behaviour.
#[derive(Clone, Debug)]
pub struct RegistryConfig {
    /// Close a handle after this long without use, streams or jobs.
    pub idle_after: Duration,
    /// Keep at most this many handles open; the least recently used idle one is closed first.
    pub max_open: usize,
    /// How long list stats are cached.
    pub stats_ttl: Duration,
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            idle_after: Duration::from_secs(10 * 60),
            max_open: 8,
            stats_ttl: Duration::from_secs(30),
        }
    }
}

/// List-card numbers for one workspace.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkspaceStats {
    /// Distinct propositions after merging the ledger.
    pub nodes: usize,
    pub chains: usize,
    pub chats: usize,
    pub due_cards: usize,
    /// Latest of chat updates, reviews and node creation (epoch ms).
    pub last_activity_at: Option<i64>,
}

/// One healthy workspace in a listing.
#[derive(Clone, Debug, PartialEq)]
pub struct WorkspaceSummary {
    pub meta: WorkspaceMeta,
    /// `None` if the stores could not be read (the error is logged).
    pub stats: Option<WorkspaceStats>,
}

/// A workspace directory whose `workspace.json` is missing or corrupt; offer Repair / Reveal.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DamagedWorkspace {
    pub id: String,
    pub path: PathBuf,
    pub reason: String,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct WorkspaceList {
    /// Most recently opened first.
    pub workspaces: Vec<WorkspaceSummary>,
    pub damaged: Vec<DamagedWorkspace>,
}

/// One open workspace: its stores, per-chat turn locks, cancellation for running work and events.
pub struct WorkspaceHandle {
    id: String,
    root: PathBuf,
    stores: Stores,
    meta: RwLock<WorkspaceMeta>,
    chat_locks: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    cancel: CancellationToken,
    events: Arc<dyn EventSink>,
    last_used: Mutex<Instant>,
    busy: AtomicUsize,
}

impl std::fmt::Debug for WorkspaceHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WorkspaceHandle")
            .field("id", &self.id)
            .field("root", &self.root)
            .field("busy", &self.busy.load(Ordering::Relaxed))
            .finish_non_exhaustive()
    }
}

/// Marks a handle busy (a stream, turn or job) so it is never idle-closed; released on drop.
pub struct BusyGuard(Arc<WorkspaceHandle>);

impl Drop for BusyGuard {
    fn drop(&mut self) {
        self.0.busy.fetch_sub(1, Ordering::AcqRel);
        self.0.touch();
    }
}

impl WorkspaceHandle {
    fn open(root: PathBuf, meta: WorkspaceMeta, events: Arc<dyn EventSink>) -> Result<Self> {
        let stores = Stores::open(&root)?;
        fsx::create_dir(&root.join("artifacts"))?;
        Ok(Self {
            id: meta.id.clone(),
            root,
            stores,
            meta: RwLock::new(meta),
            chat_locks: Mutex::new(HashMap::new()),
            cancel: CancellationToken::new(),
            events,
            last_used: Mutex::new(Instant::now()),
            busy: AtomicUsize::new(0),
        })
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    /// The workspace root (`workspaces/<id>`), which every store lives under.
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn stores(&self) -> &Stores {
        &self.stores
    }

    /// A snapshot of the current metadata.
    pub fn meta(&self) -> WorkspaceMeta {
        self.meta.read().clone()
    }

    pub fn is_archived(&self) -> bool {
        self.meta.read().archived
    }

    /// The read-only guard: every mutation calls this first. Archived ⇒
    /// [`WorkspaceError::Archived`] (`409 workspace_archived`).
    pub fn ensure_writable(&self) -> Result<()> {
        if self.is_archived() {
            Err(WorkspaceError::Archived(self.id.clone()))
        } else {
            Ok(())
        }
    }

    /// The turn lock of one chat. One active turn per chat: callers `try_lock_owned` and answer
    /// `409 turn_in_progress` when it is held.
    pub fn chat_lock(&self, chat_id: &str) -> Arc<tokio::sync::Mutex<()>> {
        let mut locks = self.chat_locks.lock();
        // Forget locks nobody holds or waits on, so the map tracks live chats only.
        locks.retain(|id, l| id == chat_id || Arc::strong_count(l) > 1);
        locks.entry(chat_id.to_owned()).or_default().clone()
    }

    /// Cancelled when the workspace is deleted; turns and jobs run under a child of it.
    pub fn cancel_token(&self) -> CancellationToken {
        self.cancel.child_token()
    }

    /// Emit an event scoped to this workspace.
    pub fn emit(&self, kind: &str, data: serde_json::Value) {
        self.events.emit(WorkspaceEvent {
            workspace: Some(self.id.clone()),
            kind: kind.into(),
            data,
        });
    }

    /// Keep this handle open while the guard lives (SSE stream, running turn or job).
    pub fn busy(self: &Arc<Self>) -> BusyGuard {
        self.busy.fetch_add(1, Ordering::AcqRel);
        self.touch();
        BusyGuard(self.clone())
    }

    fn touch(&self) {
        *self.last_used.lock() = Instant::now();
    }

    fn idle_since(&self) -> Option<Instant> {
        (self.busy.load(Ordering::Acquire) == 0).then(|| *self.last_used.lock())
    }
}

/// All workspaces under one data root.
pub struct WorkspaceRegistry {
    root: PathBuf,
    config: RegistryConfig,
    events: Arc<dyn EventSink>,
    open: Mutex<HashMap<String, Arc<WorkspaceHandle>>>,
    stats: Mutex<HashMap<String, (Instant, WorkspaceStats)>>,
    /// Serialises metadata-changing operations so name uniqueness checks cannot race.
    ops: Mutex<()>,
}

impl WorkspaceRegistry {
    /// A registry over `<root>/workspaces`, creating the directory if needed.
    pub fn new(root: impl Into<PathBuf>, events: Arc<dyn EventSink>) -> Result<Self> {
        Self::with_config(root, events, RegistryConfig::default())
    }

    pub fn with_config(
        root: impl Into<PathBuf>,
        events: Arc<dyn EventSink>,
        config: RegistryConfig,
    ) -> Result<Self> {
        let root = root.into();
        fsx::create_dir(&paths::workspaces_dir(&root))?;
        Ok(Self {
            root,
            config,
            events,
            open: Mutex::new(HashMap::new()),
            stats: Mutex::new(HashMap::new()),
            ops: Mutex::new(()),
        })
    }

    /// The data root.
    pub fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn dir(&self, id: &str) -> Result<PathBuf> {
        if !is_valid_id(id) {
            return Err(WorkspaceError::NotFound(id.to_owned()));
        }
        Ok(paths::workspace_dir(&self.root, id))
    }

    pub(crate) fn emit_global(&self, kind: &str, data: serde_json::Value) {
        self.events.emit(WorkspaceEvent {
            workspace: None,
            kind: kind.into(),
            data,
        });
    }

    /// Every workspace directory: healthy metadata or the reason it is damaged.
    pub(crate) fn scan(&self) -> Result<Vec<std::result::Result<WorkspaceMeta, DamagedWorkspace>>> {
        let mut out = vec![];
        for entry in std::fs::read_dir(paths::workspaces_dir(&self.root))? {
            let entry = entry?;
            let Some(id) = entry.file_name().to_str().map(str::to_owned) else {
                continue;
            };
            // Hidden entries are in-flight creates/imports; other names are not workspaces.
            if !is_valid_id(&id) || !entry.file_type()?.is_dir() {
                continue;
            }
            out.push(match WorkspaceMeta::load(&entry.path()) {
                Ok(m) => Ok(m),
                Err(WorkspaceError::Damaged { reason, .. }) => Err(DamagedWorkspace {
                    id,
                    path: entry.path(),
                    reason,
                }),
                Err(WorkspaceError::NotFound(_)) => continue,
                Err(e) => return Err(e),
            });
        }
        Ok(out)
    }

    /// Names (as typed) of every live (non-archived) workspace except `except`.
    pub(crate) fn live_names(&self, except: Option<&str>) -> Result<Vec<String>> {
        Ok(self
            .scan()?
            .into_iter()
            .filter_map(|r| r.ok())
            .filter(|m| !m.archived && Some(m.id.as_str()) != except)
            .map(|m| m.name)
            .collect())
    }

    pub(crate) fn slugs(&self, except: Option<&str>) -> Result<Vec<String>> {
        Ok(self
            .scan()?
            .into_iter()
            .filter_map(|r| r.ok())
            .filter(|m| Some(m.id.as_str()) != except)
            .map(|m| m.slug)
            .collect())
    }

    fn ensure_unique(&self, name: &str, except: Option<&str>) -> Result<()> {
        let key = name_key(name);
        if self.live_names(except)?.iter().any(|n| name_key(n) == key) {
            return Err(WorkspaceError::NameTaken(name.to_owned()));
        }
        Ok(())
    }

    /// Workspaces with stats (cached for `stats_ttl`), most recently opened first; archived ones
    /// only when `include_archived`. Damaged directories are always reported.
    pub fn list(&self, include_archived: bool) -> Result<WorkspaceList> {
        let mut list = WorkspaceList::default();
        for entry in self.scan()? {
            match entry {
                Ok(meta) if meta.archived && !include_archived => {}
                Ok(meta) => {
                    let stats = match self.stats(&meta.id) {
                        Ok(s) => Some(s),
                        Err(e) => {
                            tracing::warn!(workspace = %meta.id, "stats unavailable: {e}");
                            None
                        }
                    };
                    list.workspaces.push(WorkspaceSummary { meta, stats });
                }
                Err(d) => list.damaged.push(d),
            }
        }
        list.workspaces.sort_by(|a, b| {
            b.meta
                .last_opened_at
                .cmp(&a.meta.last_opened_at)
                .then_with(|| name_key(&a.meta.name).cmp(&name_key(&b.meta.name)))
        });
        list.damaged.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(list)
    }

    /// One workspace's metadata.
    pub fn get(&self, id: &str) -> Result<WorkspaceMeta> {
        WorkspaceMeta::load(&self.dir(id)?)
    }

    /// Stats for one workspace, from cache when fresh. Uses the open handle's stores, else opens
    /// them briefly without caching a handle.
    pub fn stats(&self, id: &str) -> Result<WorkspaceStats> {
        if let Some((at, s)) = self.stats.lock().get(id) {
            if at.elapsed() < self.config.stats_ttl {
                return Ok(s.clone());
            }
        }
        let open = self.open.lock().get(id).cloned();
        let s = match open {
            Some(h) => compute_stats(h.stores())?,
            None => compute_stats(&Stores::open(&self.dir(id)?)?)?,
        };
        self.stats
            .lock()
            .insert(id.to_owned(), (Instant::now(), s.clone()));
        Ok(s)
    }

    /// Drop cached stats after a mutation so the next list is fresh.
    pub fn invalidate_stats(&self, id: &str) {
        self.stats.lock().remove(id);
    }

    /// Create a workspace: validate, build it in a hidden staging dir, then rename into place so a
    /// half-created workspace is never listed. Emits `workspace.created`.
    pub fn create(&self, new: NewWorkspace) -> Result<Arc<WorkspaceHandle>> {
        let _ops = self.ops.lock();
        let name = meta::validate_name(&new.name)?;
        self.ensure_unique(&name, None)?;
        let now = crate::now_ms();
        let mut m = WorkspaceMeta::new(self.fresh_id()?, name, now);
        m.icon = meta::validate_icon(new.icon.as_deref().unwrap_or(meta::DEFAULT_ICON))?;
        m.hue = meta::validate_hue(new.hue.as_deref().unwrap_or(meta::DEFAULT_HUE))?;
        m.description = meta::validate_description(new.description.as_deref().unwrap_or(""))?;
        if let Some(s) = new.sources {
            m.sources = meta::validate_sources(s)?;
        }
        m.slug = unique_slug(&m.slug, &self.slugs(None)?);
        let staging = paths::workspaces_dir(&self.root).join(format!(".creating-{}", m.id));
        let built = (|| {
            fsx::create_dir(&staging)?;
            Stores::open(&staging)?;
            fsx::create_dir(&staging.join("artifacts"))?;
            m.save(&staging)?;
            fsx::rename(&staging, &self.dir(&m.id)?)
        })();
        if let Err(e) = built {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(e);
        }
        let handle = self.insert_handle(m.clone())?;
        self.emit_global("workspace.created", json!({ "id": m.id }));
        Ok(handle)
    }

    /// An id whose directory does not exist yet.
    pub(crate) fn fresh_id(&self) -> Result<String> {
        for _ in 0..64 {
            let id = meta::new_id();
            if !fsx::exists(&self.dir(&id)?) {
                return Ok(id);
            }
        }
        Err(WorkspaceError::Internal(
            "could not mint a free workspace id".into(),
        ))
    }

    /// The open handle for `id`, opening the stores on first use. Archived workspaces open
    /// read-only (see [`WorkspaceHandle::ensure_writable`]).
    pub fn open(&self, id: &str) -> Result<Arc<WorkspaceHandle>> {
        if let Some(h) = self.open.lock().get(id) {
            h.touch();
            return Ok(h.clone());
        }
        let m = self.get(id)?;
        self.insert_handle(m)
    }

    fn insert_handle(&self, m: WorkspaceMeta) -> Result<Arc<WorkspaceHandle>> {
        let id = m.id.clone();
        let fresh = Arc::new(WorkspaceHandle::open(
            self.dir(&id)?,
            m,
            self.events.clone(),
        )?);
        let mut open = self.open.lock();
        // Another caller may have opened it meanwhile; keep theirs.
        if let Some(h) = open.get(&id) {
            return Ok(h.clone());
        }
        while open.len() >= self.config.max_open {
            let lru = open
                .iter()
                .filter_map(|(k, h)| h.idle_since().map(|t| (t, k.clone())))
                .min();
            match lru {
                Some((_, k)) => {
                    open.remove(&k);
                }
                // Everything is busy: run over the cap rather than block a request.
                None => break,
            }
        }
        open.insert(id, fresh.clone());
        Ok(fresh)
    }

    /// Record that the user opened `id`: bumps `last_opened_at` and `state.json` `last_workspace`.
    pub fn touch(&self, id: &str) -> Result<()> {
        let now = crate::now_ms();
        self.update_meta(id, false, |m| {
            m.last_opened_at = now;
            Ok(())
        })?;
        State::update(&self.root, |s| s.last_workspace = Some(id.to_owned()))?;
        Ok(())
    }

    /// Close handles idle for at least `idle_after` as of `now` (call periodically). Returns the
    /// closed ids.
    pub fn close_idle(&self, now: Instant) -> Vec<String> {
        let mut open = self.open.lock();
        let doomed: Vec<String> = open
            .iter()
            .filter(|(_, h)| {
                h.idle_since()
                    .is_some_and(|t| now.saturating_duration_since(t) >= self.config.idle_after)
            })
            .map(|(k, _)| k.clone())
            .collect();
        for k in &doomed {
            open.remove(k);
        }
        doomed
    }

    /// Ids of the currently open handles.
    pub fn open_ids(&self) -> Vec<String> {
        let mut ids: Vec<String> = self.open.lock().keys().cloned().collect();
        ids.sort();
        ids
    }

    /// Read-modify-write of `workspace.json` under the ops lock, mirrored into the open handle.
    fn update_meta(
        &self,
        id: &str,
        allow_archived: bool,
        f: impl FnOnce(&mut WorkspaceMeta) -> Result<()>,
    ) -> Result<WorkspaceMeta> {
        let _ops = self.ops.lock();
        let dir = self.dir(id)?;
        let mut m = WorkspaceMeta::load(&dir)?;
        if m.archived && !allow_archived {
            return Err(WorkspaceError::Archived(id.to_owned()));
        }
        f(&mut m)?;
        m.save(&dir)?;
        if let Some(h) = self.open.lock().get(id) {
            *h.meta.write() = m.clone();
        }
        Ok(m)
    }

    /// PATCH name/icon/hue/description/sources/overrides. Archived workspaces are read-only.
    /// Emits `workspace.updated`.
    pub fn update(&self, id: &str, patch: WorkspacePatch) -> Result<WorkspaceMeta> {
        let new_name = patch.name.as_deref().map(meta::validate_name).transpose()?;
        // Checked under the ops lock (inside `update_meta`) so concurrent renames cannot race.
        let m = self.update_meta(id, false, |m| {
            if let Some(name) = &new_name {
                self.ensure_unique(name, Some(id))?;
                if name != &m.name {
                    m.slug = unique_slug(&meta::slugify(name), &self.slugs(Some(id))?);
                }
            }
            m.apply(patch)
        })?;
        self.emit_global("workspace.updated", json!({ "id": id }));
        Ok(m)
    }

    /// Make a workspace read-only and hide it from the default list.
    pub fn archive(&self, id: &str) -> Result<WorkspaceMeta> {
        let m = self.update_meta(id, true, |m| {
            m.archived = true;
            Ok(())
        })?;
        self.emit_global("workspace.updated", json!({ "id": id }));
        Ok(m)
    }

    /// Un-archive. If a live workspace took the name meanwhile, this one gets a ` (2)` suffix.
    pub fn restore(&self, id: &str) -> Result<WorkspaceMeta> {
        let m = self.update_meta(id, true, |m| {
            m.archived = false;
            m.name = unique_name(&m.name, &self.live_names(Some(id))?);
            Ok(())
        })?;
        self.emit_global("workspace.updated", json!({ "id": id }));
        Ok(m)
    }

    /// Remove an open handle and cancel its running turns and jobs (delete, purge).
    pub(crate) fn evict(&self, id: &str) {
        if let Some(h) = self.open.lock().remove(id) {
            h.cancel.cancel();
        }
        self.invalidate_stats(id);
    }

    pub(crate) fn ops_lock(&self) -> parking_lot::MutexGuard<'_, ()> {
        self.ops.lock()
    }

    /// Rewrite minimal metadata for a damaged workspace from its directory name. The name is the
    /// id (made unique), every other field takes its default. Emits `workspace.updated`.
    pub fn repair(&self, id: &str) -> Result<WorkspaceMeta> {
        let _ops = self.ops.lock();
        let dir = self.dir(id)?;
        match WorkspaceMeta::load(&dir) {
            Ok(_) => {
                return Err(WorkspaceError::Conflict(format!(
                    "workspace {id} is not damaged"
                )))
            }
            Err(WorkspaceError::Damaged { .. }) => {}
            Err(e) => return Err(e),
        }
        let created = std::fs::metadata(&dir)
            .and_then(|m| m.modified())
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map_or_else(crate::now_ms, |d| d.as_millis() as i64);
        let mut m = WorkspaceMeta::new(id.to_owned(), id.to_owned(), created);
        m.name = unique_name(id, &self.live_names(Some(id))?);
        m.slug = unique_slug(&meta::slugify(&m.name), &self.slugs(Some(id))?);
        m.save(&dir)?;
        self.invalidate_stats(id);
        self.emit_global("workspace.updated", json!({ "id": id }));
        Ok(m)
    }
}

fn compute_stats(s: &Stores) -> Result<WorkspaceStats> {
    let snap = crate::graph::snapshot(&*s.graph)?;
    let chats = s.chats.list_chats()?;
    let now = crate::now_ms();
    let due = crate::review::due_cards(&*s.tracker, now)?.len();
    let last_chat = chats.iter().map(|c| c.updated_at).max();
    let last_review = s.tracker.reviews()?.iter().map(|r| r.ts).max();
    let last_node = snap
        .nodes
        .values()
        .map(|n| n.created_at.timestamp_millis())
        .max();
    Ok(WorkspaceStats {
        nodes: snap.nodes.len(),
        chains: snap.chains.len(),
        chats: chats.len(),
        due_cards: due,
        last_activity_at: [last_chat, last_review, last_node]
            .into_iter()
            .flatten()
            .max(),
    })
}
