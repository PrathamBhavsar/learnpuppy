//! `GET /events?workspace=ws_x` (docs/web/03 §SSE protocol).
//!
//! A stream opens with `hello`. A fresh connection's `hello` carries the current id so a
//! reconnect resumes from there; a reconnect (`Last-Event-ID`) gets `hello` without an id, then
//! the missed events, or `resync` when they are gone. Live events follow in id order, filtered
//! to global + the stream's workspace. A receiver that falls behind the broadcast buffer gets
//! `resync`. A comment heartbeat keeps proxies from closing idle streams. Streams end on
//! shutdown.

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::sse::{Event as SseEvent, KeepAlive, Sse};
use axum::routing::get;
use axum::Router;
use futures::{Stream, StreamExt};
use lp_service::events::{Event, EventHub, Replay, Stored};
use serde::Deserialize;
use tokio::sync::broadcast::error::RecvError;
use tokio::sync::broadcast::Receiver;
use utoipa::IntoParams;

use crate::app::{AppState, VERSION};
use crate::dto::events::ServerEvent;
use crate::error::{ApiError, ApiQuery};

pub const LAST_EVENT_ID: &str = "last-event-id";

pub fn routes() -> Router<AppState> {
    Router::new().route("/events", get(events))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct EventsQuery {
    /// Also receive this workspace's events; without it only global events arrive.
    pub workspace: Option<String>,
}

fn valid_workspace_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
}

/// One SSE frame; `id` is omitted for frames that must not move the client's cursor.
fn frame(event: &Event, id: Option<u64>) -> SseEvent {
    let f = SseEvent::default()
        .event(event.name())
        .data(ServerEvent::from(event).data_json());
    match id {
        Some(id) => f.id(id.to_string()),
        None => f,
    }
}

struct Live {
    rx: Receiver<Arc<Stored>>,
    hub: Arc<EventHub>,
    workspace: Option<String>,
    /// Highest id sent; anything at or below it is skipped.
    floor: u64,
}

impl Live {
    async fn next(mut self) -> Option<(SseEvent, Self)> {
        loop {
            match self.rx.recv().await {
                Ok(e) if e.id > self.floor && e.visible_to(self.workspace.as_deref()) => {
                    self.floor = e.id;
                    return Some((frame(&e.event, Some(e.id)), self));
                }
                Ok(_) => {}
                Err(RecvError::Lagged(n)) => {
                    tracing::warn!(missed = n, "sse stream lagged; sending resync");
                    self.floor = self.hub.last_id();
                    return Some((frame(&Event::Resync, Some(self.floor)), self));
                }
                Err(RecvError::Closed) => return None,
            }
        }
    }
}

/// The frames of one stream: `hello`, replay or `resync`, then live events.
pub fn event_stream(
    hub: Arc<EventHub>,
    workspace: Option<String>,
    last_event_id: Option<u64>,
) -> impl Stream<Item = SseEvent> + Send + 'static {
    let sub = hub.subscribe(workspace.as_deref(), last_event_id);
    let hello = Event::Hello {
        server_time: lp_service::now_ms(),
        version: VERSION.to_owned(),
    };
    let mut head = Vec::new();
    match (last_event_id, sub.replay) {
        (None, _) => head.push(frame(&hello, Some(sub.cursor))),
        (Some(_), Replay::Events(missed)) => {
            head.push(frame(&hello, None));
            head.extend(missed.iter().map(|e| frame(&e.event, Some(e.id))));
        }
        (Some(_), Replay::Resync { id }) => {
            head.push(frame(&hello, None));
            head.push(frame(&Event::Resync, Some(id)));
        }
    }
    let live = Live {
        rx: sub.live,
        hub,
        workspace,
        floor: sub.cursor,
    };
    futures::stream::iter(head).chain(futures::stream::unfold(live, Live::next))
}

/// Server-sent events.
#[utoipa::path(
    get,
    path = "/events",
    tag = "events",
    params(
        EventsQuery,
        ("Last-Event-ID" = Option<String>, Header, description = "Resume after this event id"),
    ),
    responses(
        (status = 200, description = "`text/event-stream`; each frame's `event:` is `type` and `data:` is `data`",
         content_type = "text/event-stream", body = ServerEvent),
        (status = 400, description = "Bad workspace id", body = crate::error::ErrorEnvelope),
        (status = 401, description = "No session", body = crate::error::ErrorEnvelope),
    )
)]
pub async fn events(
    State(st): State<AppState>,
    ApiQuery(q): ApiQuery<EventsQuery>,
    headers: HeaderMap,
) -> Result<Sse<impl Stream<Item = Result<SseEvent, Infallible>>>, ApiError> {
    if let Some(ws) = q.workspace.as_deref().filter(|w| !valid_workspace_id(w)) {
        return Err(ApiError::invalid(
            "workspace",
            format!("not a workspace id: {ws:?}"),
        ));
    }
    // An unparsable id is treated as unknown, which replays nothing and sends `resync`.
    let last = headers
        .get(LAST_EVENT_ID)
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim().parse::<u64>().unwrap_or(0));
    let stream = event_stream(st.hub.clone(), q.workspace, last)
        .map(Ok)
        .take_until(st.shutdown.clone().cancelled_owned());
    Ok(Sse::new(stream).keep_alive(
        KeepAlive::new()
            .interval(st.config.heartbeat)
            .text("heartbeat"),
    ))
}
