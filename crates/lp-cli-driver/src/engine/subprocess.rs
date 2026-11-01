use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde_json::Value;
use tokio::io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader, Lines};
use tokio::process::{Child, ChildStdout, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio::time::Instant;
use tokio_util::sync::CancellationToken;

use super::{
    DriverError, LlmEngine, StreamDelta, StructuredOutput, StructuredRequest, ToolCall,
    TurnRequest, TurnResponse,
};

const STDERR_CAP: usize = 64 * 1024;
const CALL_OPEN: &str = "<tool_call>";
const CALL_CLOSE: &str = "</tool_call>";
/// Env that real setups need (proxies, TLS roots, locale, temp dir); everything else is stripped.
const ENV_ALLOW: [&str; 19] = [
    "HOME",
    // The Claude CLI looks up per-profile Keychain credentials by user name.
    "USER",
    "LOGNAME",
    "PATH",
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
    "ALL_PROXY",
    "https_proxy",
    "http_proxy",
    "no_proxy",
    // Where opencode (and other XDG tools) keep config, data and cache; replaced for the sandboxed CLIs.
    "XDG_CONFIG_HOME",
    "XDG_DATA_HOME",
    "XDG_CACHE_HOME",
    "XDG_STATE_HOME",
    "SSL_CERT_FILE",
    "SSL_CERT_DIR",
    "TMPDIR",
    "LANG",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CliKind {
    Claude,
    Codex,
    Agy,
    /// `opencode run --format json` (free `opencode/*` models need no login).
    Opencode,
    /// `kiro-cli chat --no-interactive --output-format stream-json`.
    Kiro,
}

/// Tutor config for opencode: every tool permission denied, nothing that talks to the outside.
/// Needs `--standalone`, otherwise the already-running background service (with the user's config) answers.
const OPENCODE_LOCKDOWN: &str = r#"{"permission":{"*":"deny"},"autoupdate":false,"share":"disabled","mcp":{},"lsp":false,"formatter":false,"plugin":[]}"#;
/// Kiro agent with no tools, no MCP servers and no resources; selected with `--agent lp-tutor`.
const KIRO_AGENT: &str = r#"{"name":"lp-tutor","description":"LearnPuppy tutor: no tools","prompt":null,"mcpServers":{},"tools":[],"allowedTools":[],"toolsSettings":{},"resources":[],"includeMcpJson":false}"#;
const KIRO_AGENT_NAME: &str = "lp-tutor";
/// Model used when opencode is asked to run without one.
const OPENCODE_DEFAULT_MODEL: &str = "opencode/big-pickle";

/// Short stable log label for an account id like `claude:me@example.com`: provider plus 6 hex chars
/// of an FNV-1a hash, so the id itself never reaches the log.
pub fn account_label(id: &str) -> String {
    let provider = id.split_once(':').map_or(id, |(p, _)| p);
    let provider = if provider.len() == id.len() {
        "acct"
    } else {
        provider
    };
    let hash = id.bytes().fold(0xcbf2_9ce4_8422_2325_u64, |h, b| {
        (h ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3)
    });
    format!("{provider}-{:06x}", hash & 0xff_ffff)
}

/// Drives one agent CLI (`claude -p`, `codex exec`, `agy -p`) as a child process.
#[derive(Debug, Clone)]
pub struct SubprocessEngine {
    pub kind: CliKind,
    pub program: PathBuf,
    pub timeout: Duration,
    /// Claude only: `CLAUDE_CONFIG_DIR` selecting which account to use.
    pub config_dir: Option<PathBuf>,
    /// Model and effort used when a request names none (a role profile's choice).
    pub model: Option<String>,
    pub effort: Option<String>,
    /// Role and account id, for the call log only.
    pub role: String,
    pub account: String,
    /// Where `LEARNPUPPY_DEBUG_RAW=1` dumps each turn's redacted raw stdout; `None` = off.
    pub raw_dir: Option<PathBuf>,
    /// Input tokens, output tokens, and micro-credits (credits x 1e6, kiro only) the CLI reported for the
    /// calls made through this engine (and its clones).
    pub usage: Arc<[AtomicU64; 3]>,
    /// Private (0700, random name) working directory, removed when the last clone drops.
    sandbox: Arc<tempfile::TempDir>,
    /// Private HOME (kiro, opencode): the CLI sees no user skills, MCP servers, instructions or history,
    /// only symlinks to its own auth store. Removed with the last clone.
    home: Option<Arc<tempfile::TempDir>>,
}

impl SubprocessEngine {
    pub fn new(kind: CliKind) -> Result<Self, DriverError> {
        let program = match kind {
            CliKind::Claude => "claude",
            CliKind::Codex => "codex",
            CliKind::Agy => "agy",
            CliKind::Opencode => "opencode",
            CliKind::Kiro => "kiro-cli",
        };
        let home = match kind {
            CliKind::Kiro | CliKind::Opencode => {
                let dir = tempfile::Builder::new().prefix("lp-home-").tempdir()?;
                let real = std::env::var_os("HOME").map(PathBuf::from);
                if let Some(real) = &real {
                    if kind == CliKind::Kiro {
                        prepare_kiro_home(real, dir.path())?;
                    } else {
                        prepare_opencode_home(dir.path())?;
                    }
                }
                Some(Arc::new(dir))
            }
            _ => None,
        };
        Ok(Self {
            kind,
            program: program.into(),
            timeout: Duration::from_secs(600),
            config_dir: None,
            model: None,
            effort: None,
            role: String::new(),
            account: String::new(),
            raw_dir: raw_dir_from_env(),
            usage: Arc::new([AtomicU64::new(0), AtomicU64::new(0), AtomicU64::new(0)]),
            sandbox: Arc::new(tempfile::Builder::new().prefix("lp-sandbox-").tempdir()?),
            home,
        })
    }

    fn note_usage(&self, line: &str) {
        if let Some((i, o)) = usage_of(self.kind, line) {
            self.usage[0].fetch_add(i, Ordering::Relaxed);
            self.usage[1].fetch_add(o, Ordering::Relaxed);
        }
        if let Some(c) = credits_of(self.kind, line) {
            self.usage[2].fetch_add(c, Ordering::Relaxed);
        }
    }

    /// Opencode with a paid provider: expose the user's `auth.json` (symlink; a token the CLI rotates is
    /// then written through to the real file only if opencode rewrites in place). No-op for other CLIs.
    pub fn link_opencode_login(&self) -> std::io::Result<()> {
        let (Some(home), Some(real)) = (&self.home, std::env::var_os("HOME")) else {
            return Ok(());
        };
        let src = Path::new(&real).join(".local/share/opencode/auth.json");
        let link = home.path().join("data/opencode/auth.json");
        if self.kind == CliKind::Opencode && src.exists() && !link.exists() {
            std::os::unix::fs::symlink(src, link)?;
        }
        Ok(())
    }

    fn sandbox(&self) -> &Path {
        self.sandbox.path()
    }

    /// One info line per CLI call. The prompt travels on stdin and the schema is blanked, so neither is logged.
    fn log_start(&self, args: &[String]) -> std::time::Instant {
        let mut argv = args.to_vec();
        for i in 1..argv.len() {
            if argv[i - 1] == "--json-schema" {
                argv[i] = "<redacted>".into();
            }
        }
        let flag = |names: [&str; 2]| {
            args.windows(2)
                .find(|w| names.contains(&w[0].as_str()))
                .map_or("", |w| w[1].as_str())
                .to_owned()
        };
        tracing::info!(
            role = %self.role,
            provider = ?self.kind,
            account = %account_label(&self.account),
            model = %flag(["-m", "--model"]),
            effort = %flag(["--effort", "-c"]),
            argv = ?argv,
            "cli turn start"
        );
        std::time::Instant::now()
    }

    fn log_end<T>(&self, started: std::time::Instant, r: &Result<T, DriverError>) {
        let status = match r {
            Ok(_) => "ok".to_owned(),
            Err(DriverError::Exit { code, .. }) => format!("exit {code:?}"),
            Err(e) => format!("error: {e}"),
        };
        tracing::info!(
            role = %self.role,
            provider = ?self.kind,
            account = %account_label(&self.account),
            duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            status = %status,
            "cli turn end"
        );
    }

    fn spawn(
        &self,
        args: Vec<String>,
        stdin: Option<String>,
        cancel: CancellationToken,
    ) -> Result<Proc, DriverError> {
        let mut cmd = Command::new(&self.program);
        // env_clear strips nested-session vars (CLAUDECODE, CLAUDE_*, ...).
        // process_group(0): grandchildren die with the CLI when it is killed.
        cmd.args(args)
            .env_clear()
            .current_dir(self.sandbox())
            .process_group(0);
        let private_home = self.home.as_ref().map(|h| h.path());
        for key in ENV_ALLOW {
            // A private HOME replaces the user's; their XDG dirs would point back at the real config.
            if private_home.is_some() && (key == "HOME" || key.starts_with("XDG_")) {
                continue;
            }
            if let Some(v) = std::env::var_os(key) {
                cmd.env(key, v);
            }
        }
        if let Some(h) = private_home {
            cmd.env("HOME", h);
            if self.kind == CliKind::Opencode {
                cmd.env("XDG_CONFIG_HOME", h.join("config"))
                    .env("XDG_DATA_HOME", h.join("data"))
                    .env("XDG_CACHE_HOME", h.join("cache"))
                    .env("XDG_STATE_HOME", h.join("state"))
                    .env("OPENCODE_DISABLE_CLAUDE_CODE", "1")
                    .env("OPENCODE_CONFIG_CONTENT", OPENCODE_LOCKDOWN);
            }
        }
        // The default account lives in `~/.claude.json` + Keychain; exporting the default dir breaks it.
        let default_dir = std::env::var_os("HOME").map(|h| Path::new(&h).join(".claude"));
        if let Some(dir) = self
            .config_dir
            .as_ref()
            .filter(|d| Some(*d) != default_dir.as_ref())
        {
            cmd.env("CLAUDE_CONFIG_DIR", dir);
        }
        let mut child = cmd
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .spawn()?;
        let mut sin = child.stdin.take().expect("stdin piped");
        tokio::spawn(async move {
            if let Some(text) = stdin {
                let _ = sin.write_all(text.as_bytes()).await;
            }
        });
        let mut err = child.stderr.take().expect("stderr piped");
        // Drain to EOF so a chatty CLI never blocks on a full pipe; keep the first and last STDERR_CAP bytes.
        let stderr = tokio::spawn(async move {
            let (mut head, mut tail, mut buf) = (Vec::new(), Vec::new(), [0u8; 8192]);
            while let Ok(n) = err.read(&mut buf).await {
                if n == 0 {
                    break;
                }
                let room = STDERR_CAP.saturating_sub(head.len()).min(n);
                head.extend_from_slice(&buf[..room]);
                tail.extend_from_slice(&buf[room..n]);
                if tail.len() > STDERR_CAP {
                    tail.drain(..tail.len() - STDERR_CAP);
                }
            }
            head.extend_from_slice(&tail);
            String::from_utf8_lossy(&head).into_owned()
        });
        let lines = BufReader::new(child.stdout.take().expect("stdout piped")).lines();
        Ok(Proc {
            child,
            lines,
            stderr,
            cancel,
            deadline: Instant::now() + self.timeout,
            timeout: self.timeout,
        })
    }

    fn turn_args(&self, req: &TurnRequest) -> (Vec<String>, Option<String>) {
        let mut a: Vec<String> = Vec::new();
        match self.kind {
            CliKind::Claude => {
                a.extend(
                    [
                        "-p",
                        "--output-format",
                        "stream-json",
                        "--verbose",
                        "--strict-mcp-config",
                        "--tools",
                        "",
                    ]
                    .map(String::from),
                );
                a.extend(CLAUDE_ISOLATION.map(String::from));
            }
            CliKind::Codex => a.extend(self.codex_base()),
            CliKind::Agy => a.extend(AGY_STREAM.map(String::from)),
            CliKind::Opencode => a.extend(OPENCODE_BASE.map(String::from)),
            CliKind::Kiro => a.extend(KIRO_BASE.map(String::from)),
        }
        self.finish_args(
            a,
            &with_tools(&req.prompt, &req.tools),
            req.model.as_deref(),
            req.effort.as_deref(),
        )
    }

    fn codex_base(&self) -> Vec<String> {
        let mut a: Vec<String> = CODEX_BASE.map(String::from).to_vec();
        a.push(self.sandbox().display().to_string());
        a
    }

    /// Adds model and prompt delivery: stdin for every CLI (agy: one stream-json message), so large
    /// prompts never hit argv limits or `ps`. Real agy treats `-p -` as the literal prompt "-".
    fn finish_args(
        &self,
        mut a: Vec<String>,
        prompt: &str,
        model: Option<&str>,
        effort: Option<&str>,
    ) -> (Vec<String>, Option<String>) {
        let model = model.or(self.model.as_deref());
        let effort = effort.or(self.effort.as_deref());
        let model = match (self.kind, model) {
            (CliKind::Opencode, None) => Some(OPENCODE_DEFAULT_MODEL),
            (_, m) => m,
        };
        if let Some(m) = model {
            a.push(
                if matches!(self.kind, CliKind::Codex | CliKind::Opencode) {
                    "-m"
                } else {
                    "--model"
                }
                .into(),
            );
            a.push(m.into());
        }
        if let Some(e) = effort {
            match self.kind {
                CliKind::Codex => {
                    a.extend(["-c".into(), format!("model_reasoning_effort=\"{e}\"")]);
                }
                CliKind::Claude => a.extend(["--effort".into(), e.into()]),
                // agy slugs already encode effort (`gemini-4-argon-high`); only a bare slug takes the flag.
                CliKind::Agy if !model.is_some_and(slug_has_effort) => {
                    a.extend(["--effort".into(), e.into()]);
                }
                CliKind::Agy => {}
                // Neither exposes a portable effort knob (kiro rejects it on models without reasoning).
                CliKind::Opencode | CliKind::Kiro => {}
            }
        }
        match self.kind {
            CliKind::Opencode | CliKind::Kiro => (a, Some(prompt.into())),
            CliKind::Codex => {
                a.push("-".into());
                (a, Some(prompt.into()))
            }
            CliKind::Agy => (a, Some(agy_message(prompt))),
            CliKind::Claude => (a, Some(prompt.into())),
        }
    }
}

/// `~/.learnpuppy/logs/raw` (or under `$LEARNPUPPY_HOME`), only when `LEARNPUPPY_DEBUG_RAW=1`.
fn raw_dir_from_env() -> Option<PathBuf> {
    if std::env::var("LEARNPUPPY_DEBUG_RAW").as_deref() != Ok("1") {
        return None;
    }
    let root = std::env::var_os("LEARNPUPPY_HOME")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| Path::new(&h).join(".learnpuppy")))?;
    Some(root.join("logs").join("raw"))
}

