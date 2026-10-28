use std::future::Future;
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use lp_agent::loops::{run_turn, Session};
use lp_cli_driver::accounts::{
    discover, AccountHealth, AccountPool, DiscoverRoots, DiscoveredAccount, ProviderKind, Strategy,
};
use lp_cli_driver::engine::{CliKind, StreamDelta, SubprocessEngine};
use lp_core::graph::GraphSnapshot;
use lp_core::ports::GraphStore;
use lp_store::{FsChatStore, FsGraphStore, SqliteTrackerStore};
use tokio::sync::{mpsc, OnceCell};

pub type BackendResult<T> = Result<T, String>;
pub type BackendFuture<'a, T> = Pin<Box<dyn Future<Output = BackendResult<T>> + Send + 'a>>;
pub type BackendHandle = Arc<dyn Backend>;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountSlot {
    pub id: String,
    pub provider: String,
    pub email: Option<String>,
    pub tier: Option<String>,
    pub config_dir: Option<PathBuf>,
    pub status: String,
    pub requests: u32,
}

impl From<DiscoveredAccount> for AccountSlot {
    fn from(account: DiscoveredAccount) -> Self {
        Self {
            id: account.id.0,
            provider: match account.provider {
                ProviderKind::ClaudeCli => "claude",
                ProviderKind::CodexCli => "codex",
                ProviderKind::AgyCli => "agy",
            }
            .into(),
            email: account.email,
            tier: account.rate_limit_tier,
            config_dir: account.config_dir,
            status: if account.auth_expired {
                "Auth expired"
            } else {
                "Ready"
            }
            .into(),
            requests: 0,
        }
    }
}

pub trait Backend: Send + Sync {
    fn accounts(&self) -> BackendFuture<'_, Vec<AccountSlot>>;
    fn graph(&self) -> BackendFuture<'_, GraphSnapshot>;
    fn ingest(
        &self,
        allow: Vec<PathBuf>,
        since_ms: i64,
    ) -> BackendFuture<'_, lp_ingest::IngestReport>;
    fn turn(
        &self,
        session: Session,
        events: mpsc::Sender<StreamDelta>,
    ) -> BackendFuture<'_, Session>;
}

pub struct DesktopBackend {
    pub home: PathBuf,
    pub data_dir: PathBuf,
    pub repo_root: PathBuf,
    pub claude_config_dir: Option<PathBuf>,
    pub extra_claude_dirs: Vec<PathBuf>,
    pool: OnceCell<Arc<AccountPool>>,
}

impl DesktopBackend {
    pub fn new(home: PathBuf, data_dir: PathBuf, repo_root: PathBuf) -> Self {
        Self {
            home,
            data_dir,
            repo_root,
            claude_config_dir: std::env::var_os("CLAUDE_CONFIG_DIR").map(PathBuf::from),
            extra_claude_dirs: Vec::new(),
            pool: OnceCell::new(),
        }
    }

