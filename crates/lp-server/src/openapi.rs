//! The OpenAPI document (utoipa). `learnpuppy openapi` prints it; `make api-gen` writes
//! `web/openapi/openapi.json` and `make api-check` fails when the committed file drifts.

use utoipa::OpenApi;

use crate::dto::{events, system, textbook};
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
        crate::routes::search::search,
        crate::sse::events,
        crate::routes::workspaces::list,
        crate::routes::workspaces::create,
        crate::routes::workspaces::get_one,
        crate::routes::workspaces::patch,
        crate::routes::workspaces::archive,
        crate::routes::workspaces::restore,
        crate::routes::workspaces::repair,
        crate::routes::workspaces::remove,
        crate::routes::workspaces::trash_list,
        crate::routes::workspaces::trash_restore,
        crate::routes::workspaces::trash_purge,
        crate::routes::graph::snapshot,
        crate::routes::graph::node,
        crate::routes::graph::search,
        crate::routes::graph::chains,
        crate::routes::chats_write::create,
        crate::routes::chats_write::turn,
        crate::routes::chats_write::cancel,
        crate::routes::chats_write::export,
        crate::routes::jobs::list,
        crate::routes::jobs::abort,
        crate::routes::artifacts::artifact,
        crate::routes::sources::pdf,
        crate::routes::sources::note,
        crate::routes::sources::textbook,
        crate::routes::textbook::list,
        crate::routes::textbook::book,
        crate::routes::textbook::chapter,
        crate::routes::textbook::import,
        crate::routes::textbook::check,
        crate::routes::textbook::answer,
        crate::routes::textbook::self_grade,
        crate::routes::textbook::usage,
        crate::routes::graph_write::preview,
        crate::routes::graph_write::remove,
        crate::routes::graph_write::relayout,
        crate::routes::graph_write::practice,
        crate::routes::ingest::sources,
        crate::routes::ingest::save_sources,
        crate::routes::ingest::run,
        crate::routes::ingest::runs,
        crate::routes::ingest::cancel,
        crate::routes::exports::export,
        crate::routes::exports::download,
        crate::routes::exports::import,
        crate::routes::chats::list,
        crate::routes::chats::view,
        crate::routes::chats::rename,
        crate::routes::chats::remove,
        crate::routes::review::queue,
        crate::routes::review::answer,
        crate::routes::review::grade,
        crate::routes::review::preview,
        crate::routes::review::stats,
        crate::routes::review::cards,
        crate::routes::review::card_detail,
        crate::routes::review::sync,
        crate::routes::courses::list,
        crate::routes::courses::create,
        crate::routes::courses::placement,
        crate::routes::courses::pending_placement,
        crate::routes::courses::misses,
        crate::routes::courses::status,
        crate::routes::courses::retry,
        crate::routes::courses::next,
        crate::routes::courses::seen,
        crate::routes::courses::answer,
        crate::routes::courses::self_grade,
        crate::routes::courses::flag,
        crate::routes::courses::ask,
        crate::routes::courses::check,
        crate::routes::courses::usage,
        crate::routes::learning::plan_get,
        crate::routes::learning::plan_confirm,
        crate::routes::learning::plan_prereq,
        crate::routes::learning::step,
        crate::routes::learning::predict,
        crate::routes::learning::explain_seen,
        crate::routes::learning::note,
        crate::routes::learning::quiz_answer,
        crate::routes::learning::feynman,
        crate::routes::learning::take_break,
        crate::routes::learning::close_session,
        crate::routes::learning::braindump,
        crate::routes::learning::review_dates,
        crate::routes::learning::save_review_dates,
        crate::routes::learning::register,
        crate::routes::learning::mastery,
        crate::routes::learning::clock,
        crate::routes::learning::bout_start,
        crate::routes::learning::bout_heartbeat,
        crate::routes::learning::bout_end,
        crate::routes::learning::bout_focus_lost,
        crate::routes::learning::bout_override,
        crate::routes::learning::checkin_needed,
        crate::routes::learning::checkin,
        crate::routes::learning::method_settings_get,
        crate::routes::learning::method_settings_set,
        crate::routes::learning::schedule_get,
        crate::routes::learning::schedule_set,
        crate::routes::learning::schedule_suggest,
        crate::routes::learning::schedule_track,
        crate::routes::learning::profile_get,
        crate::routes::learning::profile_set,
        crate::routes::digest::digest,
        crate::routes::digest::decision,
        crate::routes::digest::regenerate,
        crate::routes::accounts::list,
        crate::routes::accounts::rescan,
        crate::routes::accounts::add_claude,
        crate::routes::accounts::patch_account,
        crate::routes::accounts::patch_account_model,
        crate::routes::accounts::apply_free_defaults,
        crate::routes::accounts::get_settings,
        crate::routes::accounts::put_settings,
        crate::routes::accounts::catalog,
        crate::routes::accounts::test_profile,
    ),
    components(schemas(
        ErrorEnvelope,
        ErrorBody,
        ErrorCode,
        FieldError,
        system::EngineMode,
        events::ServerEvent,
        textbook::TextbookUploadDto,
        textbook::TextbookUploadForm,
        textbook::TextbookListDto,
        textbook::TextbookSummaryDto,
        textbook::BookPhaseDto,
        textbook::TextbookDto,
        textbook::ChapterRowDto,
        textbook::MapSourceDto,
        textbook::ChapterKindDto,
        textbook::ChapterStateDto,
        textbook::ChapterStatusDto,
        textbook::ChapterDetailDto,
        textbook::ObjectiveRowDto,
        textbook::ImportRequest,
        textbook::ImportStartedDto,
        textbook::CheckItemDto,
        textbook::ReadingCheckDto,
        textbook::CheckAnswerRequest,
        textbook::CheckFeedbackDto,
        textbook::TextbookUsageDto,
    )),
    tags(
        (name = "system", description = "Health and app bootstrap"),
        (name = "events", description = "Server-sent events"),
        (name = "workspaces", description = "Workspace lifecycle and trash"),
        (name = "graph", description = "Knowledge graph"),
        (name = "chats", description = "Learn chats and the Classroom view"),
        (name = "review", description = "FSRS spaced repetition"),
        (name = "courses", description = "Courses: stored lessons, placement, follow-ups"),
        (name = "textbooks", description = "Textbook mode: PDF import, chapters, reading checks"),
        (name = "digest", description = "Decision digest"),
        (name = "accounts", description = "CLI accounts"),
        (name = "settings", description = "Global settings"),
    )
)]
pub struct ApiDoc;

/// The document, pretty JSON with a trailing newline (stable across runs).
pub fn document_json() -> serde_json::Result<String> {
    let mut doc = ApiDoc::openapi();
    doc.info.version = crate::app::VERSION.to_owned();
    let mut value = serde_json::to_value(&doc)?;
    unique_operation_ids(&mut value);
    let mut s = serde_json::to_string_pretty(&value)?;
    s.push('\n');
    Ok(s)
}

/// utoipa names operations after their handler (`list`, `create`, ...), which collide across modules and
/// break client generators; `get_workspaces_ws_chats` is unique by construction.
fn unique_operation_ids(doc: &mut serde_json::Value) {
    let Some(paths) = doc["paths"].as_object_mut() else {
        return;
    };
    for (path, item) in paths {
        let slug: String = path
            .split(|c: char| !c.is_ascii_alphanumeric())
            .filter(|s| !s.is_empty())
            .collect::<Vec<_>>()
            .join("_");
        let Some(methods) = item.as_object_mut() else {
            continue;
        };
        for (method, op) in methods {
            if let Some(op) = op.as_object_mut() {
                op.insert("operationId".into(), format!("{method}_{slug}").into());
            }
        }
    }
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
