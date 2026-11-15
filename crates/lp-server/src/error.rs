//! The error envelope (docs/web/03 §Conventions), request ids, and extractors whose rejections
//! use the envelope.
//!
//! Every response carries `X-Request-Id`; an error body echoes it as `requestId` so the UI's
//! "Copy diagnostics" can point at the log line.

use std::time::Instant;

use axum::extract::{FromRequest, FromRequestParts, Request};
use axum::http::header::CONTENT_TYPE;
use axum::http::request::Parts;
use axum::http::{HeaderValue, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

pub const REQUEST_ID_HEADER: &str = "x-request-id";

/// Machine-readable error codes; the UI switches on these, never on messages.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NotFound,
    Validation,
    Conflict,
    TurnInProgress,
    StaleQuestion,
    WorkspaceArchived,
    StorageFull,
    EngineUnavailable,
    /// `details.retryAfterMs` says when to retry.
    RateLimited,
    AuthRequired,
    /// Cross-origin or missing `X-LP-CSRF` on a mutation.
    Forbidden,
    /// `Host` is not this server (DNS rebinding guard).
    BadHost,
    MethodNotAllowed,
    Internal,
}

impl ErrorCode {
    pub fn status(self) -> StatusCode {
        match self {
            ErrorCode::NotFound => StatusCode::NOT_FOUND,
            ErrorCode::Validation => StatusCode::BAD_REQUEST,
            ErrorCode::Conflict
            | ErrorCode::TurnInProgress
            | ErrorCode::StaleQuestion
            | ErrorCode::WorkspaceArchived => StatusCode::CONFLICT,
            ErrorCode::StorageFull => StatusCode::INSUFFICIENT_STORAGE,
            ErrorCode::EngineUnavailable => StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            ErrorCode::AuthRequired => StatusCode::UNAUTHORIZED,
            ErrorCode::Forbidden => StatusCode::FORBIDDEN,
            ErrorCode::BadHost => StatusCode::MISDIRECTED_REQUEST,
            ErrorCode::MethodNotAllowed => StatusCode::METHOD_NOT_ALLOWED,
            ErrorCode::Internal => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }

    /// Whether the same request may succeed later without changes.
    pub fn retryable(self) -> bool {
        matches!(
            self,
            ErrorCode::EngineUnavailable | ErrorCode::RateLimited | ErrorCode::Internal
        )
    }
}

/// One invalid input field; `path` is dotted (`profiles.teacher.model`, `items[2].id`).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct FieldError {
    pub path: String,
    pub message: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorBody {
    pub code: ErrorCode,
    pub message: String,
    /// Code-specific data: `fields` for `validation`, `retryAfterMs` for `rate_limited`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[schema(value_type = Option<Object>)]
    pub details: Option<serde_json::Value>,
    pub retryable: bool,
    pub request_id: String,
}

/// Body of every non-2xx API response.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

/// An API failure; renders as [`ErrorEnvelope`] with the code's status.
#[derive(Debug)]
pub struct ApiError {
    pub code: ErrorCode,
    pub message: String,
    pub details: Option<serde_json::Value>,
}

impl ApiError {
    pub fn new(code: ErrorCode, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            details: None,
        }
    }

    pub fn with_details(mut self, details: serde_json::Value) -> Self {
        self.details = Some(details);
        self
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(ErrorCode::NotFound, message)
    }

    /// Invalid input; `details.fields` lists each bad field.
    pub fn validation(fields: Vec<FieldError>) -> Self {
        let message = match fields.as_slice() {
            [f] if f.path.is_empty() => f.message.clone(),
            [f] => format!("{}: {}", f.path, f.message),
            _ => format!("{} invalid fields", fields.len()),
        };
        Self::new(ErrorCode::Validation, message)
            .with_details(serde_json::json!({ "fields": fields }))
    }

    pub fn invalid(path: impl Into<String>, message: impl Into<String>) -> Self {
        Self::validation(vec![FieldError {
            path: path.into(),
            message: message.into(),
        }])
    }

    /// A gate rejection (409 `conflict`): `gate` is the stable, machine-readable code the UI
    /// switches on (docs/web/03 §Conventions), one per distinct learner-order violation.
    pub fn gate(gate: &str, message: impl Into<String>) -> Self {
        Self::new(ErrorCode::Conflict, message).with_details(serde_json::json!({ "gate": gate }))
    }

    pub fn rate_limited(retry_after_ms: u64) -> Self {
        Self::new(ErrorCode::RateLimited, "all accounts are rate limited")
            .with_details(serde_json::json!({ "retryAfterMs": retry_after_ms }))
    }

