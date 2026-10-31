//! SSE event payloads. `ServerEvent` is the OpenAPI schema of the stream: `type` is the SSE
//! `event:` name and `data` is the JSON on the `data:` line.

use lp_service::events as ev;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct HelloData {
    pub server_time: i64,
    pub version: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkspaceRef {
    pub workspace_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatRef {
    pub chat_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct GraphChangedData {
    pub version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum DeltaKind {
    Text,
    Thinking,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatDeltaData {
    pub chat_id: String,
    pub turn_id: String,
    /// Counts from 1 per turn without gaps; a gap means the client must refetch.
    pub seq: u64,
    pub kind: DeltaKind,
    pub text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ToolCall {
    pub id: String,
    pub name: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatToolCallData {
    pub chat_id: String,
    pub turn_id: String,
    pub call: ToolCall,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatRestData {
    pub chat_id: String,
    pub view_version: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct TurnError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ChatErrorData {
    pub chat_id: String,
    pub turn_id: String,
    pub error: TurnError,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Job {
    pub id: String,
    pub kind: String,
    pub status: String,
    pub title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artifact_id: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct JobUpdatedData {
    pub job: Job,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IngestProgressData {
    pub run_id: String,
    pub done: u64,
    pub total: u64,
    pub phase: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IngestReport {
    pub sessions: u64,
    pub new_decisions: u64,
    pub new_cards: u64,
    pub skipped: u64,
    pub errors: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IngestDoneData {
    pub run_id: String,
    pub report: IngestReport,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportReadyData {
    pub job_id: String,
    pub token: String,
}

/// Payload-less events carry `{}` so `EventSource` still dispatches them.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct Empty {}

/// One server-sent event.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(tag = "type", content = "data")]
pub enum ServerEvent {
    #[serde(rename = "hello")]
    Hello(HelloData),
    #[serde(rename = "workspace.created")]
    WorkspaceCreated(WorkspaceRef),
    #[serde(rename = "workspace.updated")]
    WorkspaceUpdated(WorkspaceRef),
    #[serde(rename = "workspace.deleted")]
    WorkspaceDeleted(WorkspaceRef),
    #[serde(rename = "accounts.changed")]
    AccountsChanged(Empty),
    #[serde(rename = "graph.changed")]
    GraphChanged(GraphChangedData),
    #[serde(rename = "chat.created")]
    ChatCreated(ChatRef),
    #[serde(rename = "chat.updated")]
    ChatUpdated(ChatRef),
    #[serde(rename = "chat.delta")]
    ChatDelta(ChatDeltaData),
    #[serde(rename = "chat.toolCall")]
    ChatToolCall(ChatToolCallData),
    #[serde(rename = "chat.rest")]
    ChatRest(ChatRestData),
    #[serde(rename = "chat.error")]
    ChatError(ChatErrorData),
    #[serde(rename = "job.updated")]
    JobUpdated(JobUpdatedData),
    #[serde(rename = "ingest.progress")]
    IngestProgress(IngestProgressData),
    #[serde(rename = "ingest.done")]
    IngestDone(IngestDoneData),
    #[serde(rename = "review.changed")]
    ReviewChanged(Empty),
    #[serde(rename = "export.ready")]
    ExportReady(ExportReadyData),
    /// Missed events are gone; refetch everything.
    #[serde(rename = "resync")]
    Resync(Empty),
}

impl ServerEvent {
    /// The JSON for the SSE `data:` line.
    pub fn data_json(&self) -> String {
        let v = serde_json::to_value(self).unwrap_or_default();
        v.get("data")
            .map_or_else(|| "{}".to_owned(), serde_json::Value::to_string)
    }
}

impl From<&ev::Event> for ServerEvent {
    fn from(e: &ev::Event) -> Self {
        use ev::Event as E;
        match e.clone() {
            E::Hello {
                server_time,
                version,
            } => Self::Hello(HelloData {
                server_time,
                version,
            }),
            E::WorkspaceCreated { workspace_id } => {
                Self::WorkspaceCreated(WorkspaceRef { workspace_id })
            }
            E::WorkspaceUpdated { workspace_id } => {
                Self::WorkspaceUpdated(WorkspaceRef { workspace_id })
            }
            E::WorkspaceDeleted { workspace_id } => {
                Self::WorkspaceDeleted(WorkspaceRef { workspace_id })
            }
            E::AccountsChanged => Self::AccountsChanged(Empty {}),
            E::GraphChanged { version } => Self::GraphChanged(GraphChangedData { version }),
            E::ChatCreated { chat_id } => Self::ChatCreated(ChatRef { chat_id }),
            E::ChatUpdated { chat_id } => Self::ChatUpdated(ChatRef { chat_id }),
            E::ChatDelta {
                chat_id,
                turn_id,
                seq,
                kind,
                text,
            } => Self::ChatDelta(ChatDeltaData {
                chat_id,
                turn_id,
                seq,
                kind: match kind {
                    ev::DeltaKind::Text => DeltaKind::Text,
                    ev::DeltaKind::Thinking => DeltaKind::Thinking,
                },
                text,
            }),
            E::ChatToolCall {
                chat_id,
                turn_id,
                call,
            } => Self::ChatToolCall(ChatToolCallData {
                chat_id,
                turn_id,
                call: ToolCall {
                    id: call.id,
                    name: call.name,
                },
            }),
            E::ChatRest {
                chat_id,
                view_version,
            } => Self::ChatRest(ChatRestData {
                chat_id,
                view_version,
            }),
            E::ChatError {
                chat_id,
                turn_id,
                error,
            } => Self::ChatError(ChatErrorData {
                chat_id,
                turn_id,
                error: TurnError {
                    code: error.code,
                    message: error.message,
                    retryable: error.retryable,
                },
            }),
            E::JobUpdated { job } => Self::JobUpdated(JobUpdatedData {
                job: Job {
                    id: job.id,
                    kind: job.kind,
                    status: job.status,
                    title: job.title,
                    artifact_id: job.artifact_id,
                },
            }),
            E::IngestProgress {
                run_id,
                done,
                total,
                phase,
            } => Self::IngestProgress(IngestProgressData {
                run_id,
                done,
                total,
                phase,
            }),
            E::IngestDone { run_id, report } => Self::IngestDone(IngestDoneData {
                run_id,
                report: IngestReport {
                    sessions: report.sessions,
                    new_decisions: report.new_decisions,
                    new_cards: report.new_cards,
                    skipped: report.skipped,
                    errors: report.errors,
                },
            }),
            E::ReviewChanged => Self::ReviewChanged(Empty {}),
            E::ExportReady { job_id, token } => {
                Self::ExportReady(ExportReadyData { job_id, token })
            }
            E::Resync => Self::Resync(Empty {}),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn every_event() -> Vec<ev::Event> {
        use ev::Event as E;
        vec![
            E::Hello {
                server_time: 1,
                version: "0.1.0".into(),
            },
            E::WorkspaceCreated {
                workspace_id: "ws_a".into(),
            },
            E::WorkspaceUpdated {
                workspace_id: "ws_a".into(),
            },
            E::WorkspaceDeleted {
                workspace_id: "ws_a".into(),
            },
            E::AccountsChanged,
            E::GraphChanged { version: 3 },
            E::ChatCreated {
                chat_id: "c".into(),
            },
            E::ChatUpdated {
                chat_id: "c".into(),
            },
            E::ChatDelta {
                chat_id: "c".into(),
                turn_id: "t".into(),
                seq: 1,
                kind: ev::DeltaKind::Thinking,
                text: "hm".into(),
            },
            E::ChatToolCall {
                chat_id: "c".into(),
                turn_id: "t".into(),
                call: ev::ToolCallInfo {
                    id: "1".into(),
                    name: "quiz".into(),
                },
            },
            E::ChatRest {
                chat_id: "c".into(),
                view_version: 7,
            },
            E::ChatError {
                chat_id: "c".into(),
                turn_id: "t".into(),
                error: ev::TurnError {
                    code: "engine_unavailable".into(),
                    message: "m".into(),
                    retryable: true,
                },
            },
            E::JobUpdated {
                job: ev::JobInfo {
                    id: "j".into(),
                    kind: "pdf".into(),
                    status: "running".into(),
                    title: "Paper".into(),
                    artifact_id: None,
                },
            },
            E::IngestProgress {
                run_id: "r".into(),
                done: 1,
                total: 2,
                phase: "scan".into(),
            },
            E::IngestDone {
                run_id: "r".into(),
                report: ev::IngestReport {
                    sessions: 1,
                    new_decisions: 2,
                    new_cards: 3,
                    skipped: 4,
                    errors: vec!["x".into()],
                },
            },
            E::ReviewChanged,
            E::ExportReady {
                job_id: "j".into(),
                token: "tok".into(),
            },
            E::Resync,
        ]
    }

    #[test]
    fn every_event_round_trips_and_type_matches_the_sse_name() {
        for e in every_event() {
            let dto = ServerEvent::from(&e);
            let v = serde_json::to_value(&dto).unwrap();
            assert_eq!(v["type"], e.name(), "{e:?}");
            assert_eq!(serde_json::from_value::<ServerEvent>(v).unwrap(), dto);
            let data: serde_json::Value = serde_json::from_str(&dto.data_json()).unwrap();
            assert!(data.is_object(), "{e:?} data must be an object");
        }
    }

    #[test]
    fn payloads_are_camel_case() {
        let d = ServerEvent::from(&ev::Event::ChatRest {
            chat_id: "c".into(),
            view_version: 2,
        });
        let json = |s: String| serde_json::from_str::<serde_json::Value>(&s).unwrap();
        assert_eq!(
            json(d.data_json()),
            serde_json::json!({ "chatId": "c", "viewVersion": 2 })
        );
        assert_eq!(ServerEvent::Resync(Empty {}).data_json(), "{}");
        let d = ServerEvent::from(&ev::Event::ChatDelta {
            chat_id: "c".into(),
            turn_id: "t".into(),
            seq: 4,
            kind: ev::DeltaKind::Text,
            text: "hi".into(),
        });
        assert_eq!(
            json(d.data_json()),
            serde_json::json!({ "chatId": "c", "turnId": "t", "seq": 4, "kind": "text", "text": "hi" })
        );
    }
}
