//! The OpenAPI document (utoipa). `learnpuppy openapi` prints it; `make api-gen` writes
//! `web/openapi/openapi.json` and `make api-check` fails when the committed file drifts.

use utoipa::OpenApi;

use crate::dto::{events, system};
use crate::error::{ErrorBody, ErrorCode, ErrorEnvelope, FieldError};

#[derive(OpenApi)]
#[openapi(
    info(
        title = "LearnPuppy",
        description = "Local LearnPuppy server API. Session cookie `lp_session` from the launch link; mutations need same Origin and `X-LP-CSRF: 1`."
    ),
    servers((url = "/api/v1")),
    paths(
        crate::routes::system::health,
        crate::routes::system::bootstrap,
        crate::sse::events,
    ),
    components(schemas(
        ErrorEnvelope,
        ErrorBody,
        ErrorCode,
        FieldError,
        system::EngineMode,
        events::ServerEvent,
    )),
    tags(
        (name = "system", description = "Health and app bootstrap"),
        (name = "events", description = "Server-sent events"),
    )
)]
pub struct ApiDoc;

/// The document, pretty JSON with a trailing newline (stable across runs).
pub fn document_json() -> serde_json::Result<String> {
    let mut doc = ApiDoc::openapi();
    doc.info.version = crate::app::VERSION.to_owned();
    let mut s = serde_json::to_string_pretty(&doc)?;
    s.push('\n');
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn document_lists_routes_schemas_and_error_codes() {
        let doc: serde_json::Value = serde_json::from_str(&document_json().unwrap()).unwrap();
        assert!(doc["openapi"].as_str().unwrap().starts_with("3.1"));
        assert_eq!(doc["info"]["version"], crate::app::VERSION);
        assert_eq!(doc["servers"][0]["url"], "/api/v1");
        for p in ["/health", "/bootstrap", "/events"] {
            assert!(doc["paths"][p]["get"].is_object(), "{p}");
        }
        let schemas = &doc["components"]["schemas"];
        for s in [
            "ErrorEnvelope",
            "ErrorCode",
            "Health",
            "Bootstrap",
            "ServerEvent",
            "AccountSummaryDto",
        ] {
            assert!(schemas[s].is_object(), "{s}");
        }
        let codes = schemas["ErrorCode"].to_string();
        for c in [
            "not_found",
            "turn_in_progress",
            "rate_limited",
            "auth_required",
            "bad_host",
        ] {
            assert!(codes.contains(c), "{c}");
        }
        assert!(schemas["ServerEvent"].to_string().contains("chat.delta"));
        assert_eq!(document_json().unwrap(), document_json().unwrap());
    }
}
