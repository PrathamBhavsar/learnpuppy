//! Learning-method routes (docs/web/03 §Learning method): the HTTP surface for a
//! `method_version >= 1` course's plan, its single `next_step`/`check_action` gate, Feynman,
//! close, the brain dump, review dates, the register and the mastery bar.
//!
//! Stateless: every call rebuilds its answer from the stored course, its event log and (for due
//! reviews) the FSRS tracker — there is no per-session state to restore after a restart. A
//! rejected action is `409 conflict` with `details.gate` naming the violated rule.

use std::sync::Arc;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use lp_core::course::CourseEvent;
use lp_core::method::bout::BoutStatus;
use lp_core::method::dag::PrereqError;
use lp_core::method::{pacing, Action, GateError, Settings as MethodSettings};
use lp_service::course::plan::{self, PlanError};
use lp_service::course::{close, grading};
use lp_service::learning::{Clock as SvcClock, LearningError, LearningService, Refusal};
use lp_service::settings::Settings;
use lp_service::workspace::WorkspaceHandle;

use super::common::{blocking, open_writable, open_ws};
use crate::app::AppState;
use crate::dto::course::{
    BrainDumpRequest, BrainDumpResultDto, BreakRequest, CloseReportDto, CoursePlanDto,
    ExplainSeenRequest, FeynmanRequest, FeynmanResultDto, InsertPrereqRequest, MasteryCountDto,
    MethodAnswerRequest, MethodAnswerResultDto, MethodStepDto, NoteRequest, PredictRequest,
    RegisterListDto, ReviewDateDto, ReviewDateListDto, ReviewDatesRequest,
};
use crate::dto::learning::{
    clock_dto, method_settings_dto, CheckinNeededDto, CheckinRequest, CheckinResultDto, ClockDto,
    HeartbeatRequest, MethodSettingsDto, OverrideRequest, ProfileResultDto, ScheduleDto,
    SetMethodSettingRequest, SetProfileRequest, SetScheduleRequest, StartBoutRequest,
    SuggestSlotsRequest, SuggestSlotsResultDto, TrackSlotsResultDto,
};
use crate::error::{ApiError, ApiJson, ErrorCode, ErrorEnvelope};

const MAX_ID: usize = 128;
const MAX_TEXT: usize = 5_000;
const MAX_REASON: usize = 2_000;
/// `track_slots`' window; the UI sends no day count, so this mirrors the sleep summary's own
/// 14-day window for consistency.
const TRACK_DAYS: u32 = 14;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/workspaces/{ws}/courses/{id}/plan", get(plan_get))
        .route(
            "/workspaces/{ws}/courses/{id}/plan/confirm",
            post(plan_confirm),
        )
        .route(
            "/workspaces/{ws}/courses/{id}/plan/prereq",
            post(plan_prereq),
        )
        .route("/workspaces/{ws}/courses/{id}/step", post(step))
        .route("/workspaces/{ws}/courses/{id}/predict", post(predict))
        .route(
            "/workspaces/{ws}/courses/{id}/explain-seen",
            post(explain_seen),
        )
        .route("/workspaces/{ws}/courses/{id}/note", post(note))
        .route(
            "/workspaces/{ws}/courses/{id}/quiz-answer",
            post(quiz_answer),
        )
        .route("/workspaces/{ws}/courses/{id}/feynman", post(feynman))
        .route("/workspaces/{ws}/courses/{id}/break", post(take_break))
        .route("/workspaces/{ws}/courses/{id}/close", post(close_session))
        .route("/workspaces/{ws}/courses/{id}/braindump", post(braindump))
        .route(
            "/workspaces/{ws}/courses/{id}/review-dates",
            get(review_dates).post(save_review_dates),
        )
        .route("/workspaces/{ws}/courses/{id}/register", get(register))
        .route("/workspaces/{ws}/courses/{id}/mastery", get(mastery))
        .route("/workspaces/{ws}/learning/clock", get(clock))
        .route("/workspaces/{ws}/learning/bout/start", post(bout_start))
        .route(
            "/workspaces/{ws}/learning/bout/heartbeat",
            post(bout_heartbeat),
        )
        .route("/workspaces/{ws}/learning/bout/end", post(bout_end))
        .route(
            "/workspaces/{ws}/learning/bout/focus-lost",
            post(bout_focus_lost),
        )
        .route(
            "/workspaces/{ws}/learning/bout/override",
            post(bout_override),
        )
        .route(
            "/workspaces/{ws}/learning/checkin/needed",
            get(checkin_needed),
        )
        .route("/workspaces/{ws}/learning/checkin", post(checkin))
        .route(
            "/workspaces/{ws}/learning/settings",
            get(method_settings_get).post(method_settings_set),
        )
        .route(
            "/workspaces/{ws}/learning/schedule",
            get(schedule_get).post(schedule_set),
        )
        .route(
            "/workspaces/{ws}/learning/schedule/suggest",
            post(schedule_suggest),
        )
        .route(
            "/workspaces/{ws}/learning/schedule/track",
            get(schedule_track),
        )
        .route(
            "/workspaces/{ws}/learning/profile",
            get(profile_get).post(profile_set),
        )
}

