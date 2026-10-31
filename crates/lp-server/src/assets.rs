//! The SPA: `web/dist` embedded with rust-embed (debug builds read it from disk), served with
//! the CSP from docs/web/01. Vite's hashed `assets/*` are immutable; `index.html` is never
//! cached. Paths without a file extension fall back to `index.html` (client routing). Without
//! a build, every page explains how to make one.

use std::borrow::Cow;
use std::path::{Component, Path, PathBuf};

use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_TYPE, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS};
use axum::http::{HeaderValue, Method, StatusCode};
use axum::response::{IntoResponse, Response};
use rust_embed::RustEmbed;

use crate::app::AppState;
use crate::error::{ApiError, ErrorCode};

pub const CSP: &str = "default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; connect-src 'self'; frame-ancestors 'none'";
const IMMUTABLE: &str = "public, max-age=31536000, immutable";
const NO_CACHE: &str = "no-cache";
const INDEX: &str = "index.html";

#[derive(RustEmbed)]
#[folder = "../../web/dist"]
#[allow_missing = true]
struct Dist;

/// Where the built SPA comes from.
#[derive(Clone, Debug)]
pub enum Assets {
    /// Compiled into the binary (read from `web/dist` on disk in debug builds).
    Embedded,
    /// A directory on disk (tests, custom builds).
    Dir(PathBuf),
}

impl Assets {
    /// File bytes at `rel` (no leading slash). Blocking.
    pub fn get(&self, rel: &str) -> Option<Cow<'static, [u8]>> {
        match self {
            Assets::Embedded => Dist::get(rel).map(|f| f.data),
            Assets::Dir(root) => {
                let rel = Path::new(rel);
                if !rel.components().all(|c| matches!(c, Component::Normal(_))) {
                    return None;
                }
                let path = root.join(rel);
                path.is_file()
                    .then(|| std::fs::read(path).ok())
                    .flatten()
                    .map(Cow::Owned)
            }
        }
    }

    pub fn has_index(&self) -> bool {
        self.get(INDEX).is_some()
    }
}

/// Client-side routes have no extension in their last segment.
fn is_spa_route(rel: &str) -> bool {
    !rel.rsplit('/').next().unwrap_or("").contains('.')
}

fn cache_policy(rel: &str) -> &'static str {
    if rel.starts_with("assets/") {
        IMMUTABLE
    } else {
        NO_CACHE
    }
}

fn respond(status: StatusCode, content_type: &str, cache: &'static str, body: Body) -> Response {
    let mut res = (status, body).into_response();
    let h = res.headers_mut();
    if let Ok(ct) = HeaderValue::from_str(content_type) {
        h.insert(CONTENT_TYPE, ct);
    }
    h.insert(CACHE_CONTROL, HeaderValue::from_static(cache));
    h.insert("content-security-policy", HeaderValue::from_static(CSP));
    h.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
    h.insert(REFERRER_POLICY, HeaderValue::from_static("no-referrer"));
    res
}

const MISSING_PAGE: &str = r#"<!doctype html>
<html lang="en"><head><meta charset="utf-8"><title>LearnPuppy: web UI not built</title>
<style>body{font:16px/1.5 system-ui,sans-serif;max-width:40rem;margin:4rem auto;padding:0 1rem;color:#1f1f24}
code{background:#f1f0f4;padding:.1rem .3rem;border-radius:4px}</style></head>
<body><h1>The LearnPuppy web UI is not built</h1>
<p>The server is running and its API works, but there is no compiled SPA to serve.</p>
<p>Build it, then restart the server:</p>
<p><code>bun install --cwd web &amp;&amp; bun run --cwd web build</code></p>
<p>Or develop against the Vite dev server: <code>bun --cwd web dev</code> and open
<code>http://localhost:5173</code>.</p></body></html>
"#;

/// Router fallback: SPA files and routes; unknown `/api/*` paths get the JSON 404.
pub async fn serve(State(st): State<AppState>, req: Request) -> Response {
    let path = req.uri().path().to_owned();
    if path == "/api" || path.starts_with("/api/") {
        return crate::error::api_not_found(req).await.into_response();
    }
    if !matches!(*req.method(), Method::GET | Method::HEAD) {
        return crate::error::method_not_allowed(req).await.into_response();
    }
    let rel = path.trim_start_matches('/').to_owned();
    let assets = st.assets.clone();
    let found = tokio::task::spawn_blocking(move || {
        let file = (!rel.is_empty()).then(|| assets.get(&rel)).flatten();
        match file {
            Some(bytes) => Found::File(rel, bytes),
            None if is_spa_route(&rel) => match assets.get(INDEX) {
                Some(bytes) => Found::File(INDEX.to_owned(), bytes),
                None => Found::NoBuild,
            },
            None => Found::Missing,
        }
    })
    .await;
    match found {
        Ok(Found::File(rel, bytes)) => {
            let mime = mime_guess::from_path(&rel).first_or_octet_stream();
            respond(
                StatusCode::OK,
                mime.as_ref(),
                cache_policy(&rel),
                Body::from(bytes.into_owned()),
            )
        }
        Ok(Found::NoBuild) => respond(
            StatusCode::SERVICE_UNAVAILABLE,
            "text/html; charset=utf-8",
            NO_CACHE,
            Body::from(MISSING_PAGE),
        ),
        Ok(Found::Missing) => respond(
            StatusCode::NOT_FOUND,
            "text/plain; charset=utf-8",
            NO_CACHE,
            Body::from("not found\n"),
        ),
        Err(e) => {
            ApiError::new(ErrorCode::Internal, format!("asset read failed: {e}")).into_response()
        }
    }
}

enum Found {
    File(String, Cow<'static, [u8]>),
    NoBuild,
    Missing,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spa_routes_and_cache_policy() {
        assert!(is_spa_route(""));
        assert!(is_spa_route("graph"));
        assert!(is_spa_route("ws/ws_a/learn/c1"));
        assert!(!is_spa_route("favicon.ico"));
        assert!(!is_spa_route("assets/index-abc.js"));
        assert_eq!(cache_policy("assets/index-abc.js"), IMMUTABLE);
        assert_eq!(cache_policy("index.html"), NO_CACHE);
        assert_eq!(cache_policy("favicon.svg"), NO_CACHE);
    }

    #[test]
    fn dir_assets_refuse_traversal() {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().join("dist");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(d.path().join("secret.txt"), "s").unwrap();
        std::fs::write(root.join(INDEX), "<html>").unwrap();
        let a = Assets::Dir(root);
        assert!(a.has_index());
        assert!(a.get("../secret.txt").is_none());
        assert!(a.get("/etc/passwd").is_none());
        assert!(a.get("assets").is_none());
        assert!(!Assets::Dir(d.path().join("nope")).has_index());
    }
}