/// agy reads NDJSON messages from stdin only with a stream-json output.
const AGY_STREAM: [&str; 5] = [
    "--input-format",
    "stream-json",
    "--output-format",
    "stream-json",
    "--sandbox",
];

/// Claude: ignore the user's `~/.claude` instructions, output style and settings (issue #413); only
/// project and local scope (the empty sandbox) load. Auth lives outside the settings sources.
const CLAUDE_ISOLATION: [&str; 2] = ["--setting-sources", "project,local"];

/// Codex: no user config (MCP servers, notify hooks, project trust), no session files.
///
/// Issue #414, what remains: `-s read-only` still lets the model *read* any file the user can (a real
/// turn ran `ls ~/.ssh`). Codex has no flag to narrow reads and a private `HOME` hangs its auth, so
/// this is a documented residual (docs/web/12-cli-isolation-and-failures.md); agy has the same class
/// of exposure behind `--sandbox`. kiro and opencode are tool-less, so they read nothing.
const CODEX_BASE: [&str; 8] = [
    "exec",
    "--json",
    "--skip-git-repo-check",
    "--ignore-user-config",
    "--ephemeral",
    "-s",
    "read-only",
    "-C",
];

const OPENCODE_BASE: [&str; 4] = ["run", "--standalone", "--format", "json"];

const KIRO_BASE: [&str; 7] = [
    "chat",
    "--no-interactive",
    "--agent",
    KIRO_AGENT_NAME,
    "--trust-tools=",
    "--output-format",
    "stream-json",
];

/// Kiro: a HOME with the tool-less agent, and symlinks to the CLI's own binaries and login store (a
/// directory symlink, so refreshed tokens are written to the real store). User skills, MCP config,
/// steering and session history stay out of reach.
fn prepare_kiro_home(real_home: &Path, dest: &Path) -> std::io::Result<()> {
    let agents = dest.join(".kiro/agents");
    std::fs::create_dir_all(&agents)?;
    std::fs::write(agents.join(format!("{KIRO_AGENT_NAME}.json")), KIRO_AGENT)?;
    for rel in [
        "Library/Application Support/kiro-cli",
        ".local/bin",
        ".local/share/kiro-cli",
        ".config/kiro-cli",
    ] {
        let src = real_home.join(rel);
        if src.exists() {
            let link = dest.join(rel);
            if let Some(parent) = link.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::os::unix::fs::symlink(&src, &link)?;
        }
    }
    Ok(())
}

/// Opencode: empty config/data/cache/state dirs. Free `opencode/*` models need no login; a provider
/// login is linked in on request by `SubprocessEngine::link_opencode_login`.
fn prepare_opencode_home(dest: &Path) -> std::io::Result<()> {
    for d in ["config", "data/opencode", "cache", "state"] {
        std::fs::create_dir_all(dest.join(d))?;
    }
    Ok(())
}

/// The one stdin line that makes agy run a turn on `prompt`.
fn agy_message(prompt: &str) -> String {
    format!(
        "{}\n",
        serde_json::json!({"event": "user", "message": {"content": prompt}})
    )
}

fn slug_has_effort(slug: &str) -> bool {
    slug.rsplit('-')
        .next()
        .is_some_and(|t| matches!(t, "low" | "medium" | "high" | "xhigh" | "max" | "ultra"))
}

struct Proc {
    child: Child,
    lines: Lines<BufReader<ChildStdout>>,
    stderr: JoinHandle<String>,
    cancel: CancellationToken,
    deadline: Instant,
    timeout: Duration,
}

impl Proc {
    /// Next stdout line; `None` after a clean exit. Kills and reaps the child on
    /// cancel or timeout, and reports a non-zero exit with its stderr.
    async fn next(&mut self) -> Result<Option<String>, DriverError> {
        let line = tokio::select! {
            biased;
            _ = self.cancel.cancelled() => Err(DriverError::Cancelled),
            _ = tokio::time::sleep_until(self.deadline) => Err(DriverError::Timeout(self.timeout)),
            l = self.lines.next_line() => l.map_err(DriverError::from),
        };
        match line {
            Ok(Some(l)) => Ok(Some(l)),
            Ok(None) => {
                let status = tokio::select! {
                    biased;
                    _ = self.cancel.cancelled() => Err(DriverError::Cancelled),
                    _ = tokio::time::sleep_until(self.deadline) => Err(DriverError::Timeout(self.timeout)),
                    s = self.child.wait() => s.map_err(DriverError::from),
                };
                let status = match status {
                    Ok(s) => s,
                    Err(e) => {
                        let _ = self.child.kill().await;
                        return Err(e);
                    }
                };
                if status.success() {
                    return Ok(None);
                }
                let stderr = (&mut self.stderr).await.unwrap_or_default();
                Err(DriverError::Exit {
                    code: status.code(),
                    stderr,
                })
            }
            Err(e) => {
                let _ = self.child.kill().await;
                Err(e)
            }
        }
    }
}

/// Appends the text/JSON tool-call protocol and the tool list to the prompt.
pub(super) fn with_tools(prompt: &str, tools: &[super::ToolDecl]) -> String {
    if tools.is_empty() {
        return prompt.into();
    }
    let mut out = format!(
        "{prompt}\n\nYou have no built-in tools. To call a tool, reply with exactly\n\
         {CALL_OPEN}{{\"name\":\"<tool>\",\"input\":{{...}}}}{CALL_CLOSE}\n\
         and nothing after it; the result arrives in the next turn. Available tools:\n"
    );
    for t in tools {
        out.push_str(&format!("- {}: {}\n", t.name, t.usage));
    }
    out
}

/// Parses JSON that is only missing its closing quote, brackets or braces.
fn repair_json(raw: &str) -> Option<Value> {
    let raw = raw.trim();
    if let Ok(v) = serde_json::from_str(raw) {
        return Some(v);
    }
    let (mut stack, mut in_str, mut esc) = (Vec::new(), false, false);
    for c in raw.chars() {
        match (in_str, esc, c) {
            (true, true, _) => esc = false,
            (true, false, '\\') => esc = true,
            (true, false, '"') => in_str = false,
            (true, ..) => {}
            (false, _, '"') => in_str = true,
            (false, _, '{') => stack.push('}'),
            (false, _, '[') => stack.push(']'),
            (false, _, '}' | ']') => {
                stack.pop();
            }
            _ => {}
        }
    }
    let mut fixed = raw.to_owned();
    if in_str {
        fixed.push('"');
    }
    fixed.extend(stack.iter().rev());
    serde_json::from_str(&fixed).ok()
}