/// Mirrors the gate code used in each `GateError`/`PrereqError`, the stable, machine-readable
/// value the UI switches on (docs/web/03 §Conventions).
fn gate_code(e: &GateError) -> &'static str {
    match e {
        GateError::MasteryBarOpen { .. } => "mastery_bar_open",
        GateError::PlanNotConfirmed => "plan_not_confirmed",
        GateError::PlanHasCycle => "plan_has_cycle",
        GateError::PrereqCycle => "prereq_cycle",
        GateError::PrereqReordersVisited => "prereq_reorders_visited",
        GateError::OutOfOrder { .. } => "out_of_order",
    }
}

impl From<GateError> for ApiError {
    fn from(e: GateError) -> Self {
        let code = gate_code(&e);
        ApiError::gate(code, e.to_string())
    }
}

impl From<PrereqError> for ApiError {
    fn from(e: PrereqError) -> Self {
        let code = match e {
            PrereqError::Cycle => "prereq_cycle",
            PrereqError::ReordersVisited => "prereq_reorders_visited",
        };
        ApiError::gate(code, e.to_string())
    }
}

impl From<PlanError> for ApiError {
    fn from(e: PlanError) -> Self {
        match e {
            PlanError::Gate(g) => g.into(),
            PlanError::Prereq(p) => p.into(),
            PlanError::Store(lp_core::ports::StoreError::NotFound(m)) => ApiError::not_found(m),
            PlanError::Store(e) => ApiError::from(anyhow::anyhow!(e.to_string())),
            PlanError::Agent(e) => ApiError::new(ErrorCode::EngineUnavailable, e.to_string()),
            PlanError::NotFound(m) => ApiError::not_found(m),
            PlanError::NeedsDedicatedCall(a) => {
                ApiError::from(anyhow::anyhow!("{a:?} needs its own route"))
            }
        }
    }
}

/// The stable, machine-readable code for a refused bout start (docs/web/03 §Conventions).
fn refusal_gate(r: Refusal) -> &'static str {
    match r {
        Refusal::Gap { .. } => "gap",
        Refusal::DailyLimit => "daily_limit",
        Refusal::NotReady => "not_ready",
    }
}

/// The refusal for a step asked of a locked clock, with the same stable codes a bout start uses.
fn locked_gate(status: BoutStatus) -> ApiError {
    match status {
        BoutStatus::Gap { remaining_minutes } => ApiError::new(
            ErrorCode::Conflict,
            format!("in the gap between bouts: {remaining_minutes} min left"),
        )
        .with_details(serde_json::json!({ "gate": "gap", "remainingMinutes": remaining_minutes })),
        BoutStatus::DailyLimitReached => {
            ApiError::gate("daily_limit", "today's study time is used up")
        }
        BoutStatus::BoutLimitReached => ApiError::gate("bout_limit", "this bout hit its limit"),
        BoutStatus::Active { .. } => ApiError::gate("bout_required", "no open bout"),
    }
}

impl From<LearningError> for ApiError {
    fn from(e: LearningError) -> Self {
        match e {
            LearningError::Refused(r) => {
                let details = match r {
                    Refusal::Gap { remaining_minutes } => {
                        serde_json::json!({ "gate": refusal_gate(r), "remainingMinutes": remaining_minutes })
                    }
                    _ => serde_json::json!({ "gate": refusal_gate(r) }),
                };
                ApiError::new(ErrorCode::Conflict, e.to_string()).with_details(details)
            }
            LearningError::Invalid(m) => ApiError::invalid("", m),
            LearningError::NotFound(m) => ApiError::not_found(m),
            LearningError::OverrideUsed => ApiError::gate("override_used", e.to_string()),
            LearningError::Store(m) => ApiError::from(anyhow::anyhow!(m)),
        }
    }
}

/// A course id is `[a-z0-9_-]` up to 128 characters; anything else cannot name a course, so it is
/// a plain 404 (same rule as `routes::courses::course_id`).
fn course_id(id: &str) -> Result<(), ApiError> {
    let ok = !id.is_empty()
        && id.len() <= MAX_ID
        && id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'-');
    if ok {
        Ok(())
    } else {
        Err(ApiError::not_found(format!("no course {id}")))
    }
}

fn text(field: &str, v: &str, min: usize, max: usize) -> Result<(), ApiError> {
    let n = v.trim().chars().count();
    if n < min || v.chars().count() > max {
        return Err(ApiError::invalid(
            field,
            format!("must be {min} to {max} characters"),
        ));
    }
    Ok(())
}

