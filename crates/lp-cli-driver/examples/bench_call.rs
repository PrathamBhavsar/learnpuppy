//! One real CLI call through the production driver, for `bench/cheap_tutor`.
//! stdin: `{"provider","model","effort","prompt"}`; stdout: one JSON line with text, usage and time.
use lp_cli_driver::engine::{CliKind, LlmEngine, SubprocessEngine, TurnRequest};
use serde_json::{json, Value};
use std::io::Read;
use tokio_util::sync::CancellationToken;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut raw = String::new();
    std::io::stdin().read_to_string(&mut raw)?;
    let v: Value = serde_json::from_str(&raw)?;
    let s = |k: &str| v.get(k).and_then(Value::as_str).map(str::to_owned);
    let kind = match s("provider").as_deref() {
        Some("claude") => CliKind::Claude,
        Some("codex") => CliKind::Codex,
        Some("agy") => CliKind::Agy,
        Some("opencode") => CliKind::Opencode,
        Some("kiro") => CliKind::Kiro,
        other => anyhow::bail!("unknown provider {other:?}"),
    };
    let mut e = SubprocessEngine::new(kind)?;
    e.timeout = std::time::Duration::from_secs(300);
    let req = TurnRequest {
        prompt: s("prompt").unwrap_or_default(),
        model: s("model"),
        effort: s("effort"),
        tools: vec![],
    };
    let (tx, mut rx) = tokio::sync::mpsc::channel(1024);
    tokio::spawn(async move { while rx.recv().await.is_some() {} });
    let t = std::time::Instant::now();
    let r = e.stream_turn(req, tx, CancellationToken::new()).await;
    let (text, error) = match r {
        Ok(r) => (r.text, None),
        Err(err) => (String::new(), Some(err.to_string())),
    };
    let n = |i: usize| e.usage[i].load(std::sync::atomic::Ordering::Relaxed);
    println!(
        "{}",
        json!({"text": text, "error": error, "in": n(0), "out": n(1),
               "credits": n(2) as f64 / 1e6, "ms": t.elapsed().as_millis() as u64})
    );
    Ok(())
}