/// Splits `<tool_call>{json}</tool_call>` blocks out of model text. Tags match case-insensitively,
/// a missing closing tag is fine once the JSON object is complete; anything else stays text.
pub(super) fn split_calls(text: String) -> Vec<StreamDelta> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let (mut out, mut rest) = (Vec::new(), text.as_str());
    while let Some(start) = rest.to_ascii_lowercase().find(CALL_OPEN) {
        let body = rest[start + CALL_OPEN.len()..].trim_start();
        let mut objs = serde_json::Deserializer::from_str(body).into_iter::<Value>();
        let parsed = objs
            .next()
            .and_then(Result::ok)
            .filter(|v| v.get("name").and_then(Value::as_str).is_some());
        let (v, after) = if let Some(v) = parsed {
            let after = body[objs.byte_offset()..].trim_start();
            let after = match after.get(..CALL_CLOSE.len()) {
                Some(tag) if tag.eq_ignore_ascii_case(CALL_CLOSE) => &after[CALL_CLOSE.len()..],
                _ => after,
            };
            (v, after)
        } else if let Some(end) = body.to_ascii_lowercase().find(CALL_CLOSE) {
            // A closed block with broken JSON (models drop a trailing brace): repair it, or drop it;
            // the learner must never see raw tool-call text (it carries the answer key).
            let after = &body[end + CALL_CLOSE.len()..];
            match repair_json(&body[..end])
                .filter(|v| v.get("name").and_then(Value::as_str).is_some())
            {
                Some(v) => (v, after),
                None => {
                    if start > 0 {
                        out.push(StreamDelta::Text(rest[..start].into()));
                    }
                    rest = after;
                    continue;
                }
            }
        } else {
            break;
        };
        if start > 0 {
            out.push(StreamDelta::Text(rest[..start].into()));
        }
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        out.push(StreamDelta::ToolCall(ToolCall {
            id: v
                .get("id")
                .and_then(Value::as_str)
                .map_or(format!("call_{n}"), String::from),
            name: v["name"].as_str().unwrap_or_default().into(),
            input: v.get("input").cloned().unwrap_or(Value::Null),
        }));
        rest = after;
    }
    if !rest.is_empty() {
        out.push(StreamDelta::Text(rest.into()));
    }
    out
}

/// Stream deltas carried by one stdout line; text is scanned for protocol tool calls.
fn deltas(kind: CliKind, line: &str) -> Result<Vec<StreamDelta>, DriverError> {
    Ok(raw_deltas(kind, line)?
        .into_iter()
        .flat_map(|d| match d {
            StreamDelta::Text(t) => split_calls(t),
            d => vec![d],
        })
        .collect())
}

/// The answer text of agy's final JSON (`{"status","response"}`, the older `result` string, or the
/// `{"event":"result","result":{..}}` stream-json line); `status` other than SUCCESS is an error.
fn agy_text(v: &Value) -> Result<Option<String>, DriverError> {
    let body = match v.get("result") {
        Some(o) if o.is_object() => o,
        _ => v,
    };
    let str_of = |k: &str| body.get(k).and_then(Value::as_str);
    if let Some(status) = str_of("status").filter(|s| *s != "SUCCESS") {
        let err = str_of("error").filter(|e| !e.is_empty()).unwrap_or(status);
        return Err(DriverError::Failed(format!("agy {status}: {err}")));
    }
    Ok(str_of("response")
        .or_else(|| str_of("result"))
        .map(String::from))
}

/// `(input, output)` tokens from a CLI's final stats line: claude `{"type":"result","usage":{..}}` (cache
/// reads and writes count as input) codex `{"type":"turn.completed","usage":{..}}` or agy's final `result` event.
fn usage_of(kind: CliKind, line: &str) -> Option<(u64, u64)> {
    let v: Value = serde_json::from_str(line).ok()?;
    let u = match kind {
        CliKind::Claude if v.get("type")?.as_str()? == "result" => v.get("usage")?,
        CliKind::Codex if v.get("type")?.as_str()? == "turn.completed" => v.get("usage")?,
        // agy: `{"event":"result","result":{..,"usage":{..}}}`
        CliKind::Agy if v.get("event")?.as_str()? == "result" => v.get("result")?.get("usage")?,
        // opencode: `{"type":"step_finish","part":{"tokens":{input,output,reasoning,cache:{read,write}}}}`
        CliKind::Opencode if v.get("type")?.as_str()? == "step_finish" => {
            let t = v.get("part")?.get("tokens")?;
            let n = |o: &Value, k: &str| o.get(k).and_then(Value::as_u64).unwrap_or(0);
            let cache = t.get("cache").cloned().unwrap_or(Value::Null);
            return Some((
                n(t, "input") + n(&cache, "read") + n(&cache, "write"),
                n(t, "output") + n(t, "reasoning"),
            ));
        }
        _ => return None,
    };
    let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
    let input = n("input_tokens") + n("cache_creation_input_tokens") + n("cache_read_input_tokens");
    Some((input, n("output_tokens")))
}

/// Kiro reports credits, not tokens: micro-credits (credits x 1e6) from the `metadata` event
/// (`meteringUsage`, what the tool-less agent run emits) or the turn-completion summary
/// (`promptTurnSummaries`). A run emits one of them, never both.
fn credits_of(kind: CliKind, line: &str) -> Option<u64> {
    if kind != CliKind::Kiro
        || !(line.contains("meteringUsage") || line.contains("promptTurnSummaries"))
    {
        return None;
    }
    let v: Value = serde_json::from_str(line).ok()?;
    let sum = |a: Option<&Value>, key: &str| -> f64 {
        a.and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|m| m.get(key)?.as_f64())
            .sum()
    };
    let credits = sum(v["data"].get("meteringUsage"), "value")
        + sum(
            v["data"]["update"]["_meta"]["kiro"].get("promptTurnSummaries"),
            "usage",
        );
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    (credits > 0.0).then(|| (credits * 1e6).round() as u64)
}

/// The failure a CLI reported on stdout (error event or `is_error` result), as text for `classify`.
fn error_of(kind: CliKind, line: &str) -> Option<String> {
    let v: Value = serde_json::from_str(line).ok()?;
    let ty = v.get("type").and_then(Value::as_str);
    let s = |p: &Value| p.as_str().map(String::from);
    match (kind, ty) {
        (CliKind::Claude, Some("result")) if v.get("is_error") == Some(&Value::Bool(true)) => {
            let status = v
                .get("api_error_status")
                .and_then(Value::as_u64)
                .map_or(String::new(), |c| format!(" (status {c})"));
            Some(format!("{}{status}", s(&v["result"]).unwrap_or_default()))
        }
        (CliKind::Codex, Some("turn.failed")) => s(&v["error"]["message"]),
        (CliKind::Codex, Some("error")) => s(&v["message"]),
        (CliKind::Opencode, Some("error")) => {
            let e = &v["error"];
            let mut m = format!(
                "{}: {}",
                s(&e["type"]).unwrap_or_default(),
                s(&e["message"]).unwrap_or_default()
            );
            if let Some(st) = e.get("status").and_then(Value::as_u64) {
                m.push_str(&format!(" (status {st})"));
            }
            if let Some(ms) = e.get("retryAfterMs").and_then(Value::as_u64) {
                m.push_str(&format!(" retryAfterMs: {ms}"));
            }
            Some(m)
        }
        (CliKind::Kiro, Some("runError")) => s(&v["data"]["message"]),
        (CliKind::Kiro, Some("runFinished"))
            if v["data"]["status"]
                .as_str()
                .is_some_and(|st| st != "success") =>
        {
            Some(format!("kiro run {}", v["data"]["status"]))
        }
        _ => None,
    }
}

/// Final answer text some CLIs repeat at the end of the stream (used only if no chunk carried text).
fn final_text_of(kind: CliKind, line: &str) -> Option<String> {
    if kind != CliKind::Kiro || !line.contains("finalText") {
        return None;
    }
    let v: Value = serde_json::from_str(line).ok()?;
    (v.get("type")?.as_str()? == "runFinished")
        .then(|| v["data"]["finalText"].as_str().map(String::from))
        .flatten()
}

fn raw_deltas(kind: CliKind, line: &str) -> Result<Vec<StreamDelta>, DriverError> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return Ok(match kind {
            CliKind::Agy if !line.is_empty() => vec![StreamDelta::Text(line.into())],
            _ => vec![],
        });
    };
    let str_of = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(String::from);
    Ok(match (kind, v.get("type").and_then(Value::as_str)) {
        // A synthetic assistant message is the CLI's own error notice, not the model; `error_of` reports it.
        (CliKind::Claude, Some("assistant")) if v["message"]["model"] == "<synthetic>" => vec![],
        (CliKind::Claude, Some("assistant")) => v["message"]["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| match b.get("type")?.as_str()? {
                "text" => Some(StreamDelta::Text(str_of(b, "text")?)),
                "thinking" => Some(StreamDelta::Thinking(str_of(b, "thinking")?)),
                "tool_use" => Some(StreamDelta::ToolCall(ToolCall {
                    id: str_of(b, "id")?,
                    name: str_of(b, "name")?,
                    input: b.get("input").cloned().unwrap_or(Value::Null),
                })),
                _ => None,
            })
            .collect(),
        (CliKind::Codex, Some("item.completed")) => {
            let item = &v["item"];
            match (
                item.get("type").and_then(Value::as_str),
                str_of(item, "text"),
            ) {
                (Some("agent_message"), Some(t)) => vec![StreamDelta::Text(t)],
                (Some("reasoning"), Some(t)) => vec![StreamDelta::Thinking(t)],
                _ => vec![],
            }
        }
        (CliKind::Opencode, Some("text")) => str_of(&v["part"], "text")
            .map(StreamDelta::Text)
            .into_iter()
            .collect(),
        (CliKind::Opencode, Some("reasoning")) => str_of(&v["part"], "text")
            .map(StreamDelta::Thinking)
            .into_iter()
            .collect(),
        (CliKind::Kiro, Some("sessionUpdate")) => {
            let u = &v["data"]["update"];
            let text = || str_of(&u["content"], "text");
            match u.get("sessionUpdate").and_then(Value::as_str) {
                Some("agent_message_chunk") => text().map(StreamDelta::Text).into_iter().collect(),
                Some("agent_thought_chunk") => {
                    text().map(StreamDelta::Thinking).into_iter().collect()
                }
                _ => vec![],
            }
        }
        (CliKind::Agy, _) => agy_text(&v)?
            .filter(|t| !t.is_empty())
            .map(StreamDelta::Text)
            .into_iter()
            .collect(),
        _ => vec![],
    })
}

/// The JSON value in a model reply that may wrap it in prose or a code fence, repairing a cut-off tail.
fn json_in_text(text: &str) -> Option<Value> {
    let start = text.find(['{', '['])?;
    let body = &text[start..];
    let end = body.rfind(['}', ']']).map_or(body.len(), |i| i + 1);
    serde_json::from_str(&body[..end])
        .ok()
        .or_else(|| repair_json(body.trim_end_matches('`')))
}

/// Claude's final `result` line carried a stop reason.
fn claude_stop_reason(line: &str) -> bool {
    line.contains("\"result\"")
        && serde_json::from_str::<Value>(line).is_ok_and(|v| {
            v.get("type").and_then(Value::as_str) == Some("result")
                && v.get("stop_reason").is_some_and(|s| !s.is_null())
        })
}