    pub fn internal(e: &anyhow::Error) -> Self {
        tracing::error!(error = format!("{e:#}"), "internal error");
        Self::new(ErrorCode::Internal, format!("{e:#}"))
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(e: anyhow::Error) -> Self {
        Self::internal(&e)
    }
}

impl From<tokio::task::JoinError> for ApiError {
    fn from(e: tokio::task::JoinError) -> Self {
        Self::internal(&anyhow::Error::new(e).context("background task failed"))
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorEnvelope {
            error: ErrorBody {
                code: self.code,
                message: self.message,
                details: self.details,
                retryable: self.code.retryable(),
                request_id: current_request_id().unwrap_or_default(),
            },
        };
        (self.code.status(), Json(body)).into_response()
    }
}

tokio::task_local! {
    static REQUEST_ID: String;
}

/// The id of the request being handled, inside [`request_id`].
pub fn current_request_id() -> Option<String> {
    REQUEST_ID.try_with(Clone::clone).ok()
}

/// A caller-supplied id is kept if it is short and plain, else a fresh one is minted.
fn pick_request_id(incoming: Option<&HeaderValue>) -> String {
    incoming
        .and_then(|v| v.to_str().ok())
        .filter(|s| {
            !s.is_empty()
                && s.len() <= 64
                && s.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        })
        .map_or_else(|| uuid::Uuid::new_v4().simple().to_string(), str::to_owned)
}

/// Outermost middleware: assigns the request id, echoes it as `X-Request-Id`, logs the request.
/// Only the path is logged; the query may hold the launch token.
pub async fn request_id(req: Request, next: Next) -> Response {
    let id = pick_request_id(req.headers().get(REQUEST_ID_HEADER));
    let method = req.method().clone();
    let path = req.uri().path().to_owned();
    let started = Instant::now();
    let mut res = REQUEST_ID.scope(id.clone(), next.run(req)).await;
    if let Ok(v) = HeaderValue::from_str(&id) {
        res.headers_mut().insert(REQUEST_ID_HEADER, v);
    }
    tracing::info!(
        request_id = %id,
        %method,
        path,
        status = res.status().as_u16(),
        ms = started.elapsed().as_millis() as u64,
        "request"
    );
    res
}

/// The path as the client sent it (nested routers see it with their prefix stripped).
fn full_path(req: &Request) -> &str {
    req.extensions()
        .get::<axum::extract::OriginalUri>()
        .map_or(req.uri().path(), |u| u.0.path())
}

/// Fallback for unknown `/api/...` paths.
pub async fn api_not_found(req: Request) -> ApiError {
    ApiError::not_found(format!("no route for {} {}", req.method(), full_path(&req)))
}

pub async fn method_not_allowed(req: Request) -> ApiError {
    ApiError::new(
        ErrorCode::MethodNotAllowed,
        format!("{} is not allowed on {}", req.method(), full_path(&req)),
    )
}

/// `serde_path_to_error` paths render the root as `.`; the envelope uses an empty path.
fn field_path(p: &serde_path_to_error::Path) -> String {
    let s = p.to_string();
    if s == "." {
        String::new()
    } else {
        s
    }
}

/// JSON body extractor whose errors are `validation` envelopes naming the bad field.
#[derive(Debug, Clone, Copy, Default)]
pub struct ApiJson<T>(pub T);

impl<S, T> FromRequest<S> for ApiJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request(req: Request, state: &S) -> Result<Self, ApiError> {
        let is_json = req
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.split(';').next())
            .is_some_and(|m| m.trim().eq_ignore_ascii_case("application/json"));
        if !is_json {
            return Err(ApiError::invalid(
                "",
                "expected a JSON body (Content-Type: application/json)",
            ));
        }
        let bytes = axum::body::Bytes::from_request(req, state)
            .await
            .map_err(|e| ApiError::invalid("", e.body_text()))?;
        let de = &mut serde_json::Deserializer::from_slice(&bytes);
        serde_path_to_error::deserialize(de)
            .map(ApiJson)
            .map_err(|e| ApiError::invalid(field_path(e.path()), e.inner().to_string()))
    }
}

impl<T: Serialize> IntoResponse for ApiJson<T> {
    fn into_response(self) -> Response {
        Json(self.0).into_response()
    }
}

/// Query string extractor whose errors are `validation` envelopes.
#[derive(Debug, Clone, Copy, Default)]
pub struct ApiQuery<T>(pub T);