fn lesson_id(v: &str) -> Result<(), ApiError> {
    text("lesson", v, 1, MAX_ID)
}

fn item_id(v: &str) -> Result<(), ApiError> {
    text("item", v, 1, MAX_ID)
}

fn bout_id(v: &str) -> Result<(), ApiError> {
    text("bout", v, 1, MAX_ID)
}

fn learning(h: &WorkspaceHandle) -> LearningService {
    LearningService::open(h.root())
}

/// The workspace's tuned method settings (`attempts_before_parking`, bout limits, ...), never the
/// built-in defaults: a setting changed in the UI has to change the teach loop.
fn ws_settings(h: &WorkspaceHandle) -> Result<MethodSettings, LearningError> {
    learning(h).settings()
}

/// F24: whether a gap pause is due right now, seeded from the open bout's own id (its start
/// timestamp) so the schedule is reproducible across polls without any extra stored state; it
/// clears on its own once a heartbeat moves the active-minute count past the minute it was due
/// on. Only meaningful while a bout is open and actively running.
fn gap_pause_due_for(clock: &SvcClock, settings: &MethodSettings) -> bool {
    let Some(bout) = &clock.bout else {
        return false;
    };
    let BoutStatus::Active { active_minutes } = clock.status else {
        return false;
    };
    let seed = bout
        .strip_prefix("b_")
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(0);
    pacing::gap_pause_due(seed, active_minutes, settings.gap_pause_every_minutes)
}

fn to_clock_dto(clock: SvcClock, settings: &MethodSettings) -> ClockDto {
    let gap_pause_due = gap_pause_due_for(&clock, settings);
    clock_dto(&clock, settings, gap_pause_due)
}

async fn open(st: &AppState, ws: &str, write: bool) -> Result<Arc<WorkspaceHandle>, ApiError> {
    if write {
        open_writable(st, ws).await
    } else {
        open_ws(st, ws).await
    }
}

/// The plan map. A `method_version == 0` course (the stored-sequence path) has none: `404`.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/courses/{id}/plan",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    responses((status = 200, body = CoursePlanDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn plan_get(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<CoursePlanDto>, ApiError> {
    course_id(&id)?;
    let h = open(&st, &ws, false).await?;
    let course = blocking(move || Ok(h.stores().courses.load_course(&id)?)).await?;
    if course.method_version == 0 {
        return Err(ApiError::not_found("this course has no plan"));
    }
    Ok(Json(CoursePlanDto::from(&course)))
}