/// The structured JSON payload out of a finished run's stdout.
fn structured_json(kind: CliKind, stdout: &str) -> Result<Value, DriverError> {
    let bad = |e: serde_json::Error| DriverError::Parse(e.to_string());
    match kind {
        CliKind::Codex => {
            let last = stdout
                .lines()
                .map(|l| deltas(kind, l))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .filter_map(|d| match d {
                    StreamDelta::Text(t) => Some(t),
                    _ => None,
                })
                .next_back()
                .ok_or_else(|| DriverError::Parse("no agent message".into()))?;
            serde_json::from_str(&last).map_err(bad)
        }
        CliKind::Agy => {
            let mut last = None;
            for v in stdout.lines().filter_map(|l| serde_json::from_str(l).ok()) {
                last = agy_text(&v)?.or(last);
            }
            let r = last.ok_or_else(|| DriverError::Parse("no agy response".into()))?;
            serde_json::from_str(&r).map_err(bad)
        }
        CliKind::Opencode | CliKind::Kiro => {
            let text: String = stdout
                .lines()
                .map(|l| raw_deltas(kind, l))
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .flatten()
                .filter_map(|d| match d {
                    StreamDelta::Text(t) => Some(t),
                    _ => None,
                })
                .collect();
            let text = if text.trim().is_empty() {
                stdout
                    .lines()
                    .rev()
                    .find_map(|l| final_text_of(kind, l))
                    .unwrap_or_default()
            } else {
                text
            };
            json_in_text(&text).ok_or_else(|| DriverError::Parse("no JSON in reply".into()))
        }
        CliKind::Claude => {
            let v: Value = serde_json::from_str(stdout.trim()).map_err(bad)?;
            if let Some(s) = v.get("structured_output") {
                return Ok(s.clone());
            }
            let r = v
                .get("result")
                .and_then(Value::as_str)
                .ok_or_else(|| DriverError::Parse("no structured_output or result".into()))?;
            serde_json::from_str(r).map_err(bad)
        }
    }
}

/// What a run printed that the learner must not see: the CLI's own reason for failing.
fn adopt_cli_error(err: DriverError, cli_error: &Option<String>) -> DriverError {
    match (err, cli_error) {
        (DriverError::Exit { stderr, .. }, Some(m)) => DriverError::Cli {
            message: format!("{m}\n{stderr}"),
        },
        // A hung offline CLI (codex "waiting for network") ends as a timeout; report the cause.
        (DriverError::Timeout(_), Some(m))
            if matches!(super::failure::classify(m, 0), super::FailureKind::Offline) =>
        {
            DriverError::Cli { message: m.clone() }
        }
        (e, _) => e,
    }
}

impl SubprocessEngine {
    async fn run_turn(
        &self,
        args: Vec<String>,
        stdin: Option<String>,
        event_tx: mpsc::Sender<StreamDelta>,
        cancel: CancellationToken,
        // Kiro streams text in small chunks; with tools in play they are joined before the
        // `<tool_call>` scan so a call split across chunks is not lost.
        join_chunks: bool,
    ) -> Result<TurnResponse, DriverError> {
        let mut proc = self.spawn(args, stdin, cancel)?;
        let (mut resp, mut raw) = (TurnResponse::default(), String::new());
        let (mut cli_error, mut final_text, mut pending) = (None, None, String::new());
        let mut claude_stopped = false;
        let ran: Result<(), DriverError> = async {
            while let Some(line) = proc.next().await? {
                self.keep_raw(&mut raw, &line);
                self.note_usage(&line);
                if let Some(e) = error_of(self.kind, &line) {
                    cli_error = Some(e);
                }
                if let Some(t) = final_text_of(self.kind, &line) {
                    final_text = Some(t);
                }
                if self.kind == CliKind::Claude && claude_stop_reason(&line) {
                    claude_stopped = true;
                }
                let ds = if join_chunks && self.kind == CliKind::Kiro {
                    let mut rest = Vec::new();
                    for d in raw_deltas(self.kind, &line)? {
                        match d {
                            StreamDelta::Text(t) => pending.push_str(&t),
                            d => rest.push(d),
                        }
                    }
                    rest
                } else {
                    deltas(self.kind, &line)?
                };
                for d in ds {
                    Self::keep(&mut resp, &d);
                    event_tx.send(d).await.map_err(|_| DriverError::Cancelled)?;
                }
            }
            let mut tail = split_calls(std::mem::take(&mut pending));
            if resp.text.trim().is_empty() && resp.tool_calls.is_empty() && tail.is_empty() {
                tail = final_text.take().map(split_calls).unwrap_or_default();
            }
            for d in tail {
                Self::keep(&mut resp, &d);
                event_tx.send(d).await.map_err(|_| DriverError::Cancelled)?;
            }
            Ok(())
        }
        .await;
        self.dump_raw(&raw);
        ran.map_err(|e| adopt_cli_error(e, &cli_error))?;
        let empty = resp.text.trim().is_empty() && resp.tool_calls.is_empty();
        if let (true, Some(message)) = (empty, cli_error) {
            return Err(DriverError::Cli { message });
        }
        if empty {
            // Claude cut off before it said why it stopped: transient, worth one more try.
            return Err(if self.kind == CliKind::Claude && !claude_stopped {
                DriverError::NoStopReason
            } else {
                DriverError::EmptyResponse
            });
        }
        Ok(resp)
    }

    fn keep(resp: &mut TurnResponse, d: &StreamDelta) {
        match d {
            StreamDelta::Text(t) => resp.text.push_str(t),
            StreamDelta::ToolCall(c) => resp.tool_calls.push(c.clone()),
            StreamDelta::Thinking(_) => {}
        }
    }

    /// One structured run: stdout lines joined, plus the CLI's own error event if it printed one.
    async fn run_structured(
        &self,
        args: Vec<String>,
        stdin: Option<String>,
        cancel: CancellationToken,
    ) -> Result<(String, Option<String>), DriverError> {
        let mut proc = self.spawn(args, stdin, cancel)?;
        let (mut stdout, mut cli_error) = (String::new(), None);
        let read = async {
            while let Some(line) = proc.next().await? {
                self.note_usage(&line);
                if let Some(e) = error_of(self.kind, &line) {
                    cli_error = Some(e);
                }
                stdout.push_str(&line);
                stdout.push('\n');
            }
            Ok(())
        }
        .await;
        self.dump_raw(&stdout);
        read.map_err(|e| adopt_cli_error(e, &cli_error))?;
        if stdout.trim().is_empty() {
            return Err(DriverError::EmptyResponse);
        }
        Ok((stdout, cli_error))
    }

    fn keep_raw(&self, raw: &mut String, line: &str) {
        if self.raw_dir.is_some() {
            raw.push_str(line);
            raw.push('\n');
        }
    }

    /// `LEARNPUPPY_DEBUG_RAW=1`: the turn's stdout, redacted, as `<date>-<role>-<n>.jsonl` (0600).
    fn dump_raw(&self, raw: &str) {
        use std::os::unix::fs::OpenOptionsExt;
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);
        let Some(dir) = &self.raw_dir else { return };
        let date = chrono::Utc::now().format("%Y-%m-%d");
        let role = if self.role.is_empty() {
            "cli"
        } else {
            &self.role
        };
        let body = lp_core::redact::redact(raw);
        let written = std::fs::create_dir_all(dir).and_then(|()| loop {
            let n = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            let opened = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(dir.join(format!("{date}-{role}-{n}.jsonl")));
            match opened {
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                f => break f.and_then(|mut f| std::io::Write::write_all(&mut f, body.as_bytes())),
            }
        });
        if let Err(e) = written {
            tracing::warn!("raw turn log not written: {e}");
        }
    }
}

#[async_trait]
impl LlmEngine for SubprocessEngine {
    async fn stream_turn(
        &self,
        req: TurnRequest,
        event_tx: mpsc::Sender<StreamDelta>,
        cancel: CancellationToken,
    ) -> Result<TurnResponse, DriverError> {
        let (args, stdin) = self.turn_args(&req);
        let started = self.log_start(&args);
        let join = !req.tools.is_empty();
        let mut r = self
            .run_turn(
                args.clone(),
                stdin.clone(),
                event_tx.clone(),
                cancel.clone(),
                join,
            )
            .await;
        // An empty turn streamed nothing, so one retry cannot repeat text the learner saw (#416).
        if matches!(
            r,
            Err(DriverError::EmptyResponse | DriverError::NoStopReason)
        ) {
            tracing::warn!(role = %self.role, "empty cli turn, retrying once");
            r = self.run_turn(args, stdin, event_tx, cancel, join).await;
        }
        self.log_end(started, &r);
        r
    }

    async fn complete_structured<T: DeserializeOwned + JsonSchema + Send>(
        &self,
        req: StructuredRequest,
        cancel: CancellationToken,
    ) -> Result<StructuredOutput<T>, DriverError> {
        let schema = serde_json::to_string(&schemars::schema_for!(T))
            .map_err(|e| DriverError::Parse(e.to_string()))?;
        // Held until the child exits so codex can read `--output-schema <file>`.
        let mut schema_file = None;
        let mut a: Vec<String> = Vec::new();
        let mut prompt = req.prompt.clone();
        match self.kind {
            CliKind::Claude => {
                a.extend(
                    [
                        "-p",
                        "--output-format",
                        "json",
                        "--strict-mcp-config",
                        "--tools",
                        "",
                        "--json-schema",
                    ]
                    .map(String::from),
                );
                a.push(schema);
                a.extend(CLAUDE_ISOLATION.map(String::from));
            }
            CliKind::Codex => {
                let mut f = tempfile::Builder::new()
                    .suffix(".json")
                    .tempfile_in(self.sandbox())?;
                std::io::Write::write_all(&mut f, schema.as_bytes())?;
                a.extend(self.codex_base());
                a.extend(["--output-schema".into(), f.path().display().to_string()]);
                schema_file = Some(f);
            }
            CliKind::Agy => {
                a.extend(AGY_STREAM.map(String::from));
                a.extend(["--json-schema".into(), schema]);
            }
            // No native schema flag: the schema rides in the prompt and the reply is parsed leniently.
            CliKind::Opencode | CliKind::Kiro => {
                a.extend(
                    if self.kind == CliKind::Kiro {
                        KIRO_BASE.to_vec()
                    } else {
                        OPENCODE_BASE.to_vec()
                    }
                    .into_iter()
                    .map(String::from),
                );
                prompt = format!(
                    "{prompt}\n\nReply with ONLY one JSON value that validates against this JSON Schema. \
                     No prose, no code fences.\nSchema: {schema}"
                );
            }
        }
        let (args, stdin) = self.finish_args(a, &prompt, req.model.as_deref(), None);
        let started = self.log_start(&args);
        let mut ran = self
            .run_structured(args.clone(), stdin.clone(), cancel.clone())
            .await;
        if matches!(ran, Err(DriverError::EmptyResponse)) {
            tracing::warn!(role = %self.role, "empty structured cli turn, retrying once");
            ran = self.run_structured(args, stdin, cancel).await;
        }
        self.log_end(started, &ran);
        drop(schema_file);
        let (stdout, cli_error) = ran?;
        let raw = match structured_json(self.kind, &stdout) {
            Ok(v) => v,
            Err(_) if cli_error.is_some() => {
                return Err(DriverError::Cli {
                    message: cli_error.unwrap_or_default(),
                })
            }
            Err(e) => return Err(e),
        };
        let value =
            serde_json::from_value(raw.clone()).map_err(|e| DriverError::Parse(e.to_string()))?;
        Ok(StructuredOutput { value, raw })
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    #[test]
    fn account_label_is_provider_plus_short_hash_without_the_id() {
        let l = account_label("claude:me@example.com");
        assert!(
            l.starts_with("claude-") && l.len() == "claude-".len() + 6,
            "{l}"
        );
        assert!(!l.contains("example") && !l.contains('@'));
        assert_eq!(l, account_label("claude:me@example.com"));
        assert_ne!(l, account_label("claude:you@example.com"));
        assert_eq!(account_label("agy:default").split('-').next(), Some("agy"));
    }

    /// Writes an executable mock CLI and an engine pointing at it.
    fn mock(kind: CliKind, script: &str) -> (tempfile::TempDir, SubprocessEngine) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("mock-cli");
        std::fs::write(&path, format!("#!/bin/sh\n{script}\n")).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).unwrap();
        let mut e = SubprocessEngine::new(kind).unwrap();
        e.program = path;
        (dir, e)
    }

    fn pid_alive(pid: &str) -> bool {
        std::process::Command::new("kill")
            .args(["-0", pid])
            .stderr(Stdio::null())
            .status()
            .unwrap()
            .success()
    }

    const CLAUDE_STREAM: &str = r#"cat >/dev/null