impl<S, T> FromRequestParts<S> for ApiQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, ApiError> {
        axum::extract::Query::<T>::from_request_parts(parts, state)
            .await
            .map(|q| ApiQuery(q.0))
            .map_err(|e| ApiError::invalid("query", e.body_text()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::routing::{get, post};
    use axum::Router;
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct Body1 {
        name: String,
        limit: u32,
    }

    #[derive(Deserialize)]
    #[allow(dead_code)]
    struct Q {
        limit: u32,
    }

    fn app() -> Router {
        Router::new()
            .route("/j", post(|ApiJson(_b): ApiJson<Body1>| async { "ok" }))
            .route("/q", get(|ApiQuery(_q): ApiQuery<Q>| async { "ok" }))
            .route(
                "/boom",
                get(|| async { Err::<(), _>(ApiError::from(anyhow::anyhow!("disk on fire"))) }),
            )
            .layer(axum::middleware::from_fn(request_id))
    }

    async fn call(req: Request) -> (StatusCode, Option<String>, ErrorEnvelope) {
        let res = app().oneshot(req).await.unwrap();
        let status = res.status();
        let rid = res
            .headers()
            .get(REQUEST_ID_HEADER)
            .map(|v| v.to_str().unwrap().to_owned());
        let body = res.into_body().collect().await.unwrap().to_bytes();
        (status, rid, serde_json::from_slice(&body).unwrap())
    }

    fn json_post(body: &str) -> Request {
        Request::post("/j")
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(body.to_owned()))
            .unwrap()
    }

    #[tokio::test]
    async fn validation_names_the_bad_field() {
        let (status, rid, env) = call(json_post(r#"{"name":"a","limit":"x"}"#)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(env.error.code, ErrorCode::Validation);
        assert!(!env.error.retryable);
        let fields = &env.error.details.as_ref().unwrap()["fields"];
        assert_eq!(fields[0]["path"], "limit");
        assert!(fields[0]["message"]
            .as_str()
            .unwrap()
            .contains("invalid type"));
        assert_eq!(rid.as_deref(), Some(env.error.request_id.as_str()));
        assert_eq!(env.error.request_id.len(), 32);

        let (_, _, env) = call(json_post(r#"{"name":"a"}"#)).await;
        assert!(env.error.message.contains("missing field `limit`"));
        let (_, _, env) = call(json_post("{not json")).await;
        assert_eq!(env.error.code, ErrorCode::Validation);

        let req = Request::post("/j").body(Body::from("{}")).unwrap();
        let (status, _, env) = call(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert!(env.error.message.contains("application/json"));

        let req = Request::get("/q?limit=-1").body(Body::empty()).unwrap();
        let (status, _, env) = call(req).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(env.error.details.unwrap()["fields"][0]["path"], "query");
    }

    #[tokio::test]
    async fn internal_errors_are_retryable_and_keep_a_caller_request_id() {
        let req = Request::get("/boom")
            .header(REQUEST_ID_HEADER, "trace-42")
            .body(Body::empty())
            .unwrap();
        let (status, rid, env) = call(req).await;
        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(
            (rid.as_deref(), env.error.request_id.as_str()),
            (Some("trace-42"), "trace-42")
        );
        assert_eq!(env.error.code, ErrorCode::Internal);
        assert!(env.error.retryable);
        assert_eq!(env.error.message, "disk on fire");

        // An unsafe incoming id is replaced.
        let req = Request::get("/boom")
            .header(REQUEST_ID_HEADER, "a b\"c")
            .body(Body::empty())
            .unwrap();
        let (_, rid, _) = call(req).await;
        assert_eq!(rid.unwrap().len(), 32);
    }

    #[test]
    fn envelope_round_trips_with_camel_case_and_snake_codes() {
        let env = ErrorEnvelope {
            error: ErrorBody {
                code: ErrorCode::RateLimited,
                message: "m".into(),
                details: Some(serde_json::json!({ "retryAfterMs": 5 })),
                retryable: true,
                request_id: "r".into(),
            },
        };
        let v = serde_json::to_value(&env).unwrap();
        assert_eq!(v["error"]["code"], "rate_limited");
        assert_eq!(v["error"]["requestId"], "r");
        assert_eq!(serde_json::from_value::<ErrorEnvelope>(v).unwrap(), env);
        assert_eq!(
            ApiError::rate_limited(5).details.unwrap()["retryAfterMs"],
            5
        );
    }

    #[test]
    fn codes_map_to_their_statuses() {
        assert_eq!(ErrorCode::BadHost.status().as_u16(), 421);
        assert_eq!(ErrorCode::TurnInProgress.status().as_u16(), 409);
        assert_eq!(ErrorCode::AuthRequired.status().as_u16(), 401);
        assert_eq!(ErrorCode::Forbidden.status().as_u16(), 403);
        assert_eq!(ErrorCode::StorageFull.status().as_u16(), 507);
    }
}
