//! Wire contract for graph mutations, ingest and archive transfers.
use super::workspace::SourcesDto;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NodeSelection {
    pub node_ids: Vec<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeleteNodes {
    pub node_ids: Vec<String>,
    pub preview_token: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DeletePreview {
    pub deleted: usize,
    pub orphaned_chains: usize,
    pub blast_radius: Vec<String>,
    pub preview_token: String,
}
impl From<lp_service::graph_mut::Preview> for DeletePreview {
    fn from(p: lp_service::graph_mut::Preview) -> Self {
        Self {
            deleted: p.deleted,
            orphaned_chains: p.blast_radius.len(),
            blast_radius: p.blast_radius,
            preview_token: p.token,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphMutation {
    pub version: u64,
    pub layout_version: u64,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PracticeChat {
    pub chat_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct Candidate {
    pub path: String,
    pub sessions: usize,
    pub last_seen: i64,
    pub source: String,
}
impl From<lp_service::ingest_svc::Candidate> for Candidate {
    fn from(c: lp_service::ingest_svc::Candidate) -> Self {
        Self {
            path: c.path,
            sessions: c.sessions,
            last_seen: c.last_seen,
            source: c.source,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct OverlapWarning {
    pub workspace_id: String,
    pub path: String,
    pub other_path: String,
}
impl From<lp_service::ingest_svc::Overlap> for OverlapWarning {
    fn from(w: lp_service::ingest_svc::Overlap) -> Self {
        Self {
            workspace_id: w.workspace_id,
            path: w.path,
            other_path: w.other_path,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IngestSources {
    pub sources: SourcesDto,
    pub enabled: bool,
    pub candidates: Vec<Candidate>,
    pub warnings: Vec<OverlapWarning>,
    pub running_id: Option<String>,
}
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StartIngest {
    pub since_days: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IngestStarted {
    pub run_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct IngestRun {
    pub id: String,
    pub started_at: i64,
    pub finished_at: Option<i64>,
    pub report: super::events::IngestReport,
}
impl From<lp_service::ingest_svc::Run> for IngestRun {
    fn from(r: lp_service::ingest_svc::Run) -> Self {
        Self {
            id: r.id,
            started_at: r.started_at,
            finished_at: r.finished_at,
            report: super::events::IngestReport {
                sessions: r.report.sessions,
                new_decisions: r.report.new_decisions,
                new_cards: r.report.new_cards,
                skipped: r.report.skipped,
                errors: r.report.errors,
            },
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct IngestHistory {
    pub items: Vec<IngestRun>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportStarted {
    pub job_id: String,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ImportedWorkspace {
    pub workspace_id: String,
}

/// Multipart upload contract; the handler streams this file to disk.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ImportArchive {
    #[schema(format = Binary)]
    pub file: String,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::de::DeserializeOwned;
    fn roundtrip<T: Serialize + DeserializeOwned + PartialEq + std::fmt::Debug>(value: T) {
        let json = serde_json::to_value(&value).expect("serialize");
        assert_eq!(
            serde_json::from_value::<T>(json).expect("deserialize"),
            value
        );
    }
    #[test]
    fn mutation_contract_roundtrips_in_camel_case() {
        roundtrip(NodeSelection {
            node_ids: vec!["n1".into()],
        });
        roundtrip(DeleteNodes {
            node_ids: vec!["n1".into()],
            preview_token: "token".into(),
        });
        roundtrip(DeletePreview {
            deleted: 1,
            orphaned_chains: 1,
            blast_radius: vec!["c1".into()],
            preview_token: "token".into(),
        });
        roundtrip(GraphMutation {
            version: 42,
            layout_version: 43,
        });
        roundtrip(PracticeChat {
            chat_id: "chat1".into(),
        });
        roundtrip(IngestSources {
            sources: lp_service::workspace::Sources::default().into(),
            enabled: false,
            candidates: vec![Candidate {
                path: "/repo".into(),
                sessions: 2,
                last_seen: 12,
                source: "claude".into(),
            }],
            warnings: vec![OverlapWarning {
                workspace_id: "ws_12345678".into(),
                path: "/repo".into(),
                other_path: "/repo/nested".into(),
            }],
            running_id: None,
        });
        roundtrip(StartIngest {
            since_days: Some(30),
        });
        roundtrip(IngestStarted {
            run_id: "run1".into(),
        });
        let history = IngestHistory {
            items: vec![IngestRun {
                id: "run1".into(),
                started_at: 1,
                finished_at: Some(2),
                report: super::super::events::IngestReport {
                    sessions: 1,
                    new_decisions: 2,
                    new_cards: 3,
                    skipped: 4,
                    errors: vec![],
                },
            }],
        };
        let json = serde_json::to_value(&history).expect("serialize");
        assert_eq!(json["items"][0]["report"]["newDecisions"], 2);
        assert_eq!(json["items"][0]["startedAt"], 1);
        roundtrip(history);
        roundtrip(ExportStarted {
            job_id: "job1".into(),
        });
        roundtrip(ImportArchive {
            file: "workspace.zip".into(),
        });
        roundtrip(ImportedWorkspace {
            workspace_id: "ws_12345678".into(),
        });
    }
}