echo '{"type":"system"}'
echo '{"type":"assistant","message":{"content":[{"type":"thinking","thinking":"hm"},{"type":"text","text":"hi "},{"type":"tool_use","id":"t1","name":"probe","input":{"q":1}}]}}'
echo '{"type":"assistant","message":{"content":[{"type":"text","text":"there"}]}}'
echo '{"type":"result","result":"hi there"}'"#;

    #[test]
    fn tool_call_with_a_missing_brace_is_repaired_and_unparseable_ones_are_dropped() {
        let broken = r#"hi <tool_call>{"name":"probe","input":{"question":"q","correctAnswer":"heap"}</tool_call> bye"#;
        let d = split_calls(broken.into());
        assert!(
            matches!(&d[1], StreamDelta::ToolCall(c) if c.name == "probe" && c.input["correctAnswer"] == "heap")
        );
        let garbage = "x <tool_call>{nope</tool_call> y";
        let d = split_calls(garbage.into());
        let text: String = d
            .iter()
            .filter_map(|d| match d {
                StreamDelta::Text(t) => Some(t.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(text, "x  y");
        assert!(!d.iter().any(|d| matches!(d, StreamDelta::ToolCall(_))));
    }

    #[test]
    fn usage_is_read_from_claude_result_and_codex_turn_completed() {
        let claude = r#"{"type":"result","usage":{"input_tokens":5,"cache_creation_input_tokens":10,"cache_read_input_tokens":100,"output_tokens":7}}"#;
        assert_eq!(usage_of(CliKind::Claude, claude), Some((115, 7)));
        let codex = r#"{"type":"turn.completed","usage":{"input_tokens":24573,"cached_input_tokens":12416,"output_tokens":5}}"#;
        assert_eq!(usage_of(CliKind::Codex, codex), Some((24573, 5)));
        assert_eq!(usage_of(CliKind::Claude, codex), None);
        assert_eq!(usage_of(CliKind::Agy, claude), None);
        let agy = r#"{"event":"result","result":{"status":"SUCCESS","usage":{"input_tokens":40,"output_tokens":3}}}"#;
        assert_eq!(usage_of(CliKind::Agy, agy), Some((40, 3)));
        assert_eq!(usage_of(CliKind::Claude, "not json"), None);
    }

    #[tokio::test]
    async fn claude_stream_json_becomes_deltas() {
        let (_d, e) = mock(CliKind::Claude, CLAUDE_STREAM);
        let (tx, mut rx) = mpsc::channel(16);
        let resp = e
            .stream_turn(
                TurnRequest {
                    prompt: "p".into(),
                    model: None,
                    effort: None,
                    tools: vec![],
                },
                tx,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(resp.text, "hi there");
        assert_eq!(resp.tool_calls[0].name, "probe");
        assert_eq!(rx.recv().await, Some(StreamDelta::Thinking("hm".into())));
    }

    #[tokio::test]
    async fn codex_events_become_deltas() {
        let (_d, e) = mock(
            CliKind::Codex,
            r#"cat >/dev/null
echo '{"type":"item.completed","item":{"type":"reasoning","text":"think"}}'
echo '{"type":"item.completed","item":{"type":"agent_message","text":"done"}}'"#,
        );
        let (tx, _rx) = mpsc::channel(16);
        let resp = e
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(resp.text, "done");
    }

    #[derive(serde::Deserialize, schemars::JsonSchema, PartialEq, Debug)]
    struct Out {
        n: u32,
    }

    #[tokio::test]
    async fn structured_claude_and_codex() {
        let (_d, e) = mock(
            CliKind::Claude,
            r#"cat >/dev/null; echo '{"type":"result","structured_output":{"n":3}}'"#,
        );
        let out: StructuredOutput<Out> = e
            .complete_structured(StructuredRequest::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(out.value, Out { n: 3 });

        // codex must be handed a readable schema file via --output-schema
        let (_d, e) = mock(
            CliKind::Codex,
            r#"cat >/dev/null
while [ "$1" != "--output-schema" ]; do shift; done
test -s "$2" || exit 9
echo '{"type":"item.completed","item":{"type":"agent_message","text":"{\"n\":4}"}}'"#,
        );
        let out: StructuredOutput<Out> = e
            .complete_structured(StructuredRequest::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(out.value, Out { n: 4 });
    }

    #[tokio::test]
    async fn nonzero_exit_reports_stderr() {
        let (_d, e) = mock(CliKind::Claude, "echo rate limited >&2; exit 3");
        let (tx, _rx) = mpsc::channel(1);
        let err = e
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap_err();
        assert!(
            matches!(err, DriverError::Exit { code: Some(3), ref stderr } if stderr.contains("rate limited"))
        );
    }

    #[tokio::test]
    async fn env_is_stripped_and_cwd_is_sandbox() {
        let (_d, e) = mock(
            CliKind::Agy,
            r#"echo "{\"result\":\"[${CLAUDECODE:-unset}] $(pwd -P)\"}""#,
        );
        std::env::set_var("CLAUDECODE", "1");
        let (tx, _rx) = mpsc::channel(4);
        let resp = e
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap();
        assert!(resp.text.starts_with("[unset]"), "{}", resp.text);
        assert!(resp.text.contains("lp-sandbox-"), "{}", resp.text);
    }

    /// Real agy 1.4.8: `-p -` takes "-" as the prompt and ignores stdin; only `-p <text>` or a
    /// stream-json message on stdin reach the model.
    const REAL_AGY: &str = r#"case "$*" in
*"--input-format stream-json"*)
  in=$(cat)
  n=$(printf %s "$in" | wc -c | tr -d ' ')
  case "$in" in
    '{"event":"user","message":{"content":"'*) ok=yes;;
    *) ok=no;;
  esac
  case "$in" in *END'"}}') end=yes;; *) end=no;; esac
  echo '{"event":"init","init":{"cwd":"."}}'
  echo '{"event":"step_update","step_update":{"step_type":"agent_response","text_delta":"partial"}}'
  printf '{"event":"result","result":{"status":"SUCCESS","response":"argc=%s bytes=%s shape=%s end=%s"}}\n' "$#" "$n" "$ok" "$end";;
*"-p -"*)
  echo '{"status":"SUCCESS","response":"How can I help you today?\n"}';;
*)
  echo '{"status":"ERROR","response":"","error":"no prompt"}'; exit 1;;
