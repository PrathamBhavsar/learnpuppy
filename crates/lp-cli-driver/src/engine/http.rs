//! Optional bring-your-own-key engine over the Anthropic Messages API (`~/.learnpuppy/auth.json`).

use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::time::Duration;

use async_trait::async_trait;
use schemars::JsonSchema;
use serde::de::DeserializeOwned;
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use super::subprocess::{split_calls, with_tools};
use super::{
    DriverError, LlmEngine, StreamDelta, StructuredOutput, StructuredRequest, TurnRequest,
    TurnResponse,
};

const BASE_URL: &str = "https://api.anthropic.com";
const DEFAULT_MODEL: &str = "claude-sonnet-5-5";
const TIMEOUT: Duration = Duration::from_secs(120);
const MAX_TOKENS: u32 = 4096;

#[derive(Deserialize)]
struct AuthFile {
    anthropic_api_key: String,
}

pub struct HttpApiEngine {
    client: reqwest::Client,
    key: String,
    base_url: String,
}

// The key must never reach a log or a panic message.
impl std::fmt::Debug for HttpApiEngine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpApiEngine")
            .field("base_url", &self.base_url)
            .finish_non_exhaustive()
    }
}

impl HttpApiEngine {
    pub fn new(key: impl Into<String>) -> Self {
        Self::with_base_url(key, BASE_URL)
    }

    pub fn with_base_url(key: impl Into<String>, base_url: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::builder()
                .timeout(TIMEOUT)
                .build()
                .unwrap_or_default(),
            key: key.into(),
            base_url: base_url.into(),
        }
    }

    /// Reads `{"anthropic_api_key": "..."}`; refuses a file that group or others can read.
    /// `Ok(None)` when the file does not exist (BYOK is optional).
    pub fn from_auth_file(path: &Path) -> Result<Option<Self>, DriverError> {
        let meta = match std::fs::metadata(path) {
            Ok(m) => m,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(e) => return Err(e.into()),
        };
        if meta.permissions().mode() & 0o077 != 0 {
            return Err(DriverError::Io(std::io::Error::new(
                std::io::ErrorKind::PermissionDenied,
                format!(
                    "{} must not be accessible by group or others (chmod 600)",
                    path.display()
                ),
            )));
        }
        let auth: AuthFile = serde_json::from_slice(&std::fs::read(path)?).map_err(|_| {
            DriverError::Parse(format!("{}: expected anthropic_api_key", path.display()))
        })?;
        Ok(Some(Self::new(auth.anthropic_api_key)))
    }

    async fn complete(
        &self,
        prompt: String,
        model: Option<String>,
        cancel: &CancellationToken,
    ) -> Result<String, DriverError> {
        let body = json!({
            "model": model.as_deref().unwrap_or(DEFAULT_MODEL),
            "max_tokens": MAX_TOKENS,
            "messages": [{"role": "user", "content": prompt}],
        });
        let req = self
            .client
            .post(format!("{}/v1/messages", self.base_url))
            .header("x-api-key", &self.key)
            .header("anthropic-version", "2023-06-01")
            .json(&body)
            .send();
        let resp = tokio::select! {
            _ = cancel.cancelled() => return Err(DriverError::Cancelled),
            r = req => r.map_err(|e| DriverError::Parse(e.without_url().to_string()))?,
        };
        let status = resp.status();
        let v: Value = resp
            .json()
            .await
            .map_err(|e| DriverError::Parse(e.without_url().to_string()))?;
        if !status.is_success() {
            return Err(DriverError::Exit {
                code: Some(i32::from(status.as_u16())),
                stderr: v["error"]["message"].as_str().unwrap_or_default().into(),
            });
        }
        Ok(v["content"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|b| b["text"].as_str())
            .collect())
    }
}