/// F07: confirms the plan map. `409 plan_has_cycle` if the plan (still) contains one.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/plan/confirm",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    responses(
        (status = 204),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Gate rejection", body = ErrorEnvelope),
    )
)]
pub async fn plan_confirm(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<StatusCode, ApiError> {
    course_id(&id)?;
    let h = open(&st, &ws, true).await?;
    let now = lp_service::now_ms();
    blocking(move || Ok(plan::confirm_plan(&h.stores().courses, &id, now)?)).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// F20: writes one short prerequisite lesson (one cheap model call) and splices it into the
/// frozen plan ahead of `before`. `409 prereq_cycle` / `409 prereq_reorders_visited` if it would.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/plan/prereq",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = InsertPrereqRequest,
    responses(
        (status = 200, body = CoursePlanDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Gate rejection", body = ErrorEnvelope),
        (status = 503, description = "No AI available", body = ErrorEnvelope),
    )
)]
pub async fn plan_prereq(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<InsertPrereqRequest>,
) -> Result<Json<CoursePlanDto>, ApiError> {
    course_id(&id)?;
    lesson_id(&req.before)?;
    text("missing", &req.missing, 1, MAX_TEXT)?;
    let h = open(&st, &ws, true).await?;

    let (hb, idb, beforeb) = (h.clone(), id.clone(), req.before.clone());
    let target =
        blocking(move || Ok(plan::prereq_target(&hb.stores().courses, &idb, &beforeb)?)).await?;

    let settings = Settings::load(&st.config.data_root).unwrap_or_default();
    let engine = st.engine_for(&settings, "follow_up");
    let prereq = lp_agent::course::prereq(&engine, &target, &req.missing)
        .await
        .map_err(|e| ApiError::new(ErrorCode::EngineUnavailable, e.to_string()))?;

    let now = lp_service::now_ms();
    let (hc, idc, beforec, visitedc) = (h.clone(), id.clone(), req.before.clone(), req.visited);
    blocking(move || {
        Ok(plan::commit_prereq(
            &hc.stores().courses,
            &idc,
            &ws_settings(&hc)?,
            prereq,
            &beforec,
            &visitedc,
            now,
        )?)
    })
    .await?;

    let course = blocking(move || Ok(h.stores().courses.load_course(&id)?)).await?;
    Ok(Json(CoursePlanDto::from(&course)))
}

/// What's next, due reviews served first (F05). Read-only: `Shown`, `Predicted` and the rest are
/// logged only when the learner acts on the step through its own endpoint, never here.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/step",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    responses((status = 200, body = MethodStepDto), (status = 404, body = ErrorEnvelope), (status = 409, description = "No open bout, or the bout clock is locked", body = ErrorEnvelope))
)]
pub async fn step(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<MethodStepDto>, ApiError> {
    course_id(&id)?;
    let h = open(&st, &ws, false).await?;
    let now = lp_service::now_ms();
    let (course, st_) = blocking(move || {
        // F23: teaching needs an open bout, and a locked clock hands out nothing but Close.
        let clock = learning(&h).clock(now)?;
        if clock.bout.is_none() && !matches!(clock.status, BoutStatus::Active { .. }) {
            return Err(locked_gate(clock.status));
        }
        if clock.bout.is_none() {
            return Err(ApiError::gate(
                "bout_required",
                "start a bout (the ready gate) before studying",
            ));
        }
        let (course, st_) = plan::step(
            &h.stores().courses,
            &*h.stores().tracker,
            &id,
            &ws_settings(&h)?,
            now,
        )?;
        match clock.status {
            BoutStatus::Active { .. } => Ok((course, st_)),
            // An open bout that ran out of time, or spent the day, may only be closed.
            BoutStatus::BoutLimitReached | BoutStatus::DailyLimitReached => {
                Ok((course, lp_core::method::Step::Close))
            }
            other => Err(locked_gate(other)),
        }
    })
    .await?;
    Ok(Json(crate::dto::course::method_step_dto(&course, &st_)))
}

/// F09: an ungraded prediction before a conceptual explanation.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/predict",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = PredictRequest,
    responses(
        (status = 204),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Gate rejection", body = ErrorEnvelope),
    )
)]
pub async fn predict(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<PredictRequest>,
) -> Result<StatusCode, ApiError> {
    course_id(&id)?;
    lesson_id(&req.lesson)?;
    text("guess", &req.guess, 0, MAX_TEXT)?;
    let h = open(&st, &ws, true).await?;
    let now = lp_service::now_ms();
    blocking(move || {
        let settings = ws_settings(&h)?;
        let due = plan::due_items(&*h.stores().tracker, &id, now)?;
        let action = Action::Predict {
            lesson: req.lesson.clone(),
        };
        let event = CourseEvent::Predicted {
            ts: now,
            lesson: req.lesson.clone(),
            guess: req.guess.clone(),
        };
        Ok(plan::apply(
            &h.stores().courses,
            &id,
            &settings,
            &due,
            true,
            &action,
            &event,
        )?)
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// F10/F16: marks a lesson's explanation (or one rung of a procedure) as read.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/explain-seen",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = ExplainSeenRequest,
    responses(
        (status = 204),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Gate rejection", body = ErrorEnvelope),
    )
)]
pub async fn explain_seen(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<ExplainSeenRequest>,
) -> Result<StatusCode, ApiError> {
    course_id(&id)?;
    lesson_id(&req.lesson)?;
    let h = open(&st, &ws, true).await?;
    let now = lp_service::now_ms();
    blocking(move || {
        let settings = ws_settings(&h)?;
        let due = plan::due_items(&*h.stores().tracker, &id, now)?;
        let action = Action::Explain {
            lesson: req.lesson.clone(),
        };
        let event = CourseEvent::Shown {
            ts: now,
            lesson: req.lesson.clone(),
        };
        Ok(plan::apply(
            &h.stores().courses,
            &id,
            &settings,
            &due,
            true,
            &action,
            &event,
        )?)
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// F11: the notes gate between explanation and quiz.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/note",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = NoteRequest,
    responses(
        (status = 204),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Gate rejection", body = ErrorEnvelope),
    )
)]
pub async fn note(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<NoteRequest>,
) -> Result<StatusCode, ApiError> {
    course_id(&id)?;
    lesson_id(&req.lesson)?;
    text("text", &req.text, 0, MAX_TEXT)?;
    let h = open(&st, &ws, true).await?;
    let now = lp_service::now_ms();
    blocking(move || {
        let settings = ws_settings(&h)?;
        let due = plan::due_items(&*h.stores().tracker, &id, now)?;
        let action = Action::WriteNote {
            lesson: req.lesson.clone(),
        };
        let event = CourseEvent::NoteWritten {
            ts: now,
            lesson: req.lesson.clone(),
            text: req.text.clone(),
        };
        Ok(plan::apply(
            &h.stores().courses,
            &id,
            &settings,
            &due,
            true,
            &action,
            &event,
        )?)
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// F12/F13/F15/F19/F21/F27: grades a quiz answer with code, never a model.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/quiz-answer",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = MethodAnswerRequest,
    responses(
        (status = 200, body = MethodAnswerResultDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Gate rejection", body = ErrorEnvelope),
    )
)]
pub async fn quiz_answer(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<MethodAnswerRequest>,
) -> Result<Json<MethodAnswerResultDto>, ApiError> {
    course_id(&id)?;
    item_id(&req.item)?;
    lesson_id(&req.lesson)?;
    text("text", &req.text, 0, MAX_TEXT)?;
    text("reasoning", &req.reasoning, 0, MAX_REASON)?;
    if req.confidence.is_some_and(|c| !(1..=5).contains(&c)) {
        return Err(ApiError::invalid("confidence", "confidence is 1 to 5"));
    }
    let h = open(&st, &ws, true).await?;
    let now = lp_service::now_ms();
    let device = st.device_id.to_string();
    let input = plan::AnswerInput {
        item: req.item,
        lesson: req.lesson,
        text: req.text,
        confidence: req.confidence,
        ms: req.ms,
        reasoning: req.reasoning,
        gap: req.gap,
    };
    let out = blocking(move || {
        let settings = ws_settings(&h)?;
        Ok(plan::quiz_answer(
            &h.stores().courses,
            &*h.stores().tracker,
            &device,
            &id,
            &settings,
            &input,
            now,
        )?)
    })
    .await?;
    Ok(Json(out.into()))
}

/// F18: an explain-it-to-a-ten-year-old pass. The model's verdict never blocks the gate.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/feynman",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = FeynmanRequest,
    responses(
        (status = 200, body = FeynmanResultDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Gate rejection", body = ErrorEnvelope),
    )
)]
pub async fn feynman(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<FeynmanRequest>,
) -> Result<Json<FeynmanResultDto>, ApiError> {
    course_id(&id)?;
    lesson_id(&req.lesson)?;
    text("explanation", &req.explanation, 1, MAX_TEXT)?;
    let h = open(&st, &ws, true).await?;

    let (hb, idb, lessonb) = (h.clone(), id.clone(), req.lesson.clone());
    let lesson = blocking(move || Ok(plan::lesson(&hb.stores().courses, &idb, &lessonb)?)).await?;

    let settings = Settings::load(&st.config.data_root).unwrap_or_default();
    let engine = st.engine_for(&settings, "follow_up");
    let check = grading::grade_feynman(Some(&engine), &lesson, &req.explanation).await;

    let now = lp_service::now_ms();
    let (idc, lessonc, explanationc, correct) = (
        id.clone(),
        req.lesson.clone(),
        req.explanation.clone(),
        check.correct,
    );
    blocking(move || {
        let method_settings = ws_settings(&h)?;
        let due = plan::due_items(&*h.stores().tracker, &idc, now)?;
        Ok(plan::commit_feynman(
            &h.stores().courses,
            &idc,
            &method_settings,
            &due,
            &lessonc,
            correct,
            explanationc,
            now,
        )?)
    })
    .await?;

    Ok(Json(FeynmanResultDto {
        correct: check.correct,
        note: check.note,
    }))
}

/// F21: offered once per lesson, after two straight misses, instead of a third angle.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/break",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = BreakRequest,
    responses(
        (status = 204),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Gate rejection", body = ErrorEnvelope),
    )
)]
pub async fn take_break(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<BreakRequest>,
) -> Result<StatusCode, ApiError> {
    course_id(&id)?;
    lesson_id(&req.lesson)?;
    let h = open(&st, &ws, true).await?;
    let now = lp_service::now_ms();
    blocking(move || {
        let settings = ws_settings(&h)?;
        let due = plan::due_items(&*h.stores().tracker, &id, now)?;
        let action = Action::TakeBreak {
            lesson: req.lesson.clone(),
        };
        let event = CourseEvent::BreakOffered {
            ts: now,
            lesson: req.lesson.clone(),
        };
        Ok(plan::apply(
            &h.stores().courses,
            &id,
            &settings,
            &due,
            true,
            &action,
            &event,
        )?)
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// F29-F31/F32/F35: ends the session — makes an FSRS card for every fact confirmed just now,
/// writes the confirmed-fact nodes to the Knowledge Graph, and appends the session log.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/close",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    responses((status = 200, body = CloseReportDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn close_session(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<CloseReportDto>, ApiError> {
    course_id(&id)?;
    let h = open(&st, &ws, true).await?;
    let now = lp_service::now_ms();
    let date = lp_service::digest::day_of(now);
    let report = blocking(move || {
        let stores = h.stores();
        Ok(close::run_close(
            &stores.courses,
            &*stores.graph,
            &*stores.tracker,
            &id,
            &ws_settings(&h)?,
            &date,
            now,
        )?)
    })
    .await?;
    Ok(Json(report.into()))
}

/// F29: the key facts a brain dump left out. Never touches the store.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/braindump",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = BrainDumpRequest,
    responses(
        (status = 200, body = BrainDumpResultDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
    )
)]
pub async fn braindump(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<BrainDumpRequest>,
) -> Result<Json<BrainDumpResultDto>, ApiError> {
    course_id(&id)?;
    text("text", &req.text, 0, MAX_TEXT)?;
    let h = open(&st, &ws, false).await?;
    let missed = blocking(move || {
        let course = h.stores().courses.load_course(&id)?;
        Ok(close::missed_facts(&course, &req.text))
    })
    .await?;
    Ok(Json(BrainDumpResultDto { missed }))
}

/// F32: the batched first-test proposal for facts confirmed at close.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/courses/{id}/review-dates",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    responses((status = 200, body = ReviewDateListDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn review_dates(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<ReviewDateListDto>, ApiError> {
    course_id(&id)?;
    let h = open(&st, &ws, false).await?;
    let now = lp_service::now_ms();
    let dates: Vec<ReviewDateDto> = blocking(move || {
        let course = h.stores().courses.load_course(&id)?;
        let events = h.stores().courses.events(&id)?;
        let dates = close::propose_review_dates(&course, &events, now)
            .into_iter()
            .map(|d| {
                let label = lp_core::course::find_item(&course, &d.item)
                    .and_then(|(l, it)| it.tests.map(|f| (l, f)))
                    .and_then(|(l, f)| l.key_facts.get(f).cloned())
                    .unwrap_or_default();
                ReviewDateDto {
                    card_id: d.card_id,
                    label,
                    due: d.due,
                }
            })
            .collect();
        Ok(dates)
    })
    .await?;
    Ok(Json(ReviewDateListDto { dates }))
}

/// F32: saves the learner's chosen first-test dates (never more than a day out).
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/courses/{id}/review-dates",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    request_body = ReviewDatesRequest,
    responses(
        (status = 204),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
    )
)]
pub async fn save_review_dates(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
    ApiJson(req): ApiJson<ReviewDatesRequest>,
) -> Result<StatusCode, ApiError> {
    course_id(&id)?;
    // A card id is `c:{course}:{item}`; this keeps a date proposal from this course's own
    // `review-dates` screen from accidentally rescheduling another course's cards.
    let prefix = format!("c:{id}:");
    if req.dates.iter().any(|d| !d.card_id.starts_with(&prefix)) {
        return Err(ApiError::invalid(
            "dates",
            "cardId does not belong to this course",
        ));
    }
    let h = open(&st, &ws, true).await?;
    let now = lp_service::now_ms();
    blocking(move || {
        let dates: Vec<(String, i64)> = req.dates.into_iter().map(|d| (d.card_id, d.due)).collect();
        close::apply_review_dates(&*h.stores().tracker, &dates, now)?;
        Ok(())
    })
    .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// F19: the misconception register, folded live from the course's events.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/courses/{id}/register",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    responses((status = 200, body = RegisterListDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn register(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<RegisterListDto>, ApiError> {
    course_id(&id)?;
    let h = open(&st, &ws, false).await?;
    let rows = blocking(move || {
        let course = h.stores().courses.load_course(&id)?;
        let events = h.stores().courses.events(&id)?;
        Ok(lp_core::method::register::fold_register(&course, &events))
    })
    .await?;
    Ok(Json(RegisterListDto {
        rows: rows.into_iter().map(Into::into).collect(),
    }))
}

