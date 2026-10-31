//! Decision digest wire types (docs/web/03 §Digest).

use lp_core::ports::{CodingSession, DecisionBasis};
use lp_ingest::GitCommit;
use lp_service::digest::{CardLinks, DayGroup, DecisionRow, Regenerated, SessionGroup};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use super::review::CardBriefDto;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum BasisDto {
    /// The learner or agent said why.
    Stated,
    /// The distiller inferred the reason.
    Inferred,
}

impl From<DecisionBasis> for BasisDto {
    fn from(b: DecisionBasis) -> Self {
        match b {
            DecisionBasis::Stated => Self::Stated,
            DecisionBasis::Inferred => Self::Inferred,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AlternativeDto {
    pub option: String,
    pub rejected_because: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DecisionDto {
    pub id: String,
    pub title: String,
    pub basis: BasisDto,
    /// What was chosen.
    pub chosen: String,
    pub why: String,
    pub risk: String,
    pub alternatives: Vec<AlternativeDto>,
    pub files: Vec<String>,
    pub commit_sha: Option<String>,
    /// Cards generated from this decision.
    pub card_ids: Vec<String>,
    /// Already folded into the knowledge graph.
    pub folded: bool,
}

impl DecisionDto {
    pub fn new(row: DecisionRow, links: &CardLinks) -> Self {
        let d = row.record;
        Self {
            card_ids: links.cards_of(&d.id).to_vec(),
            id: d.id.to_string(),
            title: d.title,
            basis: d.basis.into(),
            chosen: d.what,
            why: d.why,
            risk: d.risk,
            alternatives: d
                .alternatives
                .into_iter()
                .map(|a| AlternativeDto {
                    option: a.option,
                    rejected_because: a.rejected_because,
                })
                .collect(),
            files: d.files,
            commit_sha: d.commit_sha,
            folded: row.folded,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DigestSessionDto {
    pub session_id: String,
    /// `claude`, `codex` or `agy`.
    pub source: String,
    pub project: String,
    pub started_at: Option<i64>,
    /// One line: the first decision, plus how many more.
    pub summary: String,
    pub card_count: usize,
    pub decisions: Vec<DecisionDto>,
}

impl DigestSessionDto {
    fn new(s: SessionGroup, links: &CardLinks) -> Self {
        let summary = match s.decisions.as_slice() {
            [] => String::new(),
            [only] => only.record.title.clone(),
            [first, rest @ ..] => format!("{} (+{} more)", first.record.title, rest.len()),
        };
        let decisions: Vec<DecisionDto> = s
            .decisions
            .into_iter()
            .map(|d| DecisionDto::new(d, links))
            .collect();
        Self {
            card_count: decisions.iter().map(|d| d.card_ids.len()).sum(),
            session_id: s.session_id,
            source: s.source,
            project: s.project,
            started_at: s.started_at,
            summary,
            decisions,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DigestDayDto {
    /// `YYYY-MM-DD` (UTC), or `undated`.
    pub day: String,
    pub decision_count: usize,
    pub sessions: Vec<DigestSessionDto>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DigestDto {
    /// Newest day first.
    pub days: Vec<DigestDayDto>,
    pub total_decisions: usize,
    pub total_sessions: usize,
}

impl DigestDto {
    pub fn new(days: Vec<DayGroup>, links: &CardLinks) -> Self {
        let days: Vec<DigestDayDto> = days
            .into_iter()
            .map(|d| {
                let sessions: Vec<DigestSessionDto> = d
                    .sessions
                    .into_iter()
                    .map(|s| DigestSessionDto::new(s, links))
                    .collect();
                DigestDayDto {
                    day: d.day,
                    decision_count: sessions.iter().map(|s| s.decisions.len()).sum(),
                    sessions,
                }
            })
            .collect();
        Self {
            total_decisions: days.iter().map(|d| d.decision_count).sum(),
            total_sessions: days.iter().map(|d| d.sessions.len()).sum(),
            days,
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct DigestQuery {
    /// Sessions started at or after this: epoch ms or `YYYY-MM-DD` (UTC).
    pub from: Option<String>,
    /// Sessions started before this: epoch ms or `YYYY-MM-DD` (UTC, start of that day).
    pub to: Option<String>,
    /// Comma-separated: `claude`, `codex`, `agy`.
    pub source: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CommitDto {
    pub sha: String,
    pub subject: String,
    pub body: String,
    pub files: Vec<String>,
    /// Unified diff, capped at 6,000 characters.
    pub diff: String,
}

impl From<GitCommit> for CommitDto {
    fn from(c: GitCommit) -> Self {
        Self {
            sha: c.sha,
            subject: c.subject,
            body: c.body,
            files: c.files,
            diff: c.diff,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionBriefDto {
    pub id: String,
    pub source: String,
    pub project: String,
    pub cwd: String,
    pub git_branch: Option<String>,
    pub started_at: i64,
}

impl From<&CodingSession> for SessionBriefDto {
    fn from(s: &CodingSession) -> Self {
        Self {
            id: s.id.to_string(),
            source: s.source.to_string(),
            project: std::path::Path::new(&s.cwd)
                .file_name()
                .map_or_else(|| "-".into(), |n| n.to_string_lossy().into_owned()),
            cwd: s.cwd.clone(),
            git_branch: s.git_branch.clone(),
            started_at: s.started_at,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct DecisionDetailDto {
    pub decision: DecisionDto,
    pub session: Option<SessionBriefDto>,
    /// Commits made while the session ran (allow-listed repos only).
    pub commits: Vec<CommitDto>,
    /// Why `commits` could not be read, when it could not.
    pub commits_error: Option<String>,
    pub cards: Vec<CardBriefDto>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RejectedCardDto {
    pub front: String,
    /// Why `validate_card_shape` refused it.
    pub reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegenerateDto {
    pub saved: usize,
    pub card_ids: Vec<String>,
    pub rejected: Vec<RejectedCardDto>,
}

impl From<Regenerated> for RegenerateDto {
    fn from(r: Regenerated) -> Self {
        Self {
            saved: r.saved,
            card_ids: r.card_ids,
            rejected: r
                .rejected
                .into_iter()
                .map(|(front, reason)| RejectedCardDto { front, reason })
                .collect(),
        }
    }
}
