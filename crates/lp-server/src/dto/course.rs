//! Course wire types (docs/web/03 §Courses). Questions are built field by field from the
//! service's learner views, so no `answer`, `accept` or `why` can reach a step or a placement item.

use lp_agent::course::Check;
use lp_core::course::{AskKind, ItemKind};
use lp_service::course as svc;
use lp_store::CourseSummary;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CourseSummaryDto {
    pub id: String,
    pub title: String,
    pub version: u32,
    /// Epoch ms.
    pub updated_at: i64,
}

impl From<CourseSummary> for CourseSummaryDto {
    fn from(c: CourseSummary) -> Self {
        Self {
            id: c.id,
            title: c.title,
            version: c.version,
            updated_at: c.updated_at,
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct CourseListDto {
    pub courses: Vec<CourseSummaryDto>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct CreateCourseRequest {
    pub goal: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ItemKindDto {
    Mcq,
    Recall,
}

impl From<ItemKind> for ItemKindDto {
    fn from(k: ItemKind) -> Self {
        match k {
            ItemKind::Mcq => Self::Mcq,
            ItemKind::Recall => Self::Recall,
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct PlacementItemDto {
    pub id: String,
    pub kind: ItemKindDto,
    pub q: String,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct CreatedCourseDto {
    pub id: String,
    pub items: Vec<PlacementItemDto>,
}

impl From<svc::PlacementItem> for PlacementItemDto {
    fn from(i: svc::PlacementItem) -> Self {
        Self {
            id: i.id,
            kind: i.kind.into(),
            q: i.q,
            options: i.options,
        }
    }
}

impl From<svc::Created> for CreatedCourseDto {
    fn from(c: svc::Created) -> Self {
        Self {
            id: c.id,
            items: c.items.into_iter().map(Into::into).collect(),
        }
    }
}

/// `GET …/placement`: the questions still to answer, empty once submitted.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct PlacementListDto {
    pub items: Vec<PlacementItemDto>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct PlacementAnswerDto {
    pub item: String,
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct SubmitPlacementRequest {
    pub answers: Vec<PlacementAnswerDto>,
}

/// A graded placement answer. The answer is revealed because the learner has now answered.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct GradedDto {
    pub item: String,
    pub correct: bool,
    pub answer: String,
    pub why: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct GradedListDto {
    pub graded: Vec<GradedDto>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MissDto {
    pub q: String,
    pub answer: String,
    pub why: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct MissListDto {
    pub misses: Vec<MissDto>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CourseState {
    Writing,
    Ready,
    Failed,
    Refilling,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct CourseStatusDto {
    pub state: CourseState,
    /// Why the course failed (`failed` only).
    pub reason: Option<String>,
}

impl From<svc::CourseStatus> for CourseStatusDto {
    fn from(s: svc::CourseStatus) -> Self {
        let (state, reason) = match s {
            svc::CourseStatus::Writing => (CourseState::Writing, None),
            svc::CourseStatus::Ready => (CourseState::Ready, None),
            svc::CourseStatus::Failed { reason } => (CourseState::Failed, Some(reason)),
            svc::CourseStatus::Refilling => (CourseState::Refilling, None),
        };
        Self { state, reason }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct LessonDto {
    pub id: String,
    pub title: String,
    pub objective: String,
    pub explain: String,
    pub key_facts: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AskKindDto {
    New,
    Retry,
    Interleave,
    Review,
}

impl From<AskKind> for AskKindDto {
    fn from(k: AskKind) -> Self {
        match k {
            AskKind::New => Self::New,
            AskKind::Retry => Self::Retry,
            AskKind::Interleave => Self::Interleave,
            AskKind::Review => Self::Review,
        }
    }
}

/// A question: never the answer, the accepted forms or the explanation.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ItemDto {
    pub id: String,
    pub lesson: String,
    pub kind: ItemKindDto,
    pub q: String,
    pub options: Vec<String>,
    pub ask: AskKindDto,
}

/// The next thing to show.
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum StepDto {
    Explain {
        lesson: LessonDto,
    },
    Ask {
        item: ItemDto,
    },
    /// Nothing stored is left; poll `status` and ask again.
    Waiting,
    Done,
}

impl From<svc::StepView> for StepDto {
    fn from(s: svc::StepView) -> Self {
        match s {
            svc::StepView::Explain(l) => Self::Explain {
                lesson: LessonDto {
                    id: l.id,
                    title: l.title,
                    objective: l.objective,
                    explain: l.explain,
                    key_facts: l.key_facts,
                },
            },
            svc::StepView::Ask(i) => Self::Ask {
                item: ItemDto {
                    id: i.id,
                    lesson: i.lesson,
                    kind: i.kind.into(),
                    q: i.q,
                    options: i.options,
                    ask: i.ask.into(),
                },
            },
            svc::StepView::Waiting => Self::Waiting,
            svc::StepView::Done => Self::Done,
        }
    }
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct SeenRequest {
    pub lesson: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct CourseAnswerRequest {
    pub item: String,
    pub text: String,
    /// 1..=5.
    pub confidence: Option<u8>,
    /// How long the learner took, measured by the client.
    pub ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct FeedbackDto {
    pub correct: bool,
    /// Near the right answer: the learner confirms with a self-grade.
    pub close: bool,
    pub answer: String,
    pub why: String,
}

impl From<svc::Feedback> for FeedbackDto {
    fn from(f: svc::Feedback) -> Self {
        Self {
            correct: f.correct,
            close: f.close,
            answer: f.answer,
            why: f.why,
        }
    }
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct SelfGradeRequest {
    pub item: String,
    /// `again`, `hard`, `good` or `easy`.
    pub grade: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct FlagRequest {
    pub item: String,
    pub reason: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct AskQuestionRequest {
    pub question: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct AskReplyDto {
    pub reply: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct CheckRequest {
    pub item: String,
    pub text: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct CheckDto {
    pub correct: bool,
    pub note: String,
}

impl From<Check> for CheckDto {
    fn from(c: Check) -> Self {
        Self {
            correct: c.correct,
            note: c.note,
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct UsageDto {
    pub strong_in: u64,
    pub strong_out: u64,
    pub cheap_in: u64,
    pub cheap_out: u64,
    pub strong_calls_this_hour: u32,
    /// While a refill waits for a strong slot: when one frees up (epoch ms).
    pub refill_waiting_until: Option<i64>,
}

impl From<svc::Usage> for UsageDto {
    fn from(u: svc::Usage) -> Self {
        Self {
            strong_in: u.strong_in,
            strong_out: u.strong_out,
            cheap_in: u.cheap_in,
            cheap_out: u.cheap_out,
            strong_calls_this_hour: u.strong_calls_this_hour,
            refill_waiting_until: u.refill_waiting_until,
        }
    }
}
