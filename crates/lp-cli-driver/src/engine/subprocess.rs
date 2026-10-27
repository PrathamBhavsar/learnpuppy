use std::path::{Path, PathBuf};
use std::process::Stdio;
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
const ENV_ALLOW: [&str; 9] = [
    "HOME",
    "PATH",
    "HTTPS_PROXY",
    "HTTP_PROXY",
    "NO_PROXY",
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
}

/// Drives one agent CLI (`claude -p`, `codex exec`, `agy -p`) as a child process.
#[derive(Debug, Clone)]
pub struct SubprocessEngine {
    pub kind: CliKind,
    pub program: PathBuf,
    pub timeout: Duration,
    /// Claude only: `CLAUDE_CONFIG_DIR` selecting which account to use.
    pub config_dir: Option<PathBuf>,
    /// Private (0700, random name) working directory, removed when the last clone drops.
    sandbox: Arc<tempfile::TempDir>,
}

impl SubprocessEngine {
    pub fn new(kind: CliKind) -> Result<Self, DriverError> {
        let program = match kind {
            CliKind::Claude => "claude",
            CliKind::Codex => "codex",
            CliKind::Agy => "agy",
        };
        Ok(Self {
            kind,
            program: program.into(),
            timeout: Duration::from_secs(600),
            config_dir: None,
            sandbox: Arc::new(tempfile::Builder::new().prefix("lp-sandbox-").tempdir()?),
        })
    }

    fn sandbox(&self) -> &Path {
        self.sandbox.path()
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
        for key in ENV_ALLOW {
            if let Some(v) = std::env::var_os(key) {
                cmd.env(key, v);
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
            }
            CliKind::Codex => {
                a.extend(
                    [
                        "exec",
                        "--json",
                        "--skip-git-repo-check",
                        "-s",
                        "read-only",
                        "-C",
                    ]
                    .map(String::from),
                );
                a.push(self.sandbox().display().to_string());
            }
            CliKind::Agy => a.extend(["--output-format", "json", "--sandbox"].map(String::from)),
        }
        self.finish_args(
            a,
            &with_tools(&req.prompt, &req.tools),
            req.model.as_deref(),
        )
    }

