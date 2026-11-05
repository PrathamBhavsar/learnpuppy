//! Course wire types (docs/web/03 §Courses). Questions are built field by field from the
//! service's learner views, so no `answer`, `accept` or `why` can reach a step or a placement item.

use lp_agent::course::Check;
use lp_core::course::{AskKind, Course, ItemKind, LessonKind};
use lp_core::method::register::{RegisterRow, RegisterStatus};
use lp_core::method::{AskKind as MethodAskKind, Step};
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

// ------------------------------------------------------------------ learning method (method_version >= 1)
//
// `method_version == 0` courses (above) are a stored sequence stepped through by `next`/`seen`/`answer`
// with no gate. A `method_version >= 1` course is driven by the single pure pair `next_step`/`check_action`
// (`brain/learning-method/architecture.md` §1): the DTOs below mirror `Step` and its content, never an
// answer, and a rejected action surfaces as `409 conflict` with `details.gate` (`ApiError::gate`).

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MethodAskKindDto {
    Review,
    New,
    Retry,
    Mixed,
}

impl From<MethodAskKind> for MethodAskKindDto {
    fn from(k: MethodAskKind) -> Self {
        match k {
            MethodAskKind::Review => Self::Review,
            MethodAskKind::New => Self::New,
            MethodAskKind::Retry => Self::Retry,
            MethodAskKind::Mixed => Self::Mixed,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LessonKindDto {
    Conceptual,
    Procedural,
    Definitional,
}

impl From<LessonKind> for LessonKindDto {
    fn from(k: LessonKind) -> Self {
        match k {
            LessonKind::Conceptual => Self::Conceptual,
            LessonKind::Procedural => Self::Procedural,
            LessonKind::Definitional => Self::Definitional,
        }
    }
}

/// A lesson as the plan map shows it: structure only, never its content.
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanLessonDto {
    pub id: String,
    pub title: String,
    pub kind: LessonKindDto,
    pub depends_on: Vec<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CoursePlanDto {
    pub confirmed: bool,
    pub lessons: Vec<PlanLessonDto>,
}

impl From<&Course> for CoursePlanDto {
    fn from(c: &Course) -> Self {
        Self {
            confirmed: c.plan_confirmed_at.is_some(),
            lessons: c
                .lessons
                .iter()
                .map(|l| PlanLessonDto {
                    id: l.id.clone(),
                    title: l.title.clone(),
                    kind: l.kind.into(),
                    depends_on: l.depends_on.clone(),
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct InsertPrereqRequest {
    pub before: String,
    pub missing: String,
    #[serde(default)]
    pub visited: Vec<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MethodLessonDto {
    pub id: String,
    pub title: String,
    pub hook: String,
    pub explain: String,
    pub key_facts: Vec<String>,
    /// The faded-example text at the current rung; procedural lessons only.
    pub rung_text: Option<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PredictDto {
    pub lesson: String,
    pub title: String,
    pub q: String,
    pub options: Vec<String>,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MethodItemDto {
    pub id: String,
    pub lesson: String,
    pub kind: ItemKindDto,
    pub q: String,
    pub options: Vec<String>,
    pub ask: MethodAskKindDto,
    /// An alternate explanation to show ahead of this retry, when the lesson has one.
    pub angle: Option<String>,
}

/// What comes next on a `method_version >= 1` course (`lp_core::method::Step`), with its content
/// filled in from the course; never an answer, a `why`, or a rubric.
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(tag = "step", rename_all = "snake_case")]
pub enum MethodStepDto {
    ConfirmPlan,
    Predict(PredictDto),
    Explain(MethodLessonDto),
    Notes {
        lesson: String,
        title: String,
    },
    Ask(MethodItemDto),
    Break {
        lesson: String,
        title: String,
    },
    Feynman {
        lesson: String,
        title: String,
        key_facts: Vec<String>,
    },
    Close,
}

fn lesson_title(course: &Course, id: &str) -> String {
    course
        .lessons
        .iter()
        .find(|l| l.id == id)
        .map(|l| l.title.clone())
        .unwrap_or_default()
}

/// Builds the server's view of `step` from `course`'s content; never an answer.
pub fn method_step_dto(course: &Course, step: &Step) -> MethodStepDto {
    match step {
        Step::ConfirmPlan => MethodStepDto::ConfirmPlan,
        Step::Predict { lesson } => {
            let l = course.lessons.iter().find(|l| &l.id == lesson);
            let title = l.map(|l| l.title.clone()).unwrap_or_default();
            let predict = l.and_then(|l| l.predict.clone()).unwrap_or_default();
            MethodStepDto::Predict(PredictDto {
                lesson: lesson.clone(),
                title,
                q: predict.q,
                options: predict.options,
            })
        }
        Step::Explain { lesson, rung } => {
            let l = course
                .lessons
                .iter()
                .find(|l| &l.id == lesson)
                .cloned()
                .unwrap_or_default();
            let rung_text = rung.and_then(|i| l.rungs.get(i)).map(|r| r.text.clone());
            MethodStepDto::Explain(MethodLessonDto {
                id: l.id,
                title: l.title,
                hook: l.hook,
                explain: l.explain,
                key_facts: l.key_facts,
                rung_text,
            })
        }
        Step::Notes { lesson } => MethodStepDto::Notes {
            lesson: lesson.clone(),
            title: lesson_title(course, lesson),
        },
        Step::Ask {
            item,
            lesson,
            kind,
            angle,
        } => {
            let found_lesson = course.lessons.iter().find(|l| &l.id == lesson);
            let it = found_lesson
                .and_then(|l| l.items.iter().find(|i| &i.id == item))
                .cloned()
                .unwrap_or_default();
            let angle_text =
                angle.and_then(|i| found_lesson.and_then(|l| l.angles.get(i)).cloned());
            MethodStepDto::Ask(MethodItemDto {
                id: it.id,
                lesson: lesson.clone(),
                kind: it.kind.into(),
                q: it.q,
                options: it.options,
                ask: (*kind).into(),
                angle: angle_text,
            })
        }
        Step::Break { lesson } => MethodStepDto::Break {
            lesson: lesson.clone(),
            title: lesson_title(course, lesson),
        },
        Step::Feynman { lesson } => {
            let l = course.lessons.iter().find(|l| &l.id == lesson);
            MethodStepDto::Feynman {
                lesson: lesson.clone(),
                title: l.map(|l| l.title.clone()).unwrap_or_default(),
                key_facts: l.map(|l| l.key_facts.clone()).unwrap_or_default(),
            }
        }
        Step::Close => MethodStepDto::Close,
    }
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct PredictRequest {
    pub lesson: String,
    #[serde(default)]
    pub guess: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct ExplainSeenRequest {
    pub lesson: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct NoteRequest {
    pub lesson: String,
    #[serde(default)]
    pub text: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct MethodAnswerRequest {
    pub item: String,
    pub lesson: String,
    #[serde(default)]
    pub text: String,
    pub confidence: Option<u8>,
    pub ms: Option<u64>,
    #[serde(default)]
    pub reasoning: String,
    /// "I don't know": a gap, never a miss (F12).
    #[serde(default)]
    pub gap: bool,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum MethodAnswerResultDto {
    /// Graded on the spot: multiple choice, an exact recall match, or a gap.
    Graded { correct: bool },
    /// A recall miss the rubric did not match: resolved later by a grader run (F15).
    Pending,
}

impl From<lp_service::course::plan::AnswerOutcome> for MethodAnswerResultDto {
    fn from(o: lp_service::course::plan::AnswerOutcome) -> Self {
        match o {
            lp_service::course::plan::AnswerOutcome::Graded { correct } => Self::Graded { correct },
            lp_service::course::plan::AnswerOutcome::Pending => Self::Pending,
        }
    }
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct FeynmanRequest {
    pub lesson: String,
    pub explanation: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct FeynmanResultDto {
    pub correct: bool,
    pub note: String,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct BreakRequest {
    pub lesson: String,
}

/// F30: the Close screen's numbers.
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CloseReportDto {
    pub takeaways: Vec<String>,
    pub focus: Vec<String>,
    pub count_line: String,
    pub sure_wrong: u32,
    pub sure_total: u32,
    /// `None` once the course is fully done.
    pub resume: Option<String>,
}

impl From<svc::close::CloseReport> for CloseReportDto {
    fn from(r: svc::close::CloseReport) -> Self {
        Self {
            takeaways: r.takeaways,
            focus: r.focus,
            count_line: r.count_line,
            sure_wrong: r.sure_and_wrong.0,
            sure_total: r.sure_and_wrong.1,
            resume: r.resume,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "kebab-case")]
pub enum RegisterStatusDto {
    Open,
    RetestedOk,
    Recurring,
}

impl From<RegisterStatus> for RegisterStatusDto {
    fn from(s: RegisterStatus) -> Self {
        match s {
            RegisterStatus::Open => Self::Open,
            RegisterStatus::RetestedOk => Self::RetestedOk,
            RegisterStatus::Recurring => Self::Recurring,
        }
    }
}

/// F19: one misconception row, folded live from the course's events (no stored ledger yet).
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct RegisterRowDto {
    pub item: String,
    pub belief: String,
    pub status: RegisterStatusDto,
    pub misses: u32,
}

impl From<RegisterRow> for RegisterRowDto {
    fn from(r: RegisterRow) -> Self {
        Self {
            item: r.item,
            belief: r.belief,
            status: r.status.into(),
            misses: r.misses,
        }
    }
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct RegisterListDto {
    pub rows: Vec<RegisterRowDto>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct BrainDumpRequest {
    pub text: String,
}

/// F29: the key facts a brain dump left out.
#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct BrainDumpResultDto {
    pub missed: Vec<String>,
}

/// F32: one proposed first test, labelled by the fact it covers.
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDateDto {
    pub card_id: String,
    pub label: String,
    /// Epoch ms.
    pub due: i64,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ReviewDateListDto {
    pub dates: Vec<ReviewDateDto>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewDateEntry {
    pub card_id: String,
    /// Epoch ms.
    pub due: i64,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
pub struct ReviewDatesRequest {
    pub dates: Vec<ReviewDateEntry>,
}

/// The mastery bar's count line, e.g. "11 of 14 confirmed, 3 open, 1 parked".
#[derive(Clone, Debug, Serialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct MasteryCountDto {
    pub count_line: String,
}