/// F14: the mastery bar's count line across the whole course.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/courses/{id}/mastery",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id"), ("id" = String, Path, description = "Course id")),
    responses((status = 200, body = MasteryCountDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn mastery(
    State(st): State<AppState>,
    Path((ws, id)): Path<(String, String)>,
) -> Result<Json<MasteryCountDto>, ApiError> {
    course_id(&id)?;
    let h = open(&st, &ws, false).await?;
    let count_line = blocking(move || {
        let course = h.stores().courses.load_course(&id)?;
        let events = h.stores().courses.events(&id)?;
        Ok(close::mastery_count_line(&course, &events))
    })
    .await?;
    Ok(Json(MasteryCountDto { count_line }))
}

// ------------------------------------------------------------------ bout, check-in, schedule, profile
//
// F01/F03/F04/F23/F24/F34: workspace-wide learning-session state (`lp_service::learning`), not
// tied to any one course. `BoutEvent`s are the sole stored truth, so the clock survives a restart.

/// F23's clock plus F24's gap-pause flag, built purely from the append-only bout log.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/learning/clock",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    responses((status = 200, body = ClockDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn clock(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<ClockDto>, ApiError> {
    let h = open(&st, &ws, false).await?;
    let now = lp_service::now_ms();
    let dto = blocking(move || {
        let svc = learning(&h);
        let settings = svc.settings()?;
        let clock = svc.clock(now)?;
        Ok(to_clock_dto(clock, &settings))
    })
    .await?;
    Ok(Json(dto))
}

/// Starts a bout, or returns the open one (restoring it after a restart). Refused inside the gap
/// or when the day is spent (`409` with `details.gate: gap|daily_limit`), or when F03's ready
/// checklist is not complete (`409 not_ready`).
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/learning/bout/start",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    request_body = StartBoutRequest,
    responses(
        (status = 200, body = ClockDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Refused", body = ErrorEnvelope),
    )
)]
pub async fn bout_start(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(req): ApiJson<StartBoutRequest>,
) -> Result<Json<ClockDto>, ApiError> {
    course_id(&req.course)?;
    let h = open(&st, &ws, true).await?;
    let now = lp_service::now_ms();
    let dto = blocking(move || {
        let svc = learning(&h);
        let settings = svc.settings()?;
        svc.start_bout(now, &req.course, req.ready.into())?;
        let clock = svc.clock(now)?;
        Ok(to_clock_dto(clock, &settings))
    })
    .await?;
    Ok(Json(dto))
}

