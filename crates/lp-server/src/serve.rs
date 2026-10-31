//! `learnpuppy serve`: one server per data root (docs/web/01 §Process model).
//!
//! Binds `127.0.0.1` (and `[::1]` when available) from the requested port upward, writes
//! `server.lock` (pid, port, token hash), reports the launch URL, and serves until the shutdown
//! token is cancelled. A second `serve` that finds a healthy server behind the lock reports
//! that instance instead of starting another.

use std::io::{ErrorKind, Read, Write};
use std::net::{Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Context;
use lp_service::events::EventHub;
use lp_service::settings::Settings;
use lp_service::{accounts, paths};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tokio_util::sync::CancellationToken;

use crate::app::{AppState, EngineMode, ServerConfig, VERSION};
use crate::assets::Assets;
use crate::auth;
use crate::directory::DiskWorkspaces;

/// How many ports above the requested one are tried before giving up.
pub const PORT_TRIES: u16 = 50;

pub struct ServeOptions {
    /// First port to try; 0 lets the OS pick.
    pub port: u16,
    pub dev: bool,
    pub engine: EngineMode,
    pub data_root: PathBuf,
    /// Home for CLI account discovery (`~/.claude`, `~/.codex`, …).
    pub home: PathBuf,
    pub claude_config_dir: Option<PathBuf>,
    pub assets: Assets,
    pub heartbeat: Duration,
}

/// The line `serve` prints: where the app is and how to get in.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Launch {
    pub url: String,
    pub port: u16,
    /// `None` in dev mode, and when pointing at an already running server.
    pub token: Option<String>,
    /// An already running server was found; this process starts nothing.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub existing: bool,
}

impl Launch {
    fn new(port: u16, token: Option<String>, existing: bool) -> Self {
        let url = match &token {
            Some(t) => format!("http://127.0.0.1:{port}/?{}={t}", auth::TOKEN_PARAM),
            None => format!("http://127.0.0.1:{port}/"),
        };
        Self {
            url,
            port,
            token,
            existing,
        }
    }
}

/// Contents of `server.lock`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockInfo {
    pub pid: u32,
    pub port: u16,
    pub token_hash: Option<String>,
    pub started_at: i64,
    pub version: String,
}

pub fn read_lock(root: &Path) -> Option<LockInfo> {
    let bytes = std::fs::read(paths::server_lock(root)).ok()?;
    serde_json::from_slice(&bytes).ok()
}

/// Whether a LearnPuppy server answers `/api/v1/health` on `port`. Blocking.
pub fn probe_health(port: u16, timeout: Duration) -> bool {
    let addr = SocketAddr::from((Ipv4Addr::LOCALHOST, port));
    let Ok(mut s) = TcpStream::connect_timeout(&addr, timeout) else {
        return false;
    };
    if s.set_read_timeout(Some(timeout)).is_err() || s.set_write_timeout(Some(timeout)).is_err() {
        return false;
    }
    let req = format!(
        "GET /api/v1/health HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nAccept-Encoding: identity\r\nConnection: close\r\n\r\n"
    );
    if s.write_all(req.as_bytes()).is_err() {
        return false;
    }
    let mut buf = Vec::new();
    if s.take(64 * 1024).read_to_end(&mut buf).is_err() && buf.is_empty() {
        return false;
    }
    let text = String::from_utf8_lossy(&buf);
    text.starts_with("HTTP/1.1 200") && text.contains("\"ok\":true")
}

/// The healthy server behind `server.lock`, if any. Blocking.
pub fn running_instance(root: &Path) -> Option<LockInfo> {
    read_lock(root).filter(|l| probe_health(l.port, Duration::from_millis(800)))
}

/// Owns `server.lock` while the server runs; removes it on drop if it is still ours.
#[derive(Debug)]
pub struct LockGuard {
    path: PathBuf,
    pid: u32,
}

impl LockGuard {
    /// Takes the lock, replacing a stale one. `Ok(Err(other))` when a live server holds it.
    pub fn acquire(root: &Path, info: &LockInfo) -> anyhow::Result<Result<Self, LockInfo>> {
        let path = paths::server_lock(root);
        let body = serde_json::to_vec_pretty(info)?;
        let created = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path);
        match created {
            Ok(mut f) => f.write_all(&body)?,
            Err(e) if e.kind() == ErrorKind::AlreadyExists => {
                if let Some(other) = running_instance(root).filter(|o| o.pid != info.pid) {
                    return Ok(Err(other));
                }
                let tmp = path.with_extension("lock.tmp");
                std::fs::write(&tmp, &body)?;
                std::fs::rename(&tmp, &path)?;
            }
            Err(e) => return Err(e).with_context(|| path.display().to_string()),
        }
        Ok(Ok(Self {
            path,
            pid: info.pid,
        }))
    }
}