    /// Adds model and prompt delivery: stdin for every CLI (agy: `-p -`), so large prompts never hit argv limits or `ps`.
    fn finish_args(
        &self,
        mut a: Vec<String>,
        prompt: &str,
        model: Option<&str>,
    ) -> (Vec<String>, Option<String>) {
        if let Some(m) = model {
            a.push(
                if self.kind == CliKind::Codex {
                    "-m"
                } else {
                    "--model"
                }
                .into(),
            );
            a.push(m.into());
        }
        match self.kind {
            CliKind::Codex => {
                a.push("-".into());
                (a, Some(prompt.into()))
            }
            CliKind::Agy => {
                a.extend(["-p".into(), "-".into()]);
                (a, Some(prompt.into()))
            }
            CliKind::Claude => (a, Some(prompt.into())),
        }
    }
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

/// Splits `<tool_call>{json}</tool_call>` blocks out of model text; malformed blocks stay as text.
pub(super) fn split_calls(text: String) -> Vec<StreamDelta> {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let (mut out, mut rest) = (Vec::new(), text.as_str());
    while let Some(start) = rest.find(CALL_OPEN) {
        let body = &rest[start + CALL_OPEN.len()..];
        let Some(end) = body.find(CALL_CLOSE) else {
            break;
        };
        let Some(v) = serde_json::from_str::<Value>(body[..end].trim())
            .ok()
            .filter(|v| v.get("name").and_then(Value::as_str).is_some())
        else {
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
        rest = &body[end + CALL_CLOSE.len()..];
    }
    if !rest.is_empty() {
        out.push(StreamDelta::Text(rest.into()));
    }
    out
}

/// Stream deltas carried by one stdout line; text is scanned for protocol tool calls.
fn deltas(kind: CliKind, line: &str) -> Vec<StreamDelta> {
    raw_deltas(kind, line)
        .into_iter()
        .flat_map(|d| match d {
            StreamDelta::Text(t) => split_calls(t),
            d => vec![d],
        })
        .collect()
}

fn raw_deltas(kind: CliKind, line: &str) -> Vec<StreamDelta> {
    let Ok(v) = serde_json::from_str::<Value>(line) else {
        return match kind {
            CliKind::Agy if !line.is_empty() => vec![StreamDelta::Text(line.into())],
            _ => vec![],
        };
    };
    let str_of = |v: &Value, k: &str| v.get(k).and_then(Value::as_str).map(String::from);
    match (kind, v.get("type").and_then(Value::as_str)) {
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
        (CliKind::Agy, _) => str_of(&v, "result")
            .map(StreamDelta::Text)
            .into_iter()
            .collect(),
        _ => vec![],
    }
}

/// The structured JSON payload out of a finished run's stdout.
fn structured_json(kind: CliKind, stdout: &str) -> Result<Value, DriverError> {
    let bad = |e: serde_json::Error| DriverError::Parse(e.to_string());
    match kind {
        CliKind::Codex => {
            let last = stdout
                .lines()
                .flat_map(|l| deltas(kind, l))
                .filter_map(|d| match d {
                    StreamDelta::Text(t) => Some(t),
                    _ => None,
                })
                .next_back()
                .ok_or_else(|| DriverError::Parse("no agent message".into()))?;
            serde_json::from_str(&last).map_err(bad)
        }
        CliKind::Claude | CliKind::Agy => {
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

#[async_trait]
impl LlmEngine for SubprocessEngine {
    async fn stream_turn(
        &self,
        req: TurnRequest,
        event_tx: mpsc::Sender<StreamDelta>,
        cancel: CancellationToken,
    ) -> Result<TurnResponse, DriverError> {
        let (args, stdin) = self.turn_args(&req);
        let mut proc = self.spawn(args, stdin, cancel)?;
        let mut resp = TurnResponse::default();
        while let Some(line) = proc.next().await? {
            for d in deltas(self.kind, &line) {
                match &d {
                    StreamDelta::Text(t) => resp.text.push_str(t),
                    StreamDelta::ToolCall(c) => resp.tool_calls.push(c.clone()),
                    StreamDelta::Thinking(_) => {}
                }
                event_tx.send(d).await.map_err(|_| DriverError::Cancelled)?;
            }
        }
        Ok(resp)
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
            }
            CliKind::Codex => {
                let mut f = tempfile::Builder::new()
                    .suffix(".json")
                    .tempfile_in(self.sandbox())?;
                std::io::Write::write_all(&mut f, schema.as_bytes())?;
                a.extend(
                    [
                        "exec",
                        "--json",
                        "--skip-git-repo-check",
                        "-s",
                        "read-only",
                        "-C",
                    ]
                    .map(String::from),
                );
                a.push(self.sandbox().display().to_string());
                a.extend(["--output-schema".into(), f.path().display().to_string()]);
                schema_file = Some(f);
            }
            CliKind::Agy => {
                a.extend(
                    ["--output-format", "json", "--sandbox", "--json-schema"].map(String::from),
                );
                a.push(schema);
            }
        }
        let (args, stdin) = self.finish_args(a, &req.prompt, req.model.as_deref());
        let mut proc = self.spawn(args, stdin, cancel)?;
        let mut stdout = String::new();
        while let Some(line) = proc.next().await? {
            stdout.push_str(&line);
            stdout.push('\n');
        }
        drop(schema_file);
        let raw = structured_json(self.kind, &stdout)?;
        let value =
            serde_json::from_value(raw.clone()).map_err(|e| DriverError::Parse(e.to_string()))?;
        Ok(StructuredOutput { value, raw })
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt;

    use super::*;

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

    #[tokio::test]
    async fn claude_stream_json_becomes_deltas() {
        let (_d, e) = mock(CliKind::Claude, CLAUDE_STREAM);
        let (tx, mut rx) = mpsc::channel(16);
        let resp = e
            .stream_turn(
                TurnRequest {
                    prompt: "p".into(),
                    model: None,
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

    #[tokio::test]
    async fn agy_prompt_goes_over_stdin_not_argv() {
        let (_d, e) = mock(
            CliKind::Agy,
            r#"n=$(wc -c | tr -d ' '); echo "{\"result\":\"stdin=$n argc=$#\"}""#,
        );
        let (tx, _rx) = mpsc::channel(4);
        let resp = e
            .stream_turn(
                TurnRequest {
                    prompt: "x".repeat(1_000_000),
                    ..TurnRequest::default()
                },
                tx,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert!(resp.text.starts_with("stdin=1000000 "), "{}", resp.text);
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

    #[tokio::test]
    async fn verbose_stderr_does_not_block_child() {
        let (_d, e) = mock(
            CliKind::Claude,
            "cat >/dev/null; head -c 300000 /dev/zero | tr '\\0' x >&2; echo '{\"type\":\"result\"}'",
        );
        let (tx, _rx) = mpsc::channel(4);
        let r = tokio::time::timeout(
            Duration::from_secs(10),
            e.stream_turn(TurnRequest::default(), tx, CancellationToken::new()),
        )
        .await;
        assert!(r.expect("child blocked on stderr").is_ok());
    }
}