esac"#;

    #[tokio::test]
    async fn agy_prompt_goes_over_stdin_as_a_stream_json_message() {
        let (_d, e) = mock(CliKind::Agy, REAL_AGY);
        let (tx, _rx) = mpsc::channel(8);
        let resp = e
            .stream_turn(
                TurnRequest {
                    prompt: format!("{}END", "x".repeat(1_000_000)),
                    ..TurnRequest::default()
                },
                tx,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(resp.text.contains("argc=5 "), "{}", resp.text);
        assert!(resp.text.contains("shape=yes end=yes"), "{}", resp.text);
        assert!(!resp.text.contains("help you"), "{}", resp.text);
        let bytes: usize = resp.text.split("bytes=").nth(1).unwrap()[..7]
            .parse()
            .unwrap();
        assert!(bytes > 1_000_000);
    }

    #[test]
    fn agy_argv_never_uses_dash_p_dash() {
        let e = SubprocessEngine::new(CliKind::Agy).unwrap();
        let (a, stdin) = e.turn_args(&TurnRequest::default());
        assert!(
            !a.contains(&"-p".into()) && !a.contains(&"-".into()),
            "{a:?}"
        );
        assert!(a
            .join(" ")
            .contains("--input-format stream-json --output-format stream-json"));
        assert!(stdin
            .unwrap()
            .starts_with(r#"{"event":"user","message":{"content":"#));
    }

    #[tokio::test]
    async fn agy_structured_reads_the_stream_result_response() {
        let (_d, e) = mock(
            CliKind::Agy,
            r#"cat >/dev/null
echo '{"event":"step_update","step_update":{"step_type":"agent_response"}}'
echo '{"event":"result","result":{"status":"SUCCESS","response":"{\"n\":7}"}}'"#,
        );
        let out: StructuredOutput<Out> = e
            .complete_structured(StructuredRequest::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(out.value, Out { n: 7 });
    }

    #[test]
    fn agy_real_json_response_is_read_and_result_stays_a_fallback() {
        let real = include_str!("fixtures/agy_json.txt").trim();
        assert_eq!(
            deltas(CliKind::Agy, real).unwrap(),
            vec![StreamDelta::Text("How can I help you today?\n".into())]
        );
        assert_eq!(
            deltas(CliKind::Agy, r#"{"result":"old"}"#).unwrap(),
            vec![StreamDelta::Text("old".into())]
        );
        let stream = r#"{"event":"result","result":{"status":"SUCCESS","response":"hi"}}"#;
        assert_eq!(
            deltas(CliKind::Agy, stream).unwrap(),
            vec![StreamDelta::Text("hi".into())]
        );
        for ignored in [
            r#"{"event":"init","conversation_id":"c","init":{"tools":[]}}"#,
            r#"{"event":"step_update","step_update":{"state":"ACTIVE","text_delta":"OK"}}"#,
        ] {
            assert!(deltas(CliKind::Agy, ignored).unwrap().is_empty());
        }
    }

    #[test]
    fn agy_non_success_status_is_a_driver_error() {
        for line in [
            r#"{"status":"ERROR","response":"","error":"quota"}"#,
            r#"{"event":"result","result":{"status":"ERROR","response":"","error":"quota"}}"#,
        ] {
            let err = deltas(CliKind::Agy, line).unwrap_err();
            assert!(
                matches!(&err, DriverError::Failed(m) if m.contains("quota")),
                "{err}"
            );
        }
        assert!(matches!(
            deltas(CliKind::Agy, r#"{"status":"CANCELLED","response":""}"#),
            Err(DriverError::Failed(m)) if m.contains("CANCELLED")
        ));
    }

    #[tokio::test]
    async fn empty_turn_is_retried_once_then_reported() {
        let counter = tempfile::NamedTempFile::new().unwrap();
        let script = format!(
            "cat >/dev/null; echo x >> {}; echo '{{\"type\":\"result\"}}'",
            counter.path().display()
        );
        let (_d, e) = mock(CliKind::Claude, &script);
        let (tx, _rx) = mpsc::channel(4);
        let err = e
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap_err();
        // Claude printed a result with no stop reason and no text: #416, retried once then reported.
        assert!(matches!(err, DriverError::NoStopReason), "{err}");
        assert_eq!(
            std::fs::read_to_string(counter.path())
                .unwrap()
                .lines()
                .count(),
            2
        );

        // First call empty, second answers: the retry rescues the turn.
        let flag = tempfile::tempdir().unwrap().keep().join("seen");
        let script = format!(
            "cat >/dev/null; if [ -e {f} ]; then echo '{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"back\"}}]}}}}'; else touch {f}; fi",
            f = flag.display()
        );
        let (_d, e) = mock(CliKind::Claude, &script);
        let (tx, _rx) = mpsc::channel(4);
        let resp = e
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(resp.text, "back");
    }

    #[tokio::test]
    async fn raw_turns_are_dumped_redacted_and_private_only_when_enabled() {
        let line = r#"{"type":"assistant","message":{"content":[{"type":"text","text":"token=sk-abcdefghijklmnop done"}]}}"#;
        let (_d, mut e) = mock(
            CliKind::Claude,
            &format!("cat >/dev/null; cat <<'EOF'\n{line}\nEOF"),
        );
        e.role = "teacher".into();
        let run = |e: SubprocessEngine| async move {
            let (tx, _rx) = mpsc::channel(4);
            e.stream_turn(TurnRequest::default(), tx, CancellationToken::new())
                .await
                .unwrap();
        };
        run(e.clone()).await; // raw_dir None: nothing written anywhere
        let raw = tempfile::tempdir().unwrap();
        e.raw_dir = Some(raw.path().join("logs/raw"));
        run(e.clone()).await;
        run(e).await;
        let mut files: Vec<_> = std::fs::read_dir(raw.path().join("logs/raw"))
            .unwrap()
            .map(|f| f.unwrap().path())
            .collect();
        files.sort();
        assert_eq!(files.len(), 2);
        let name = files[0].file_name().unwrap().to_str().unwrap().to_owned();
        assert!(
            name.ends_with("-teacher-1.jsonl") || name.contains("-teacher-"),
            "{name}"
        );
        assert!(name.ends_with(".jsonl"));
        for f in &files {
            let mode = std::fs::metadata(f).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode, 0o600);
            let body = std::fs::read_to_string(f).unwrap();
            assert!(
                body.contains("done") && !body.contains("sk-abcdefghijklmnop"),
                "{body}"
            );
        }
    }

    #[test]
    fn unclosed_tool_call_from_real_codex_text_is_parsed() {
        let text = include_str!("fixtures/codex_plan_init_unclosed.txt")
            .trim()
            .to_owned();
        let d = split_calls(text);
        assert_eq!(d.len(), 1, "{d:?}");
        let StreamDelta::ToolCall(c) = &d[0] else {
            panic!("{d:?}")
        };
        assert_eq!(c.name, "plan_init");
        assert!(c.input["mermaid"]
            .as_str()
            .unwrap()
            .contains("cargo --> basics"));
    }

    #[test]
    fn tool_call_tags_tolerate_case_whitespace_and_missing_close() {
        let call = |d: &StreamDelta| match d {
            StreamDelta::ToolCall(c) => Some(c.name.clone()),
            _ => None,
        };
        for text in [
            "<TOOL_CALL>{\"name\":\"q\",\"input\":{}}</Tool_Call>  \n",
            "<tool_call>\n{\"name\":\"q\",\"input\":{}}\n\n</tool_call >",
            "<tool_call>{\"name\":\"q\",\"input\":{\"s\":\"}\"}}  \n",
        ] {
            let d = split_calls(text.into());
            assert_eq!(
                d.iter().filter_map(call).collect::<Vec<_>>(),
                ["q"],
                "{text:?}"
            );
        }
        // text before an unclosed call and a second call after one are both kept
        let d = split_calls("hi <tool_call>{\"name\":\"a\"}<tool_call>{\"name\":\"b\"}".into());
        assert_eq!(d.len(), 3, "{d:?}");
        assert_eq!(d[0], StreamDelta::Text("hi ".into()));
    }

    #[test]
    fn unclosed_tool_calls_stay_text_but_closed_unrecoverable_ones_are_dropped() {
        // Unclosed: may still be streaming, so it stays text.
        for text in [r#"<tool_call>{"name":"q","input":{"a":1}"#, "<tool_call>"] {
            assert_eq!(
                split_calls(text.into()),
                vec![StreamDelta::Text(text.into())],
                "{text}"
            );
        }
        // Closed but not recoverable (not json / no name): never shown to the learner.
        for text in [
            r#"<tool_call>not json</tool_call>"#,
            r#"<tool_call>{"input":{}}</tool_call>"#,
        ] {
            assert_eq!(split_calls(text.into()), vec![], "{text}");
        }
    }

    /// The mock records its pid then blocks; the child must be dead after the error.
    async fn hanging(
        timeout: Option<Duration>,
        cancel: CancellationToken,
    ) -> (DriverError, String) {
        let pidfile = tempfile::NamedTempFile::new().unwrap();
        let (_d, mut e) = mock(
            CliKind::Claude,
            &format!(
                "echo $$ > {}; echo '{{\"type\":\"system\"}}'; exec sleep 30",
                pidfile.path().display()
            ),
        );
        if let Some(t) = timeout {
            e.timeout = t;
        }
        let (tx, _rx) = mpsc::channel(4);
        let err = e
            .stream_turn(TurnRequest::default(), tx, cancel)
            .await
            .unwrap_err();
        (
            err,
            std::fs::read_to_string(pidfile.path())
                .unwrap()
                .trim()
                .to_string(),
        )
    }

    #[tokio::test]
    async fn timeout_kills_and_reaps_child() {
        let (err, pid) = hanging(Some(Duration::from_millis(300)), CancellationToken::new()).await;
        assert!(matches!(err, DriverError::Timeout(_)));
        assert!(!pid_alive(&pid), "child {pid} survived timeout");
    }

    #[tokio::test]
    async fn cancel_kills_and_reaps_child() {
        let cancel = CancellationToken::new();
        let c = cancel.clone();
        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            c.cancel();
        });
        let (err, pid) = hanging(None, cancel).await;
        assert!(matches!(err, DriverError::Cancelled));
        assert!(!pid_alive(&pid), "child {pid} survived cancel");
    }

    #[tokio::test]
    async fn text_protocol_tool_call_is_yielded_for_every_cli() {
        const CALL: &str = r#"ok <tool_call>{\"name\":\"quiz\",\"input\":{\"q\":1}}</tool_call>"#;
        let lines = [
            (
                CliKind::Claude,
                format!(
                    r#"{{"type":"assistant","message":{{"content":[{{"type":"text","text":"{CALL}"}}]}}}}"#
                ),
            ),
            (
                CliKind::Codex,
                format!(
                    r#"{{"type":"item.completed","item":{{"type":"agent_message","text":"{CALL}"}}}}"#
                ),
            ),
            (CliKind::Agy, format!(r#"{{"result":"{CALL}"}}"#)),
        ];
        for (kind, line) in lines {
            let (_d, e) = mock(kind, &format!("cat >/dev/null; cat <<'EOF'\n{line}\nEOF"));
            let (tx, _rx) = mpsc::channel(16);
            let resp = e
                .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
                .await
                .unwrap();
            assert_eq!(resp.text, "ok ", "{kind:?}");
            assert_eq!(resp.tool_calls.len(), 1, "{kind:?}");
            assert_eq!(resp.tool_calls[0].name, "quiz");
            assert_eq!(resp.tool_calls[0].input["q"], 1);
        }
    }

    #[tokio::test]
    async fn tools_are_rendered_into_the_prompt() {
        let (_d, e) = mock(
            CliKind::Claude,
            r#"p=$(cat)
case "$p" in *'<tool_call>'*quiz*) echo '{"type":"assistant","message":{"content":[{"type":"text","text":"seen"}]}}';; esac"#,
        );
        let (tx, _rx) = mpsc::channel(4);
        let req = TurnRequest {
            prompt: "p".into(),
            tools: vec![super::super::ToolDecl {
                name: "quiz".into(),
                usage: "ask".into(),
            }],
            ..Default::default()
        };
        let resp = e
            .stream_turn(req, tx, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(resp.text, "seen");
    }

    #[tokio::test]
    async fn lockdown_flags_and_env_allowlist() {
        let (_d, e) = mock(
            CliKind::Claude,
            r#"cat >/dev/null; echo "{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"$* [${HTTPS_PROXY:-none}] [${CLAUDE_CONFIG_DIR:-unset}]\"}]}}""#,
        );
        std::env::set_var("HTTPS_PROXY", "http://proxy:1");
        let mut e = e;
        e.config_dir = Some(Path::new(&std::env::var("HOME").unwrap()).join(".claude"));
        let (tx, _rx) = mpsc::channel(4);
        let resp = e
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap();
        assert!(resp.text.contains("--tools"), "{}", resp.text);
        assert!(resp.text.contains("[http://proxy:1]"), "{}", resp.text);
        assert!(
            resp.text.contains("[unset]"),
            "default dir must not be exported: {}",
            resp.text
        );

        let (_d, mut x) = mock(
            CliKind::Codex,
            r#"cat >/dev/null; echo "{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"$*\"}}""#,
        );
        x.timeout = Duration::from_secs(10);
        let (tx, _rx) = mpsc::channel(4);
        let resp = x
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap();
        assert!(resp.text.contains("-s read-only"), "{}", resp.text);
    }

    #[test]
    fn effort_maps_to_each_clis_flag() {
        let argv = |kind, model: Option<&str>, effort: Option<&str>| {
            let e = SubprocessEngine::new(kind).unwrap();
            let req = TurnRequest {
                model: model.map(Into::into),
                effort: effort.map(Into::into),
                ..TurnRequest::default()
            };
            let (a, _) = e.turn_args(&req);
            a.join(" ")
        };
        let a = argv(CliKind::Codex, Some("gpt-6-sol"), Some("high"));
        assert!(
            a.ends_with(r#"-m gpt-6-sol -c model_reasoning_effort="high" -"#),
            "{a}"
        );
        let a = argv(CliKind::Claude, Some("m"), Some("max"));
        assert!(a.ends_with("--model m --effort max"), "{a}");
        // agy: a slug that already ends in an effort gets no flag; a bare slug does.
        let a = argv(CliKind::Agy, Some("gemini-4-argon-high"), Some("low"));
        assert!(a.ends_with("--model gemini-4-argon-high"), "{a}");
        let a = argv(CliKind::Agy, Some("gemini-4-argon"), Some("low"));
        assert!(a.ends_with("--model gemini-4-argon --effort low"), "{a}");
        assert!(!argv(CliKind::Codex, Some("m"), None).contains("effort"));
    }

    #[tokio::test]
    async fn mock_codex_receives_model_and_effort_argv() {
        let (_d, mut x) = mock(
            CliKind::Codex,
            r#"cat >/dev/null; echo "{\"type\":\"item.completed\",\"item\":{\"type\":\"agent_message\",\"text\":\"$(echo "$*" | tr -d '"')\"}}""#,
        );
        x.timeout = Duration::from_secs(10);
        let req = TurnRequest {
            model: Some("gpt-6-sol".into()),
            effort: Some("xhigh".into()),
            ..TurnRequest::default()
        };
        let (tx, _rx) = mpsc::channel(4);
        let resp = x
            .stream_turn(req, tx, CancellationToken::new())
            .await
            .unwrap();
        // The mock drops quotes so the argv fits in its JSON reply.
        assert!(
            resp.text
                .ends_with("-m gpt-6-sol -c model_reasoning_effort=xhigh -"),
            "{}",
            resp.text
        );
    }

    #[test]
    fn engine_defaults_apply_when_request_names_none() {
        let mut e = SubprocessEngine::new(CliKind::Claude).unwrap();
        e.model = Some("m".into());
        e.effort = Some("low".into());
        let (a, _) = e.turn_args(&TurnRequest::default());
        assert!(a.join(" ").ends_with("--model m --effort low"));
    }

    #[tokio::test]
    async fn verbose_stderr_does_not_block_child() {
        let (_d, e) = mock(
            CliKind::Claude,
            "cat >/dev/null; head -c 300000 /dev/zero | tr '\\0' x >&2; echo '{\"type\":\"assistant\",\"message\":{\"content\":[{\"type\":\"text\",\"text\":\"ok\"}]}}'",
        );
        let (tx, _rx) = mpsc::channel(4);
        let r = tokio::time::timeout(
            Duration::from_secs(10),
            e.stream_turn(TurnRequest::default(), tx, CancellationToken::new()),
        )
        .await;
        assert!(r.expect("child blocked on stderr").is_ok());
    }

    // ---- kiro, opencode, failure taxonomy, isolation ------------------------------------------

    fn fixture_path(name: &str) -> String {
        format!("{}/src/engine/fixtures/{name}", env!("CARGO_MANIFEST_DIR"))
    }

    /// A mock CLI that replays a recorded stdout fixture and exits with `code`.
    fn replay(kind: CliKind, fixture: &str, code: i32) -> (tempfile::TempDir, SubprocessEngine) {
        mock(
            kind,
            &format!(
                "cat >/dev/null; cat '{}'; exit {code}",
                fixture_path(fixture)
            ),
        )
    }

    async fn turn_err(e: &SubprocessEngine) -> DriverError {
        let (tx, _rx) = mpsc::channel(64);
        e.stream_turn(
            TurnRequest {
                prompt: "p".into(),
                ..TurnRequest::default()
            },
            tx,
            CancellationToken::new(),
        )
        .await
        .unwrap_err()
    }

    const NOW: u64 = 1_800_000_000_000;

    #[tokio::test]
    async fn every_cli_failure_maps_to_the_taxonomy_from_real_fixtures() {
        use super::super::FailureKind::{NoModel, NotLoggedIn, Offline, RateLimited};
        let cases = [
            (CliKind::Opencode, "opencode_no_model.jsonl", 1, NoModel),
            (CliKind::Opencode, "opencode_offline.jsonl", 1, Offline),
            (
                CliKind::Opencode,
                "opencode_free_limit.jsonl",
                1,
                RateLimited { resets_at: None },
            ),
            (CliKind::Kiro, "kiro_bad_model.jsonl", 1, NoModel),
            (CliKind::Claude, "claude_bad_model.jsonl", 0, NoModel),
            (
                CliKind::Claude,
                "claude_not_logged_in.jsonl",
                1,
                NotLoggedIn,
            ),
            (CliKind::Codex, "codex_bad_model.jsonl", 1, NoModel),
            (
                CliKind::Agy,
                "agy_rate_limited.jsonl",
                3,
                RateLimited { resets_at: None },
            ),
        ];
        for (kind, fixture, code, want) in cases {
            let (_d, e) = replay(kind, fixture, code);
            let err = turn_err(&e).await;
            let got = err.failure(NOW);
            // Rate limits carry a parsed reset time; compare the class and check the time below.
            assert_eq!(
                std::mem::discriminant(&got),
                std::mem::discriminant(&want),
                "{kind:?} {fixture}: {err} -> {got:?}"
            );
            if let RateLimited { resets_at } = got {
                assert!(resets_at.is_some_and(|r| r > NOW), "{fixture}: {got:?}");
            }
        }
    }

    #[tokio::test]
    async fn claude_error_notice_is_not_shown_as_model_text() {
        let (_d, e) = replay(CliKind::Claude, "claude_bad_model.jsonl", 0);
        let (tx, mut rx) = mpsc::channel(64);
        let err = e
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap_err();
        assert!(matches!(err, DriverError::Cli { .. }), "{err}");
        assert!(
            rx.try_recv().is_err(),
            "the synthetic notice leaked to the learner"
        );
    }

    #[tokio::test]
    async fn missing_binary_is_not_installed_and_hang_is_timed_out() {
        let mut e = SubprocessEngine::new(CliKind::Kiro).unwrap();
        e.program = "/nonexistent/lp-no-such-cli".into();
        assert_eq!(
            turn_err(&e).await.failure(NOW),
            super::super::FailureKind::NotInstalled
        );
        let (_d, mut e) = mock(CliKind::Opencode, "cat >/dev/null; sleep 30");
        e.timeout = Duration::from_millis(300);
        assert_eq!(
            turn_err(&e).await.failure(NOW),
            super::super::FailureKind::TimedOut
        );
    }

    #[tokio::test]
    async fn codex_waiting_for_network_that_times_out_is_offline() {
        let (_d, mut e) = mock(
            CliKind::Codex,
            r#"cat >/dev/null
echo '{"type":"error","message":"Reconnecting... waiting for network (Connection failed: error sending request)"}'
sleep 30"#,
        );
        e.timeout = Duration::from_millis(4000);
        assert_eq!(
            turn_err(&e).await.failure(NOW),
            super::super::FailureKind::Offline
        );
    }

    const OPENCODE_OK: &str = r#"cat >/dev/null
echo '{"type":"step_start","part":{"type":"step-start"}}'
echo '{"type":"reasoning","part":{"text":"hm"}}'
echo '{"type":"text","part":{"text":"Hi there"}}'
echo '{"type":"tool_use","part":{"tool":"read","state":{"status":"completed"}}}'
echo '{"type":"step_finish","part":{"reason":"stop","cost":0,"tokens":{"input":17,"output":6,"reasoning":2,"cache":{"read":100,"write":3}}}}'"#;

    #[tokio::test]
    async fn opencode_events_become_deltas_and_usage() {
        let (_d, e) = mock(CliKind::Opencode, OPENCODE_OK);
        let (tx, mut rx) = mpsc::channel(16);
        let resp = e
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(resp.text, "Hi there");
        assert_eq!(rx.recv().await, Some(StreamDelta::Thinking("hm".into())));
        // input = input + cache read + cache write; output = output + reasoning
        assert_eq!(e.usage[0].load(Ordering::Relaxed), 120);
        assert_eq!(e.usage[1].load(Ordering::Relaxed), 8);
    }

    #[tokio::test]
    async fn opencode_tool_call_protocol_and_structured_reply() {
        let (_d, e) = mock(
            CliKind::Opencode,
            r#"cat >/dev/null
echo '{"type":"text","part":{"text":"ok <tool_call>{\"name\":\"probe\",\"input\":{\"q\":1}}</tool_call>"}}'"#,
        );
        let (tx, _rx) = mpsc::channel(16);
        let resp = e
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(resp.tool_calls[0].name, "probe");
        assert_eq!(resp.text, "ok ");

        // Structured: no native schema flag; the schema rides in the prompt, fenced JSON is accepted.
        let (_d, e) = mock(
            CliKind::Opencode,
            r#"in=$(cat)
case "$in" in *'"properties"'*) ;; *) exit 9;; esac
printf '%s\n' '{"type":"text","part":{"text":"Sure:\n```json\n{\"n\": 5}\n```"}}'"#,
        );
        let out: StructuredOutput<Out> = e
            .complete_structured(StructuredRequest::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(out.value, Out { n: 5 });
    }

    const KIRO_OK: &str = r#"cat >/dev/null
echo '{"type":"runStarted","data":{}}'
echo '{"type":"sessionUpdate","data":{"update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"Hi <tool_"}}}}'
echo '{"type":"sessionUpdate","data":{"update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"call>{\"name\":\"probe\",\"input\":{}}</tool_call>"}}}}'
echo '{"type":"sessionUpdate","data":{"update":{"sessionUpdate":"session_info_update","_meta":{"kiro":{"promptTurnSummaries":[{"unit":"credit","usage":0.0125}]}}}}}'
echo '{"type":"runFinished","data":{"status":"success","finalText":"ignored"}}'"#;

    #[tokio::test]
    async fn kiro_chunks_are_joined_for_tool_calls_and_credits_reported() {
        let (_d, e) = mock(CliKind::Kiro, KIRO_OK);
        let (tx, _rx) = mpsc::channel(16);
        let resp = e
            .stream_turn(
                TurnRequest {
                    tools: vec![super::super::ToolDecl {
                        name: "probe".into(),
                        usage: "x".into(),
                    }],
                    ..TurnRequest::default()
                },
                tx,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        // The call was split across two chunks and still parsed; finalText is only a fallback.
        assert_eq!(resp.tool_calls[0].name, "probe");
        assert_eq!(resp.text, "Hi ");
        assert_eq!(e.usage[2].load(Ordering::Relaxed), 12_500);
        assert_eq!(e.usage[0].load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn kiro_plain_turn_streams_chunks_and_falls_back_to_final_text() {
        let (_d, e) = mock(CliKind::Kiro, KIRO_OK);
        let (tx, mut rx) = mpsc::channel(16);
        // Without tools each chunk streams as it arrives.
        e.stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(rx.recv().await, Some(StreamDelta::Text("Hi <tool_".into())));
        let (_d, e) = mock(
            CliKind::Kiro,
            r#"cat >/dev/null; echo '{"type":"runFinished","data":{"status":"success","finalText":"only final"}}'"#,
        );
        let (tx, _rx) = mpsc::channel(4);
        let resp = e
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(resp.text, "only final");
    }

    #[tokio::test]
    async fn kiro_structured_parses_json_from_reply() {
        let (_d, e) = mock(
            CliKind::Kiro,
            r#"cat >/dev/null
echo '{"type":"sessionUpdate","data":{"update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":"{\"n\":"}}}}'
echo '{"type":"sessionUpdate","data":{"update":{"sessionUpdate":"agent_message_chunk","content":{"type":"text","text":" 8}"}}}}'"#,
        );
        let out: StructuredOutput<Out> = e
            .complete_structured(StructuredRequest::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(out.value, Out { n: 8 });
    }

    #[test]
    fn real_kiro_and_opencode_fixtures_parse() {
        let ok = include_str!("fixtures/kiro_ok.jsonl");
        let text: String = ok
            .lines()
            .flat_map(|l| raw_deltas(CliKind::Kiro, l).unwrap())
            .filter_map(|d| match d {
                StreamDelta::Text(t) => Some(t),
                _ => None,
            })
            .collect();
        assert_eq!(text, "Hi! How can I help you today?");
        let credits: u64 = ok
            .lines()
            .filter_map(|l| credits_of(CliKind::Kiro, l))
            .sum();
        assert_eq!(credits, 28_862);
        let meter =
            r#"{"type":"metadata","data":{"meteringUsage":[{"value":0.016117,"unit":"credit"}]}}"#;
        assert_eq!(credits_of(CliKind::Kiro, meter), Some(16_117));
        assert_eq!(credits_of(CliKind::Claude, meter), None);
        assert_eq!(
            ok.lines()
                .filter_map(|l| error_of(CliKind::Kiro, l))
                .count(),
            0
        );
        let bad = include_str!("fixtures/kiro_bad_model.jsonl");
        assert!(bad
            .lines()
            .filter_map(|l| error_of(CliKind::Kiro, l))
            .any(|m| m.contains("not available")));
    }

    #[test]
    fn isolation_args_for_every_cli() {
        let args = |k| {
            SubprocessEngine::new(k)
                .unwrap()
                .turn_args(&TurnRequest::default())
                .0
                .join(" ")
        };
        // #413: the user's ~/.claude CLAUDE.md, output style and settings must not reach the tutor.
        assert!(args(CliKind::Claude).contains("--setting-sources project,local"));
        assert!(args(CliKind::Claude).contains("--tools "));
        // #414: codex ignores user config (MCP servers, hooks) and keeps no session files.
        let codex = args(CliKind::Codex);
        assert!(codex.contains("--ignore-user-config") && codex.contains("--ephemeral"));
        assert!(codex.contains("-s read-only"));
        // opencode: private server (the shared background service ignores our config), JSON events.
        assert!(args(CliKind::Opencode)
            .starts_with("run --standalone --format json -m opencode/big-pickle"));
        let kiro = args(CliKind::Kiro);
        assert!(
            kiro.contains("--agent lp-tutor") && kiro.contains("--trust-tools= "),
            "{kiro}"
        );
        assert!(kiro.contains("--output-format stream-json"));
        // never auto-approve anything
        for k in [
            CliKind::Opencode,
            CliKind::Kiro,
            CliKind::Codex,
            CliKind::Claude,
        ] {
            let a = args(k);
            assert!(
                !a.contains("--auto") && !a.contains("trust-all") && !a.contains("-a "),
                "{a}"
            );
        }
    }

    #[test]
    fn claude_structured_args_carry_the_isolation_flags_too() {
        let (_d, e) = mock(
            CliKind::Claude,
            r#"cat >/dev/null
case "$*" in *"--setting-sources project,local"*) echo '{"type":"result","structured_output":{"n":1}}';; *) exit 9;; esac"#,
        );
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let out: StructuredOutput<Out> = rt
            .block_on(e.complete_structured(StructuredRequest::default(), CancellationToken::new()))
            .unwrap();
        assert_eq!(out.value, Out { n: 1 });
    }

    #[tokio::test]
    async fn opencode_runs_in_a_private_locked_down_home() {
        let (_d, e) = mock(
            CliKind::Opencode,
            r#"cat >/dev/null
case "$OPENCODE_CONFIG_CONTENT" in *'"*":"deny"'*) lock=yes;; *) lock=no;; esac
printf '{"type":"text","part":{"text":"home=%s cfg=%s data=%s cwd=%s dis=%s lock=%s cc=%s"}}\n' "$HOME" "$XDG_CONFIG_HOME" "$XDG_DATA_HOME" "$(pwd -P)" "$OPENCODE_DISABLE_CLAUDE_CODE" "$lock" ${CLAUDECODE:-unset}"#,
        );
        std::env::set_var("CLAUDECODE", "1");
        let (tx, _rx) = mpsc::channel(4);
        let r = e
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap()
            .text;
        let real = std::env::var("HOME").unwrap();
        assert!(!r.contains(&format!("home={real} ")), "{r}");
        assert!(r.contains("home=") && r.contains("lp-home-"), "{r}");
        assert!(r.contains("/config ") && r.contains("/data "), "{r}");
        assert!(
            r.contains("lp-sandbox-") && r.contains("dis=1") && r.contains("cc=unset"),
            "{r}"
        );
        assert!(r.contains("lock=yes"), "{r}");
    }

    #[test]
    fn kiro_home_has_the_tool_less_agent_and_only_links_its_own_store() {
        let real = tempfile::tempdir().unwrap();
        for d in [
            "Library/Application Support/kiro-cli",
            ".local/bin",
            ".kiro/skills/secret",
            ".ssh",
        ] {
            std::fs::create_dir_all(real.path().join(d)).unwrap();
        }
        let dest = tempfile::tempdir().unwrap();
        prepare_kiro_home(real.path(), dest.path()).unwrap();
        let agent: Value = serde_json::from_slice(
            &std::fs::read(dest.path().join(".kiro/agents/lp-tutor.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(agent["tools"], serde_json::json!([]));
        assert_eq!(agent["mcpServers"], serde_json::json!({}));
        assert_eq!(agent["includeMcpJson"], false);
        let linked = |rel: &str| std::fs::symlink_metadata(dest.path().join(rel)).is_ok();
        assert!(linked("Library/Application Support/kiro-cli") && linked(".local/bin"));
        assert!(!linked(".kiro/skills") && !linked(".ssh"));
    }

    #[tokio::test]
    async fn empty_output_exit_zero_is_retried_once_for_every_cli_and_structured() {
        for kind in [CliKind::Opencode, CliKind::Kiro, CliKind::Codex] {
            let counter = tempfile::NamedTempFile::new().unwrap();
            let (_d, e) = mock(
                kind,
                &format!("cat >/dev/null; echo x >> {}", counter.path().display()),
            );
            let err = turn_err(&e).await;
            assert!(matches!(err, DriverError::EmptyResponse), "{kind:?}: {err}");
            let n = std::fs::read_to_string(counter.path())
                .unwrap()
                .lines()
                .count();
            assert_eq!(n, 2, "{kind:?}");
        }
        let counter = tempfile::NamedTempFile::new().unwrap();
        let (_d, e) = mock(
            CliKind::Kiro,
            &format!("cat >/dev/null; echo x >> {}", counter.path().display()),
        );
        let r: Result<StructuredOutput<Out>, _> = e
            .complete_structured(StructuredRequest::default(), CancellationToken::new())
            .await;
        assert!(matches!(r, Err(DriverError::EmptyResponse)));
        assert_eq!(
            std::fs::read_to_string(counter.path())
                .unwrap()
                .lines()
                .count(),
            2
        );
    }

    #[tokio::test]
    async fn claude_cut_off_without_result_is_retried_but_partial_text_is_kept() {
        let flag = tempfile::tempdir().unwrap().keep().join("seen");
        let (_d, e) = mock(
            CliKind::Claude,
            &format!(
                "cat >/dev/null; if [ -e {f} ]; then echo '{{\"type\":\"assistant\",\"message\":{{\"content\":[{{\"type\":\"text\",\"text\":\"whole\"}}]}}}}'; echo '{{\"type\":\"result\",\"stop_reason\":\"end_turn\"}}'; else touch {f}; fi",
                f = flag.display()
            ),
        );
        let (tx, _rx) = mpsc::channel(4);
        let resp = e
            .stream_turn(TurnRequest::default(), tx, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(resp.text, "whole");
        // Text without a stop reason is returned as is: retrying would repeat what the learner saw.
        let (_d, e) = mock(
            CliKind::Claude,
            r#"cat >/dev/null; echo '{"type":"assistant","message":{"content":[{"type":"text","text":"partial"}]}}'"#,
        );
        let (tx, _rx) = mpsc::channel(4);
        assert_eq!(
            e.stream_turn(TurnRequest::default(), tx, CancellationToken::new())
                .await
                .unwrap()
                .text,
            "partial"
        );
    }

    /// Real CLIs, tiny spend: `cargo test -p lp-cli-driver real_smoke -- --ignored --nocapture`.
    async fn smoke(kind: CliKind, model: &str) {
        let mut e = SubprocessEngine::new(kind).unwrap();
        e.model = Some(model.into());
        e.timeout = Duration::from_secs(150);
        let (tx, _rx) = mpsc::channel(256);
        let plain = e
            .stream_turn(
                TurnRequest {
                    prompt: "Reply with the single word: pong".into(),
                    ..TurnRequest::default()
                },
                tx,
                CancellationToken::new(),
            )
            .await;
        eprintln!("[{kind:?}] text turn: {plain:?}");
        let (tx, _rx) = mpsc::channel(256);
        let tool = e
            .stream_turn(
                TurnRequest {
                    prompt:
                        "Call the tool probe with input {\"q\":1}. Reply with only the tool call."
                            .into(),
                    tools: vec![super::super::ToolDecl {
                        name: "probe".into(),
                        usage: "probe(q: number)".into(),
                    }],
                    ..TurnRequest::default()
                },
                tx,
                CancellationToken::new(),
            )
            .await;
        eprintln!("[{kind:?}] tool turn: {tool:?}");
        eprintln!(
            "[{kind:?}] usage in/out/microcredits: {:?}",
            e.usage
                .iter()
                .map(|u| u.load(Ordering::Relaxed))
                .collect::<Vec<_>>()
        );
        for r in [plain.map(|_| ()), tool.map(|_| ())] {
            if let Err(err) = r {
                eprintln!("[{kind:?}] failure class: {:?}", err.failure(0));
            }
        }
    }

    #[tokio::test]
    #[ignore = "spends real (free) model calls"]
    async fn real_smoke_opencode() {
        smoke(CliKind::Opencode, "opencode/big-pickle").await;
    }

    #[tokio::test]
    #[ignore = "spends real (tiny) kiro credits"]
    async fn real_smoke_kiro() {
        smoke(CliKind::Kiro, "qwen3-coder-next").await;
    }
}