    pub fn from_env() -> Self {
        let home = std::env::var_os("HOME")
            .map(PathBuf::from)
            .unwrap_or_default();
        let data_dir = std::env::var_os("LP_DATA_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| home.join(".learnpuppy"));
        let repo_root = std::env::var_os("PWD")
            .map(PathBuf::from)
            .unwrap_or_default();
        Self::new(home, data_dir, repo_root)
    }

    async fn account_pool(&self) -> BackendResult<Arc<AccountPool>> {
        self.pool
            .get_or_try_init(|| async {
                let roots = DiscoverRoots {
                    home: self.home.clone(),
                    claude_config_dir: self.claude_config_dir.clone(),
                    extra_claude_dirs: self.extra_claude_dirs.clone(),
                    now_ms: now_ms(),
                };
                blocking(move || {
                    Ok(Arc::new(AccountPool::new(
                        discover(&roots),
                        Strategy::Priority,
                    )))
                })
                .await
            })
            .await
            .cloned()
    }
}

async fn blocking<T: Send + 'static>(
    operation: impl FnOnce() -> BackendResult<T> + Send + 'static,
) -> BackendResult<T> {
    tokio::task::spawn_blocking(operation)
        .await
        .map_err(|error| error.to_string())?
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

impl Backend for DesktopBackend {
    fn accounts(&self) -> BackendFuture<'_, Vec<AccountSlot>> {
        Box::pin(async move {
            Ok(self
                .account_pool()
                .await?
                .snapshot()
                .into_iter()
                .map(|account| {
                    let status = match account.health {
                        AccountHealth::Healthy => "Ready".into(),
                        AccountHealth::AuthExpired => "Auth expired".into(),
                        AccountHealth::Cooldown { until, .. } => {
                            format!(
                                "Cooldown ({}s)",
                                until.saturating_duration_since(Instant::now()).as_secs()
                            )
                        }
                    };
                    AccountSlot {
                        id: account.id.0,
                        provider: match account.provider {
                            ProviderKind::ClaudeCli => "claude",
                            ProviderKind::CodexCli => "codex",
                            ProviderKind::AgyCli => "agy",
                        }
                        .into(),
                        email: account.email,
                        tier: account.rate_limit_tier,
                        config_dir: account.config_dir,
                        status,
                        requests: account.window_requests,
                    }
                })
                .collect())
        })
    }

    fn graph(&self) -> BackendFuture<'_, GraphSnapshot> {
        let dir = self.data_dir.join("graph");
        Box::pin(blocking(move || {
            let store = FsGraphStore::open(dir).map_err(|error| error.to_string())?;
            Ok(GraphSnapshot::from_ledgers(
                &store.nodes().map_err(|error| error.to_string())?,
                &store.chains().map_err(|error| error.to_string())?,
            ))
        }))
    }

    fn ingest(
        &self,
        allow: Vec<PathBuf>,
        since_ms: i64,
    ) -> BackendFuture<'_, lp_ingest::IngestReport> {
        let home = self.home.clone();
        let dir = self.data_dir.join("tracker");
        let config_dir = self.claude_config_dir.clone();
        let extra_dirs = self.extra_claude_dirs.clone();
        Box::pin(blocking(move || {
            let store = SqliteTrackerStore::open(dir).map_err(|error| error.to_string())?;
            let mut roots = lp_ingest::Roots::from_home(&home);
            for dir in config_dir.into_iter().chain(extra_dirs) {
                let projects = dir.join("projects");
                if !roots.claude.contains(&projects) {
                    roots.claude.push(projects);
                }
            }
            Ok(lp_ingest::ingest(&store, &roots, &allow, since_ms))
        }))
    }

    fn turn(
        &self,
        mut session: Session,
        events: mpsc::Sender<StreamDelta>,
    ) -> BackendFuture<'_, Session> {
        Box::pin(async move {
            let pool = self.account_pool().await?;
            let chain = [
                ProviderKind::ClaudeCli,
                ProviderKind::CodexCli,
                ProviderKind::AgyCli,
            ];
            let id = pool
                .acquire(&chain, Instant::now())
                .ok_or("No CLI account is ready")?;
            let account = pool
                .snapshot()
                .into_iter()
                .find(|account| account.id == id)
                .ok_or("Selected account is unavailable")?;
            let engine = blocking(move || {
                let kind = match account.provider {
                    ProviderKind::ClaudeCli => CliKind::Claude,
                    ProviderKind::CodexCli => CliKind::Codex,
                    ProviderKind::AgyCli => CliKind::Agy,
                };
                let mut engine = SubprocessEngine::new(kind).map_err(|error| error.to_string())?;
                engine.config_dir = account.config_dir;
                engine.account = account.id.0;
                engine.role = "desktop".into();
                Ok(engine)
            })
            .await?;
            let root = self.repo_root.clone();
            let runtime = tokio::runtime::Handle::current();
            let result = blocking(move || {
                runtime
                    .block_on(run_turn(
                        &engine,
                        &root,
                        &mut session,
                        &events,
                        &Default::default(),
                    ))
                    .map_err(|error| error.to_string())?;
                Ok(session)
            })
            .await;
            let session = match result {
                Ok(session) => session,
                Err(message) => {
                    if lp_cli_driver::accounts::parse_cooldown(&message, now_ms() / 1000).is_some()
                        || message.to_lowercase().contains("rate limit")
                    {
                        pool.report_rate_limit(&id, &message, Instant::now(), now_ms() / 1000);
                    }
                    return Err(message);
                }
            };
            pool.report_success(&id, 0, 0);
            let dir = self.data_dir.join("chats");
            blocking(move || {
                let store = FsChatStore::open(dir).map_err(|error| error.to_string())?;
                session
                    .save(&store, now_ms() as i64)
                    .map_err(|error| error.to_string())?;
                Ok(session)
            })
            .await
        })
    }
}