/// Records a heartbeat for the open bout and returns the fresh clock.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/learning/bout/heartbeat",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    request_body = HeartbeatRequest,
    responses(
        (status = 200, body = ClockDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
    )
)]
pub async fn bout_heartbeat(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(req): ApiJson<HeartbeatRequest>,
) -> Result<Json<ClockDto>, ApiError> {
    bout_id(&req.bout)?;
    let h = open(&st, &ws, true).await?;
    let now = lp_service::now_ms();
    let dto = blocking(move || {
        let svc = learning(&h);
        let settings = svc.settings()?;
        let clock = svc.heartbeat(now, &req.bout)?;
        Ok(to_clock_dto(clock, &settings))
    })
    .await?;
    Ok(Json(dto))
}

/// Ends the open bout.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/learning/bout/end",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    request_body = HeartbeatRequest,
    responses(
        (status = 200, body = ClockDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
    )
)]
pub async fn bout_end(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(req): ApiJson<HeartbeatRequest>,
) -> Result<Json<ClockDto>, ApiError> {
    bout_id(&req.bout)?;
    let h = open(&st, &ws, true).await?;
    let now = lp_service::now_ms();
    let dto = blocking(move || {
        let svc = learning(&h);
        let settings = svc.settings()?;
        let clock = svc.end_bout(now, &req.bout)?;
        Ok(to_clock_dto(clock, &settings))
    })
    .await?;
    Ok(Json(dto))
}

