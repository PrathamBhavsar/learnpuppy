//! Request guard (docs/web/01 §Security model), in order:
//! 1. `Host` must be this server (`127.0.0.1:PORT`, `localhost:PORT`, `[::1]:PORT`) → else 421.
//! 2. `GET /?t=<token>` trades the launch token for the `lp_session` cookie and redirects to
//!    `/` without the token.
//! 3. `/api/*` (except `/api/v1/health`) needs the cookie → else 401. `--dev` skips this only.
//! 4. Mutations need `Origin` equal to our own origin and `X-LP-CSRF: 1` → else 403.
//!
//! No CORS headers are ever sent.

use axum::extract::{Request, State};
use axum::http::header::{CACHE_CONTROL, COOKIE, HOST, LOCATION, ORIGIN, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};

use crate::app::AppState;
use crate::error::{ApiError, ErrorCode};

pub const SESSION_COOKIE: &str = "lp_session";
pub const CSRF_HEADER: &str = "x-lp-csrf";
pub const TOKEN_PARAM: &str = "t";
const PUBLIC_API: &[&str] = &["/api/v1/health"];

/// A fresh 256-bit launch token, hex encoded.
pub fn new_token() -> String {
    hex::encode(rand::random::<[u8; 32]>())
}

/// Hex SHA-256 of a token, as stored in the lock file.
pub fn token_hash(token: &str) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(token.as_bytes()))
}

fn same_secret(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// The request's authority: `Host`, or the URI authority (HTTP/2).
fn request_host(req: &Request) -> Option<String> {
    req.headers()
        .get(HOST)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned)
        .or_else(|| req.uri().authority().map(|a| a.as_str().to_owned()))
}

/// Whether `host` names this server on `port`.
pub fn host_allowed(host: &str, port: u16) -> bool {
    let Some((name, p)) = host.rsplit_once(':') else {
        return false;
    };
    p.parse::<u16>() == Ok(port)
        && (name == "127.0.0.1" || name.eq_ignore_ascii_case("localhost") || name == "[::1]")
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .filter_map(|kv| kv.trim().split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

fn query_param<'a>(query: &'a str, name: &str) -> Option<&'a str> {
    query
        .split('&')
        .filter_map(|kv| kv.split_once('='))
        .find(|(k, _)| *k == name)
        .map(|(_, v)| v)
}

fn is_mutation(m: &Method) -> bool {
    !matches!(*m, Method::GET | Method::HEAD | Method::OPTIONS)
}

/// `GET /?t=…`: set the cookie if the token is right, and always drop the token from the URL.
fn token_exchange(expected: Option<&str>, given: &str) -> Response {
    let mut res = (StatusCode::SEE_OTHER, [(LOCATION, "/")]).into_response();
    res.headers_mut()
        .insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    if let Some(token) = expected.filter(|t| same_secret(t, given)) {
        let cookie = format!("{SESSION_COOKIE}={token}; HttpOnly; SameSite=Strict; Path=/");
        if let Ok(v) = HeaderValue::from_str(&cookie) {
            res.headers_mut().insert(SET_COOKIE, v);
        }
    }
    res
}

pub async fn guard(State(st): State<AppState>, req: Request, next: Next) -> Response {
    let port = st.config.port;
    let Some(host) = request_host(&req).filter(|h| host_allowed(h, port)) else {
        return ApiError::new(
            ErrorCode::BadHost,
            format!("this server only answers to 127.0.0.1:{port} or localhost:{port}"),
        )
        .into_response();
    };
    let path = req.uri().path();
    let token = st.config.token.as_deref();

    if req.method() == Method::GET && path == "/" {
        if let Some(given) = req.uri().query().and_then(|q| query_param(q, TOKEN_PARAM)) {
            return token_exchange(token, given);
        }
    }

    if let Some(token) = token {
        let needs_session = path.starts_with("/api/") && !PUBLIC_API.contains(&path);
        let has_session =
            cookie_value(req.headers(), SESSION_COOKIE).is_some_and(|c| same_secret(c, token));
        if needs_session && !has_session {
            return ApiError::new(
                ErrorCode::AuthRequired,
                "open LearnPuppy from the link printed by `learnpuppy serve`",
            )
            .into_response();
        }
    }

    if is_mutation(req.method()) {
        let own_origin = format!("http://{host}");
        let origin_ok = req
            .headers()
            .get(ORIGIN)
            .and_then(|v| v.to_str().ok())
            .is_some_and(|o| o.eq_ignore_ascii_case(&own_origin));
        if !origin_ok {
            return ApiError::new(ErrorCode::Forbidden, "cross-origin request refused")
                .into_response();
        }
        let csrf_ok = req
            .headers()
            .get(CSRF_HEADER)
            .is_some_and(|v| v.as_bytes() == b"1");
        if !csrf_ok {
            return ApiError::new(ErrorCode::Forbidden, "missing X-LP-CSRF: 1 header")
                .into_response();
        }
    }

    next.run(req).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_allowlist_is_exact() {
        assert!(host_allowed("127.0.0.1:7878", 7878));
        assert!(host_allowed("localhost:7878", 7878));
        assert!(host_allowed("LOCALHOST:7878", 7878));
        assert!(host_allowed("[::1]:7878", 7878));
        for bad in [
            "127.0.0.1:7879",
            "127.0.0.1",
            "evil.com:7878",
            "localhost.evil.com:7878",
            "0.0.0.0:7878",
            "192.168.1.2:7878",
            ":7878",
        ] {
            assert!(!host_allowed(bad, 7878), "{bad}");
        }
    }

    #[test]
    fn tokens_are_random_and_hashed() {
        let (a, b) = (new_token(), new_token());
        assert_ne!(a, b);
        assert_eq!(a.len(), 64);
        assert_eq!(token_hash(&a).len(), 64);
        assert_ne!(token_hash(&a), a);
        assert!(same_secret(&a, &a.clone()));
        assert!(!same_secret(&a, &b));
        assert!(!same_secret("ab", "abc"));
    }

    #[test]
    fn parses_cookies_and_query() {
        let mut h = HeaderMap::new();
        h.append(COOKIE, HeaderValue::from_static("a=1; lp_session=xyz"));
        h.append(COOKIE, HeaderValue::from_static("b=2"));
        assert_eq!(cookie_value(&h, SESSION_COOKIE), Some("xyz"));
        assert_eq!(cookie_value(&h, "b"), Some("2"));
        assert_eq!(cookie_value(&h, "c"), None);
        assert_eq!(query_param("x=1&t=abc", "t"), Some("abc"));
        assert_eq!(query_param("tt=abc", "t"), None);
    }
}
