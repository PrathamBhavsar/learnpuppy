//! `learnpuppy doctor`: data root, running server, default port, disk, CLIs and logins, web UI.

use std::io::Write;
use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};

use lp_service::accounts;
use lp_service::settings::Settings;

use crate::assets::Assets;

/// Free space below this is a warning.
const LOW_DISK_BYTES: u64 = 1 << 30;
pub const DEFAULT_PORT: u16 = 7878;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Level {
    Ok,
    Warn,
    Fail,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Check {
    pub level: Level,
    pub name: &'static str,
    pub detail: String,
}

fn check(level: Level, name: &'static str, detail: impl Into<String>) -> Check {
    Check {
        level,
        name,
        detail: detail.into(),
    }
}

pub struct DoctorInput {
    pub data_root: PathBuf,
    pub home: PathBuf,
    pub claude_config_dir: Option<PathBuf>,
    /// `$PATH` entries searched for the CLIs.
    pub path_dirs: Vec<PathBuf>,
    pub assets: Assets,
    pub port: u16,
    /// Run `kiro-cli`/`opencode` status commands and test the network. Off in tests.
    pub probe: bool,
}

fn data_root_check(root: &Path) -> Check {
    let probe = root.join(".doctor-probe");
    let writable = std::fs::create_dir_all(root)
        .and_then(|()| std::fs::write(&probe, b"ok"))
        .and_then(|()| std::fs::remove_file(&probe));
    match writable {
        Ok(()) => check(
            Level::Ok,
            "data root",
            format!("{} (writable)", root.display()),
        ),
        Err(e) => check(Level::Fail, "data root", format!("{}: {e}", root.display())),
    }
}

fn server_check(root: &Path, port: u16) -> Vec<Check> {
    if let Some(l) = crate::serve::running_instance(root) {
        return vec![check(
            Level::Ok,
            "server",
            format!("running (pid {}, http://127.0.0.1:{}/)", l.pid, l.port),
        )];
    }
    let mut out = Vec::new();
    if crate::serve::read_lock(root).is_some() {
        out.push(check(
            Level::Warn,
            "server",
            "stale server.lock (no server answers); the next `serve` replaces it",
        ));
    } else {
        out.push(check(Level::Ok, "server", "not running"));
    }
    match TcpListener::bind((Ipv4Addr::LOCALHOST, port)) {
        Ok(_) => out.push(check(Level::Ok, "port", format!("{port} is free"))),
        Err(e) => out.push(check(
            Level::Warn,
            "port",
            format!("{port} is busy ({e}); `serve` will use the next free port"),
        )),
    }
    out
}

/// Free bytes on the filesystem holding `path`, via `df -Pk`.
fn free_bytes(path: &Path) -> Option<u64> {
    let existing = path.ancestors().find(|p| p.exists())?;
    let out = std::process::Command::new("df")
        .arg("-Pk")
        .arg(existing)
        .output()
        .ok()?;
    let text = String::from_utf8_lossy(&out.stdout);
    let kb: u64 = text
        .lines()
        .nth(1)?
        .split_whitespace()
        .nth(3)?
        .parse()
        .ok()?;
    Some(kb * 1024)
}

fn disk_check(root: &Path) -> Check {
    match free_bytes(root) {
        Some(b) => {
            let gb = b as f64 / f64::from(1u32 << 30);
            let level = if b < LOW_DISK_BYTES {
                Level::Warn
            } else {
                Level::Ok
            };
            check(level, "disk", format!("{gb:.1} GB free"))
        }
        None => check(Level::Warn, "disk", "could not read free space (df)"),
    }
}

fn find_on_path(dirs: &[PathBuf], bin: &str) -> Option<PathBuf> {
    dirs.iter().map(|d| d.join(bin)).find(|p| p.is_file())
}

fn cli_checks(input: &DoctorInput) -> Vec<Check> {
    let settings = Settings::load(&input.data_root).unwrap_or_default();
    let mut roots = accounts::roots(&input.home, input.claude_config_dir.clone(), &settings);
    roots.probe_clis = input.probe;
    let pool = accounts::discover_pool(&roots);
    let found = accounts::list(&pool);
    let mut out = Vec::new();
    for (bin, name) in [
        ("claude", "claude cli"),
        ("codex", "codex cli"),
        ("agy", "agy cli"),
    ] {
        let logins = found.iter().filter(|a| a.provider == bin).count();
        let c = match (find_on_path(&input.path_dirs, bin), logins) {
            (Some(p), n) if n > 0 => check(
                Level::Ok,
                name,
                format!(
                    "{} ({n} login{})",
                    p.display(),
                    if n == 1 { "" } else { "s" }
                ),
            ),
            (Some(p), _) => check(
                Level::Warn,
                name,
                format!("{} (no login found)", p.display()),
            ),
            (None, n) if n > 0 => check(
                Level::Warn,
                name,
                format!("{n} login(s) found but `{bin}` is not on PATH"),
            ),
            (None, _) => check(Level::Warn, name, "not installed"),
        };
        out.push(c);
    }
    if input.probe {
        for (bin, name, provider) in [
            ("kiro-cli", "kiro cli", "kiro"),
            ("opencode", "opencode cli", "opencode"),
        ] {
            out.push(free_cli_check(&found, name, provider, bin));
        }
        out.push(network_check());
    }
    if found.is_empty() {
        out.push(check(
            Level::Fail,
            "accounts",
            "no CLI login found; connect kiro or opencode (free), or claude, codex, agy (or use `serve --engine faux`)",
        ));
    } else {
        out.push(check(
            Level::Ok,
            "accounts",
            format!("{} usable", found.len()),
        ));
    }
    out
}

/// One line for kiro/opencode: installed? logged in? how many models, how many free, limited until?
fn free_cli_check(
    found: &[accounts::Account],
    name: &'static str,
    provider: &str,
    bin: &str,
) -> Check {
    let mine: Vec<_> = found.iter().filter(|a| a.provider == provider).collect();
    if mine.is_empty() {
        return check(
            Level::Warn,
            name,
            format!(
                "not connected; install `{bin}` and run `{}` yourself",
                accounts::how_to_connect(provider)
            ),
        );
    }
    let mut parts = Vec::new();
    let mut level = Level::Ok;
    for a in mine {
        let free = a.models.iter().filter(|m| m.free).count();
        let state = match &a.status {
            accounts::Status::Ready => "ready".to_owned(),
            accounts::Status::Cooldown { secs_left, .. } => {
                level = Level::Warn;
                format!("limited, resets in {}", human_secs(*secs_left))
            }
            accounts::Status::AuthExpired => {
                level = Level::Warn;
                format!("signed out; run `{}`", accounts::how_to_connect(provider))
            }
            accounts::Status::Disabled => "disabled".to_owned(),
            accounts::Status::Offline => {
                level = Level::Warn;
                "offline".to_owned()
            }
            accounts::Status::NoModel => {
                level = Level::Warn;
                "no usable model".to_owned()
            }
            accounts::Status::Missing => {
                level = Level::Warn;
                "missing".to_owned()
            }
        };
        parts.push(format!(
            "{} {state} ({} models, {free} free)",
            a.id,
            a.models.len()
        ));
    }
    check(level, name, parts.join("; "))
}

fn human_secs(s: u64) -> String {
    match (s / 3600, s % 3600 / 60) {
        (0, m) => format!("{m}m"),
        (h, m) => format!("{h}h {m}m"),
    }
}

/// A 1.5 s TCP connect to a public resolver: the same "offline" the engine would hit.
fn network_check() -> Check {
    use std::net::{SocketAddr, TcpStream};
    use std::time::Duration;
    let addr = SocketAddr::from(([1, 1, 1, 1], 443));
    match TcpStream::connect_timeout(&addr, Duration::from_millis(1500)) {
        Ok(_) => check(Level::Ok, "network", "online"),
        Err(e) => check(
            Level::Warn,
            "network",
            format!("offline ({e}); LearnPuppy pauses and retries when the AI cannot be reached"),
        ),
    }
}

fn web_check(assets: &Assets) -> Check {
    if assets.has_index() {
        check(Level::Ok, "web ui", "built")
    } else {
        check(
            Level::Warn,
            "web ui",
            "not built; run `bun run --cwd web build` (the API still works)",
        )
    }
}

/// Every check, in display order. Blocking.
pub fn checks(input: &DoctorInput) -> Vec<Check> {
    let mut out = vec![data_root_check(&input.data_root)];
    out.extend(server_check(&input.data_root, input.port));
    out.push(disk_check(&input.data_root));
    out.extend(cli_checks(input));
    out.push(web_check(&input.assets));
    out
}

/// Prints the report; returns whether nothing failed.
pub fn report(checks: &[Check], out: &mut dyn Write) -> std::io::Result<bool> {
    for c in checks {
        let tag = match c.level {
            Level::Ok => "ok  ",
            Level::Warn => "warn",
            Level::Fail => "FAIL",
        };
        writeln!(out, "{tag}  {:<11} {}", c.name, c.detail)?;
    }
    Ok(checks.iter().all(|c| c.level != Level::Fail))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(d: &Path) -> DoctorInput {
        DoctorInput {
            data_root: d.join("data"),
            home: d.join("home"),
            claude_config_dir: None,
            path_dirs: vec![d.join("bin")],
            assets: Assets::Dir(d.join("dist")),
            port: 0,
            probe: false,
        }
    }

    fn level(cs: &[Check], name: &str) -> Level {
        cs.iter().find(|c| c.name == name).unwrap().level
    }

    #[test]
    fn empty_machine_fails_on_accounts_only() {
        let d = tempfile::tempdir().unwrap();
        let cs = checks(&input(d.path()));
        assert_eq!(level(&cs, "data root"), Level::Ok);
        assert_eq!(level(&cs, "server"), Level::Ok);
        assert_eq!(level(&cs, "claude cli"), Level::Warn);
        assert_eq!(level(&cs, "accounts"), Level::Fail);
        assert_eq!(level(&cs, "web ui"), Level::Warn);
        let mut out = Vec::new();
        assert!(!report(&cs, &mut out).unwrap());
        let text = String::from_utf8(out).unwrap();
        assert!(text.contains("FAIL  accounts"), "{text}");
        assert!(!d.path().join("data/.doctor-probe").exists());
    }

    #[test]
    fn logged_in_cli_and_built_ui_pass() {
        let d = tempfile::tempdir().unwrap();
        let i = input(d.path());
        let claude = d.path().join("home/.claude");
        std::fs::create_dir_all(&claude).unwrap();
        std::fs::write(
            claude.join(".claude.json"),
            r#"{"oauthAccount":{"emailAddress":"me@x.com"}}"#,
        )
        .unwrap();
        std::fs::create_dir_all(d.path().join("bin")).unwrap();
        std::fs::write(d.path().join("bin/claude"), "").unwrap();
        std::fs::create_dir_all(d.path().join("dist")).unwrap();
        std::fs::write(d.path().join("dist/index.html"), "<html>").unwrap();
        let cs = checks(&i);
        assert_eq!(level(&cs, "claude cli"), Level::Ok);
        assert_eq!(level(&cs, "accounts"), Level::Ok);
        assert_eq!(level(&cs, "web ui"), Level::Ok);
        assert!(report(&cs, &mut Vec::new()).unwrap());
        // A stale lock is reported, not fatal.
        std::fs::write(
            lp_service::paths::server_lock(&i.data_root),
            r#"{"pid":1,"port":1,"tokenHash":null,"startedAt":0,"version":"x"}"#,
        )
        .unwrap();
        assert_eq!(level(&checks(&i), "server"), Level::Warn);
    }

    #[test]
    fn free_cli_lines_say_how_to_connect_and_when_limits_lift() {
        assert_eq!(human_secs(4 * 3600 + 12 * 60 + 5), "4h 12m");
        assert_eq!(human_secs(90), "1m");
        let missing = free_cli_check(&[], "kiro cli", "kiro", "kiro-cli");
        assert_eq!(missing.level, Level::Warn);
        assert!(
            missing.detail.contains("kiro-cli login"),
            "{}",
            missing.detail
        );
        let limited = accounts::Account {
            id: "opencode:opencode".into(),
            provider: "opencode",
            email: None,
            tier: None,
            config_dir: None,
            status: accounts::Status::Cooldown {
                secs_left: 5 * 3600,
                reason: "Free usage limit".into(),
            },
            resets_at: None,
            requests: 0,
            tokens_in: 0,
            tokens_out: 0,
            models: vec![],
        };
        let c = free_cli_check(&[limited], "opencode cli", "opencode", "opencode");
        assert_eq!(c.level, Level::Warn);
        assert!(c.detail.contains("resets in 5h 0m"), "{}", c.detail);
    }
}