/// The tab lost focus; logged, no effect on the clock.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/learning/bout/focus-lost",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    request_body = HeartbeatRequest,
    responses(
        (status = 204),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
    )
)]
pub async fn bout_focus_lost(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(req): ApiJson<HeartbeatRequest>,
) -> Result<StatusCode, ApiError> {
    bout_id(&req.bout)?;
    let h = open(&st, &ws, true).await?;
    let now = lp_service::now_ms();
    blocking(move || Ok(learning(&h).focus_lost(now, &req.bout)?)).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// F23's one logged override a day; lifts the gap and the daily limit until tomorrow.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/learning/bout/override",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    request_body = OverrideRequest,
    responses(
        (status = 204),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Already used today", body = ErrorEnvelope),
    )
)]
pub async fn bout_override(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(req): ApiJson<OverrideRequest>,
) -> Result<StatusCode, ApiError> {
    text("reason", &req.reason, 0, MAX_REASON)?;
    let h = open(&st, &ws, true).await?;
    let now = lp_service::now_ms();
    blocking(move || Ok(learning(&h).override_limits(now, &req.reason)?)).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// True only before the first check-in or bout of the day (F04).
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/learning/checkin/needed",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    responses((status = 200, body = CheckinNeededDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn checkin_needed(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<CheckinNeededDto>, ApiError> {
    let h = open(&st, &ws, false).await?;
    let now = lp_service::now_ms();
    let needed = blocking(move || Ok(learning(&h).needs_checkin(now)?)).await?;
    Ok(Json(CheckinNeededDto { needed }))
}

/// Records the day's sleep check-in; suggests a review-only day under the poor-sleep threshold.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/learning/checkin",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    request_body = CheckinRequest,
    responses(
        (status = 200, body = CheckinResultDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
    )
)]
pub async fn checkin(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(req): ApiJson<CheckinRequest>,
) -> Result<Json<CheckinResultDto>, ApiError> {
    text("note", &req.note, 0, MAX_TEXT)?;
    let h = open(&st, &ws, true).await?;
    let now = lp_service::now_ms();
    let review_only_suggested =
        blocking(move || Ok(learning(&h).record_checkin(now, req.sleep_hours, &req.note)?)).await?;
    Ok(Json(CheckinResultDto {
        review_only_suggested,
    }))
}

/// F01: the saved wake time and weekly slots.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/learning/schedule",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    responses((status = 200, body = ScheduleDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn schedule_get(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<ScheduleDto>, ApiError> {
    let h = open(&st, &ws, false).await?;
    let sch = blocking(move || Ok(learning(&h).schedule()?)).await?;
    Ok(Json(sch.into()))
}