impl Drop for LockGuard {
    fn drop(&mut self) {
        let ours = std::fs::read(&self.path)
            .ok()
            .and_then(|b| serde_json::from_slice::<LockInfo>(&b).ok())
            .is_some_and(|l| l.pid == self.pid);
        if ours {
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Binds `127.0.0.1:port`, moving up while ports are busy. Port 0 lets the OS choose.
pub async fn bind_from(port: u16) -> std::io::Result<TcpListener> {
    if port == 0 {
        return TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await;
    }
    let last = port.saturating_add(PORT_TRIES - 1);
    for p in port..=last {
        match TcpListener::bind((Ipv4Addr::LOCALHOST, p)).await {
            Ok(l) => return Ok(l),
            Err(e) if e.kind() == ErrorKind::AddrInUse => continue,
            Err(e) => return Err(e),
        }
    }
    Err(std::io::Error::new(
        ErrorKind::AddrInUse,
        format!("ports {port}..={last} on 127.0.0.1 are all in use"),
    ))
}

/// Opens the browser at `url` (best effort).
pub fn open_browser(url: &str) {
    let cmd = if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    if let Err(e) = std::process::Command::new(cmd)
        .arg(url)
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        tracing::warn!(error = %e, "could not open the browser");
    }
}

/// Runs the server until `shutdown` is cancelled. `on_ready` gets the launch info once the
/// server accepts connections (or once an existing server is found, after which this returns).
pub async fn run(
    opts: ServeOptions,
    shutdown: CancellationToken,
    on_ready: impl FnOnce(&Launch),
) -> anyhow::Result<()> {
    let root = opts.data_root.clone();
    std::fs::create_dir_all(&root).with_context(|| root.display().to_string())?;

    let probe_root = root.clone();
    if let Some(other) = tokio::task::spawn_blocking(move || running_instance(&probe_root)).await? {
        on_ready(&Launch::new(other.port, None, true));
        return Ok(());
    }

    let listener = bind_from(opts.port).await.context("binding 127.0.0.1")?;
    let port = listener.local_addr()?.port();
    let mut listeners = vec![listener];
    match TcpListener::bind((Ipv6Addr::LOCALHOST, port)).await {
        Ok(l) => listeners.push(l),
        Err(e) => tracing::debug!(error = %e, port, "no [::1] listener"),
    }

    let token = (!opts.dev).then(auth::new_token);
    let info = LockInfo {
        pid: std::process::id(),
        port,
        token_hash: token.as_deref().map(auth::token_hash),
        started_at: lp_service::now_ms(),
        version: VERSION.to_owned(),
    };
    let lock_root = root.clone();
    let guard =
        match tokio::task::spawn_blocking(move || LockGuard::acquire(&lock_root, &info)).await?? {
            Ok(g) => g,
            Err(other) => {
                on_ready(&Launch::new(other.port, None, true));
                return Ok(());
            }
        };

    let (home, ccd, settings_root) = (
        opts.home.clone(),
        opts.claude_config_dir.clone(),
        root.clone(),
    );
    let (device_id, pool) = tokio::task::spawn_blocking(move || {
        let mut settings = Settings::load(&settings_root)?;
        let device = settings.ensure_device(&settings_root)?.to_owned();
        let pool = accounts::discover_pool(&accounts::roots(&home, ccd, &settings));
        anyhow::Ok((device, pool))
    })
    .await??;

    let config = ServerConfig {
        port,
        token: token.clone(),
        engine: opts.engine,
        data_root: root.clone(),
        heartbeat: opts.heartbeat,
    };
    let mut state = AppState::new(
        config,
        Arc::new(EventHub::new()),
        Arc::new(DiskWorkspaces::new(root.clone())),
        pool,
        opts.assets,
        device_id,
    );
    state.shutdown = shutdown.clone();

    let pump = state.pump.spawn(shutdown.clone());
    let app = crate::app::router(state);
    let servers: Vec<_> = listeners
        .into_iter()
        .map(|l| {
            let (app, stop) = (app.clone(), shutdown.clone());
            tokio::spawn(async move {
                axum::serve(l, app)
                    .with_graceful_shutdown(stop.cancelled_owned())
                    .await
            })
        })
        .collect();
    tracing::info!(port, engine = ?opts.engine, dev = opts.dev, root = %root.display(), "serving");
    on_ready(&Launch::new(port, token, false));

    let mut result = Ok(());
    for s in servers {
        if let Err(e) = s.await? {
            shutdown.cancel();
            result = Err(anyhow::Error::new(e).context("server failed"));
        }
    }
    pump.await?;
    drop(guard);
    tracing::info!(port, "stopped");
    result
}

/// Cancels `token` on SIGINT or SIGTERM.
pub fn cancel_on_signals(token: CancellationToken) {
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use tokio::signal::unix::{signal, SignalKind};
            match signal(SignalKind::terminate()) {
                Ok(mut term) => {
                    tokio::select! {
                        _ = tokio::signal::ctrl_c() => {}
                        _ = term.recv() => {}
                    }
                }
                Err(_) => {
                    let _ = tokio::signal::ctrl_c().await;
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = tokio::signal::ctrl_c().await;
        }
        tracing::info!("shutdown signal");
        token.cancel();
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opts(root: &Path, dev: bool) -> ServeOptions {
        ServeOptions {
            port: 0,
            dev,
            engine: EngineMode::Faux,
            data_root: root.join("data"),
            home: root.join("home"),
            claude_config_dir: None,
            assets: Assets::Dir(root.join("dist")),
            heartbeat: Duration::from_secs(15),
        }
    }

    #[tokio::test]
    async fn busy_ports_auto_increment() {
        let taken = bind_from(0).await.unwrap();
        let busy = taken.local_addr().unwrap().port();
        let next = bind_from(busy).await.unwrap();
        assert!(next.local_addr().unwrap().port() > busy);
        assert!(next.local_addr().unwrap().ip().is_loopback());
    }

    #[test]
    fn launch_url_carries_the_token_only_when_set() {
        let l = Launch::new(7878, Some("abc".into()), false);
        assert_eq!(l.url, "http://127.0.0.1:7878/?t=abc");
        let j = serde_json::to_value(&l).unwrap();
        assert_eq!(
            j,
            serde_json::json!({"url": l.url, "port": 7878, "token": "abc"})
        );
        let l = Launch::new(7878, None, true);
        assert_eq!(l.url, "http://127.0.0.1:7878/");
        assert_eq!(serde_json::to_value(&l).unwrap()["existing"], true);
    }

    #[test]
    fn stale_lock_is_replaced_and_removed_on_drop() {
        let d = tempfile::tempdir().unwrap();
        let info = |pid| LockInfo {
            pid,
            port: 1,
            token_hash: None,
            started_at: 0,
            version: "x".into(),
        };
        // Port 1 never answers, so this lock is stale.
        std::fs::write(
            paths::server_lock(d.path()),
            serde_json::to_vec(&info(999_999)).unwrap(),
        )
        .unwrap();
        let g = LockGuard::acquire(d.path(), &info(42)).unwrap().unwrap();
        assert_eq!(read_lock(d.path()).unwrap().pid, 42);
        drop(g);
        assert!(read_lock(d.path()).is_none());

        // A lock rewritten by someone else is left alone.
        let g = LockGuard::acquire(d.path(), &info(42)).unwrap().unwrap();
        std::fs::write(
            paths::server_lock(d.path()),
            serde_json::to_vec(&info(7)).unwrap(),
        )
        .unwrap();
        drop(g);
        assert_eq!(read_lock(d.path()).unwrap().pid, 7);
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn serves_writes_lock_defers_to_a_running_server_and_shuts_down() {
        let d = tempfile::tempdir().unwrap();
        let stop = CancellationToken::new();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(run(opts(d.path(), false), stop.clone(), move |l| {
            let _ = tx.send(l.clone());
        }));
        let launch = rx.await.unwrap();
        assert!(!launch.existing);
        let token = launch.token.clone().unwrap();
        assert_eq!(
            launch.url,
            format!("http://127.0.0.1:{}/?t={token}", launch.port)
        );

        let root = d.path().join("data");
        let lock = read_lock(&root).unwrap();
        assert_eq!(lock.port, launch.port);
        assert_eq!(lock.pid, std::process::id());
        assert_eq!(lock.token_hash, Some(auth::token_hash(&token)));
        let port = launch.port;
        assert!(
            tokio::task::spawn_blocking(move || probe_health(port, Duration::from_secs(2)))
                .await
                .unwrap()
        );
        // `ensure_device` minted and saved a device id.
        assert!(!Settings::load(&root).unwrap().device_id.is_empty());

        // A second serve on the same data root points at the first one.
        let (tx2, rx2) = tokio::sync::oneshot::channel();
        run(opts(d.path(), false), CancellationToken::new(), move |l| {
            let _ = tx2.send(l.clone());
        })
        .await
        .unwrap();
        let second = rx2.await.unwrap();
        assert_eq!(
            (second.existing, second.port, second.token),
            (true, port, None)
        );

        stop.cancel();
        tokio::time::timeout(Duration::from_secs(5), server)
            .await
            .unwrap()
            .unwrap()
            .unwrap();
        assert!(read_lock(&root).is_none(), "lock removed on shutdown");
        assert!(!tokio::task::spawn_blocking(move || probe_health(
            port,
            Duration::from_millis(300)
        ))
        .await
        .unwrap());
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn dev_mode_has_no_token() {
        let d = tempfile::tempdir().unwrap();
        let stop = CancellationToken::new();
        let (tx, rx) = tokio::sync::oneshot::channel();
        let server = tokio::spawn(run(opts(d.path(), true), stop.clone(), move |l| {
            let _ = tx.send(l.clone());
        }));
        let launch = rx.await.unwrap();
        assert_eq!(launch.token, None);
        assert_eq!(launch.url, format!("http://127.0.0.1:{}/", launch.port));
        assert_eq!(read_lock(&d.path().join("data")).unwrap().token_hash, None);
        stop.cancel();
        server.await.unwrap().unwrap();
    }
}