#[async_trait]
impl LlmEngine for HttpApiEngine {
    async fn stream_turn(
        &self,
        req: TurnRequest,
        event_tx: mpsc::Sender<StreamDelta>,
        cancel: CancellationToken,
    ) -> Result<TurnResponse, DriverError> {
        let text = self
            .complete(with_tools(&req.prompt, &req.tools), req.model, &cancel)
            .await?;
        let mut resp = TurnResponse::default();
        for d in split_calls(text) {
            match &d {
                StreamDelta::Text(t) => resp.text.push_str(t),
                StreamDelta::ToolCall(c) => resp.tool_calls.push(c.clone()),
                StreamDelta::Thinking(_) => {}
            }
            event_tx.send(d).await.map_err(|_| DriverError::Cancelled)?;
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
        let prompt = format!(
            "{}\n\nReply with only a JSON value matching this JSON schema, no prose:\n{schema}",
            req.prompt
        );
        let text = self.complete(prompt, req.model, &cancel).await?;
        let bad = |e: serde_json::Error| DriverError::Parse(e.to_string());
        let json = match (text.find('{'), text.rfind('}')) {
            (Some(a), Some(b)) if a < b => &text[a..=b],
            _ => text.trim(),
        };
        let raw: Value = serde_json::from_str(json).map_err(bad)?;
        let value = serde_json::from_value(raw.clone()).map_err(bad)?;
        Ok(StructuredOutput { value, raw })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    /// Serves one canned reply and returns the raw request it received.
    async fn mock(reply: &'static str, status: u16) -> (String, tokio::task::JoinHandle<String>) {
        let l = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}", l.local_addr().unwrap());
        let h = tokio::spawn(async move {
            let (mut s, _) = l.accept().await.unwrap();
            let mut buf = vec![0; 16384];
            let n = s.read(&mut buf).await.unwrap();
            let head = format!(
                "HTTP/1.1 {status} X\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{reply}",
                reply.len()
            );
            s.write_all(head.as_bytes()).await.unwrap();
            String::from_utf8_lossy(&buf[..n]).into_owned()
        });
        (url, h)
    }

    #[tokio::test]
    async fn turn_sends_key_and_parses_text_and_tool_calls() {
        let reply = r#"{"content":[{"type":"text","text":"hi <tool_call>{\"name\":\"probe\",\"input\":{\"q\":1}}</tool_call>"}]}"#;
        let (url, req) = mock(reply, 200).await;
        let engine = HttpApiEngine::with_base_url("sk-secret", url);
        let (tx, mut rx) = mpsc::channel(8);
        let r = engine
            .stream_turn(
                TurnRequest {
                    prompt: "hello".into(),
                    ..TurnRequest::default()
                },
                tx,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        assert_eq!(r.text, "hi ");
        assert_eq!(r.tool_calls[0].name, "probe");
        assert_eq!(rx.recv().await, Some(StreamDelta::Text("hi ".into())));
        let sent = req.await.unwrap().to_lowercase();
        assert!(sent.contains("x-api-key: sk-secret") && sent.contains("/v1/messages"));
        assert!(!format!("{engine:?}").contains("sk-secret"));
    }

    #[tokio::test]
    async fn structured_extracts_json_and_api_errors_surface() {
        #[derive(serde::Deserialize, JsonSchema, PartialEq, Debug)]
        struct Out {
            n: u32,
        }
        let reply = r#"{"content":[{"type":"text","text":"```json\n{\"n\": 7}\n```"}]}"#;
        let (url, _) = mock(reply, 200).await;
        let out: StructuredOutput<Out> = HttpApiEngine::with_base_url("k", url)
            .complete_structured(StructuredRequest::default(), CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(out.value, Out { n: 7 });

        let (url, _) = mock(r#"{"error":{"message":"invalid x-api-key"}}"#, 401).await;
        let err = HttpApiEngine::with_base_url("k", url)
            .complete_structured::<Out>(StructuredRequest::default(), CancellationToken::new())
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            DriverError::Exit {
                code: Some(401),
                ..
            }
        ));
    }

    #[test]
    fn auth_file_requires_private_permissions() {
        let d = tempfile::tempdir().unwrap();
        let p = d.path().join("auth.json");
        assert!(HttpApiEngine::from_auth_file(&p).unwrap().is_none());
        std::fs::write(&p, r#"{"anthropic_api_key":"sk-1"}"#).unwrap();
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o644)).unwrap();
        let err = HttpApiEngine::from_auth_file(&p).unwrap_err();
        assert!(
            matches!(err, DriverError::Io(ref e) if e.kind() == std::io::ErrorKind::PermissionDenied)
        );
        assert!(!err.to_string().contains("sk-1"));
        std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o600)).unwrap();
        assert!(HttpApiEngine::from_auth_file(&p).unwrap().is_some());
        std::fs::write(&p, "{}").unwrap();
        assert!(matches!(
            HttpApiEngine::from_auth_file(&p),
            Err(DriverError::Parse(_))
        ));
    }
}