/// Saves the wake time and weekly slots ("HH:MM").
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/learning/schedule",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    request_body = SetScheduleRequest,
    responses(
        (status = 200, body = ScheduleDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
    )
)]
pub async fn schedule_set(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(req): ApiJson<SetScheduleRequest>,
) -> Result<Json<ScheduleDto>, ApiError> {
    let h = open(&st, &ws, true).await?;
    let sch = blocking(move || Ok(learning(&h).set_schedule(&req.wake_time, req.slots)?)).await?;
    Ok(Json(sch.into()))
}

/// Three suggested slots: the first about 3 hours after waking, then one bout plus gap apart.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/learning/schedule/suggest",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    request_body = SuggestSlotsRequest,
    responses(
        (status = 200, body = SuggestSlotsResultDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
    )
)]
pub async fn schedule_suggest(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(req): ApiJson<SuggestSlotsRequest>,
) -> Result<Json<SuggestSlotsResultDto>, ApiError> {
    let h = open(&st, &ws, false).await?;
    let slots = blocking(move || Ok(learning(&h).suggest_slots(&req.wake_time)?)).await?;
    Ok(Json(SuggestSlotsResultDto { slots }))
}

/// Kept, missed or upcoming for each slot over the last two weeks.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/learning/schedule/track",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    responses((status = 200, body = TrackSlotsResultDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn schedule_track(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<TrackSlotsResultDto>, ApiError> {
    let h = open(&st, &ws, false).await?;
    let now = lp_service::now_ms();
    let slots = blocking(move || Ok(learning(&h).track_slots(now, TRACK_DAYS)?)).await?;
    Ok(Json(TrackSlotsResultDto {
        slots: slots.into_iter().map(Into::into).collect(),
    }))
}

/// F34: the current profile entries (latest per key), excluding per-course whys.
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/learning/profile",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    responses((status = 200, body = ProfileResultDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn profile_get(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<ProfileResultDto>, ApiError> {
    let h = open(&st, &ws, false).await?;
    let entries = blocking(move || Ok(learning(&h).profile()?)).await?;
    Ok(Json(ProfileResultDto {
        entries: entries.into_iter().map(Into::into).collect(),
    }))
}

/// Adds a dated profile entry; the latest per key wins and older ones stay in the log.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/learning/profile",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    request_body = SetProfileRequest,
    responses(
        (status = 200, body = ProfileResultDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
    )
)]
pub async fn profile_set(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(req): ApiJson<SetProfileRequest>,
) -> Result<Json<ProfileResultDto>, ApiError> {
    text("key", &req.key, 1, MAX_ID)?;
    text("value", &req.value, 1, MAX_TEXT)?;
    let h = open(&st, &ws, true).await?;
    let now = lp_service::now_ms();
    let entries = blocking(move || {
        let svc = learning(&h);
        svc.set_profile(now, &req.key, &req.value)?;
        Ok(svc.profile()?)
    })
    .await?;
    Ok(Json(ProfileResultDto {
        entries: entries.into_iter().map(Into::into).collect(),
    }))
}

/// Every tunable method setting with its default and allowed range (the plan's defaults ship as
/// settings, not as facts).
#[utoipa::path(
    get,
    path = "/workspaces/{ws}/learning/settings",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    responses((status = 200, body = MethodSettingsDto), (status = 404, body = ErrorEnvelope))
)]
pub async fn method_settings_get(
    State(st): State<AppState>,
    Path(ws): Path<String>,
) -> Result<Json<MethodSettingsDto>, ApiError> {
    let h = open(&st, &ws, false).await?;
    let current = blocking(move || Ok(ws_settings(&h)?)).await?;
    Ok(Json(method_settings_dto(&current)))
}

/// Changes one setting for this workspace; the value must sit inside the setting's range.
#[utoipa::path(
    post,
    path = "/workspaces/{ws}/learning/settings",
    tag = "courses",
    params(("ws" = String, Path, description = "Workspace id")),
    request_body = SetMethodSettingRequest,
    responses(
        (status = 200, body = MethodSettingsDto),
        (status = 400, body = ErrorEnvelope),
        (status = 404, body = ErrorEnvelope),
        (status = 409, description = "Workspace archived", body = ErrorEnvelope),
    )
)]
pub async fn method_settings_set(
    State(st): State<AppState>,
    Path(ws): Path<String>,
    ApiJson(req): ApiJson<SetMethodSettingRequest>,
) -> Result<Json<MethodSettingsDto>, ApiError> {
    let h = open(&st, &ws, true).await?;
    let current = blocking(move || {
        learning(&h).set_setting(&req.name, req.value)?;
        Ok(ws_settings(&h)?)
    })
    .await?;
    Ok(Json(method_settings_dto(&current)))
}