#[derive(Clone, Default)]
pub struct FauxBackend {
    pub slots: Vec<AccountSlot>,
    pub error: Option<String>,
}

impl Backend for FauxBackend {
    fn accounts(&self) -> BackendFuture<'_, Vec<AccountSlot>> {
        Box::pin(async {
            self.error
                .clone()
                .map_or_else(|| Ok(self.slots.clone()), Err)
        })
    }

    fn graph(&self) -> BackendFuture<'_, GraphSnapshot> {
        Box::pin(async {
            self.error
                .clone()
                .map_or_else(|| Ok(GraphSnapshot::from_ledgers(&[], &[])), Err)
        })
    }

    fn ingest(&self, _: Vec<PathBuf>, _: i64) -> BackendFuture<'_, lp_ingest::IngestReport> {
        Box::pin(async {
            self.error
                .clone()
                .map_or_else(|| Ok(lp_ingest::IngestReport::default()), Err)
        })
    }

    fn turn(
        &self,
        session: Session,
        events: mpsc::Sender<StreamDelta>,
    ) -> BackendFuture<'_, Session> {
        Box::pin(async move {
            if let Some(error) = &self.error {
                return Err(error.clone());
            }
            events
                .send(StreamDelta::Text("Ready to learn.".into()))
                .await
                .map_err(|error| error.to_string())?;
            Ok(session)
        })
    }
}

pub fn spawn_turn(
    backend: BackendHandle,
    session: Session,
    mut text: dioxus::prelude::Signal<String>,
    mut result: dioxus::prelude::Signal<Option<BackendResult<Session>>>,
) -> dioxus::dioxus_core::Task {
    use dioxus::prelude::*;
    spawn(async move {
        text.set(String::new());
        result.set(None);
        let (events, mut deltas) = mpsc::channel(64);
        let turn = backend.turn(session, events);
        tokio::pin!(turn);
        loop {
            tokio::select! {
                delta = deltas.recv(), if !deltas.is_closed() || !deltas.is_empty() => {
                    if let Some(StreamDelta::Text(delta)) = delta {
                        text.write().push_str(&delta);
                    }
                }
                outcome = &mut turn => {
                    while let Ok(delta) = deltas.try_recv() {
                        if let StreamDelta::Text(delta) = delta {
                            text.write().push_str(&delta);
                        }
                    }
                    result.set(Some(outcome));
                    break;
                }
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_agent::loops::Kind;

    #[tokio::test]
    async fn faux_backend_streams_without_a_cli() {
        let backend = FauxBackend::default();
        let (events, mut deltas) = mpsc::channel(4);
        let session = backend
            .turn(Session::new("fixture", Kind::Teach), events)
            .await
            .unwrap();
        assert_eq!(session.id, "fixture");
        assert_eq!(
            deltas.recv().await,
            Some(StreamDelta::Text("Ready to learn.".into()))
        );
        assert!(deltas.recv().await.is_none());
    }

    #[tokio::test]
    async fn desktop_reads_stores_and_ingests_on_a_current_thread_runtime() {
        let dir = std::env::temp_dir().join(format!("lp-app-{}-{}", std::process::id(), now_ms()));
        let mut backend = DesktopBackend::new(dir.join("home"), dir.join("data"), dir.clone());
        backend.claude_config_dir = None;
        assert!(backend.accounts().await.unwrap().is_empty());
        assert!(backend.graph().await.unwrap().nodes.is_empty());
        let report = backend.ingest(vec![dir.clone()], 0).await.unwrap();
        assert_eq!(report.sessions, 0);
        assert!(report.errors.is_empty());
        let (events, _) = mpsc::channel(4);
        assert_eq!(
            backend
                .turn(Session::new("fixture", Kind::Teach), events)
                .await
                .unwrap_err(),
            "No CLI account is ready"
        );
        tokio::task::spawn_blocking(move || std::fs::remove_dir_all(dir))
            .await
            .unwrap()
            .unwrap();
    }
}
