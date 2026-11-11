//! FSRS review wire types (docs/web/03 §Review).

use lp_core::fsrs::{Rating, State};
use lp_core::ports::{CardKind, FlashCard, FsrsReviewLog};
use lp_service::review;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

const MS_DAY: f64 = 86_400_000.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum AnswerMode {
    Mcq,
    Recall,
}

impl AnswerMode {
    pub fn of(c: &FlashCard) -> Self {
        match c.kind {
            CardKind::Mcq => Self::Mcq,
            CardKind::Recall => Self::Recall,
        }
    }
}

/// An answer choice. `key` is `a`..`d` and is what `answer` takes.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct ChoiceDto {
    pub key: String,
    pub text: String,
}

/// Recall cards are typed, so they have no choices.
fn choices(c: &FlashCard) -> Vec<ChoiceDto> {
    if c.kind == CardKind::Recall {
        return Vec::new();
    }
    c.options
        .iter()
        .enumerate()
        .map(|(i, text)| ChoiceDto {
            key: review::option_key(i).to_string(),
            text: text.clone(),
        })
        .collect()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CardState {
    New,
    Learning,
    Review,
    Relearning,
}

impl From<State> for CardState {
    fn from(s: State) -> Self {
        match s {
            State::New => Self::New,
            State::Learning => Self::Learning,
            State::Review => Self::Review,
            State::Relearning => Self::Relearning,
        }
    }
}

/// A card as lists show it: scheduling, never the answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CardBriefDto {
    pub id: String,
    pub front: String,
    pub state: CardState,
    /// Epoch ms.
    pub due: i64,
    pub reps: u32,
    pub lapses: u32,
    pub last_review: Option<i64>,
    /// `0..=1`.
    pub mastery: f64,
    pub source_decision_id: Option<String>,
    /// Due at the server's `now`.
    pub is_due: bool,
}

impl CardBriefDto {
    pub fn new(c: &FlashCard, decision: Option<&str>, now: i64) -> Self {
        Self {
            id: c.id.to_string(),
            front: c.front.clone(),
            state: c.card.state.into(),
            due: c.card.due,
            reps: c.card.reps,
            lapses: c.card.lapses,
            last_review: c.card.last_review,
            mastery: review::mastery(&c.card),
            source_decision_id: decision.map(str::to_owned),
            is_due: c.card.due <= now,
        }
    }
}

/// A due card to answer. The correct option and rationale are withheld until `answer`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct QueueCardDto {
    pub id: String,
    pub front: String,
    /// Empty for a `recall` card.
    pub options: Vec<ChoiceDto>,
    /// `mcq` (pick a choice) or `recall` (type the answer; send `text`).
    pub answer_mode: AnswerMode,
    /// `decision` for cards made from an ingested decision, else `card`.
    pub kind: String,
    pub source_decision_id: Option<String>,
    pub due: i64,
    pub state: CardState,
}

impl QueueCardDto {
    pub fn new(c: &FlashCard, decision: Option<&str>) -> Self {
        Self {
            id: c.id.to_string(),
            front: c.front.clone(),
            options: choices(c),
            answer_mode: AnswerMode::of(c),
            kind: if decision.is_some() {
                "decision"
            } else {
                "card"
            }
            .into(),
            source_decision_id: decision.map(str::to_owned),
            due: c.card.due,
            state: c.card.state.into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct QueueDto {
    pub items: Vec<QueueCardDto>,
    /// Cards due now, beyond the page.
    pub due_now: usize,
    /// The server clock the queue was computed at (epoch ms).
    pub now: i64,
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct QueueQuery {
    pub limit: Option<usize>,
}

/// What grading with one rating would schedule.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct PreviewDto {
    /// 1 again, 2 hard, 3 good, 4 easy.
    pub rating: u8,
    pub label: String,
    pub interval_ms: i64,
    /// `1m`, `3h`, `5d`.
    pub interval_label: String,
    pub due: i64,
}

pub fn rating_label(r: Rating) -> &'static str {
    match r {
        Rating::Again => "again",
        Rating::Hard => "hard",
        Rating::Good => "good",
        Rating::Easy => "easy",
    }
}

pub fn preview(c: &FlashCard, now: i64) -> Vec<PreviewDto> {
    review::preview_entries(&c.card, now)
        .into_iter()
        .map(|(r, ms, label)| PreviewDto {
            rating: r as u8,
            label: rating_label(r).to_owned(),
            interval_ms: ms,
            interval_label: label,
            due: now + ms,
        })
        .collect()
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
pub struct PreviewListDto {
    /// `[again, hard, good, easy]`.
    pub items: Vec<PreviewDto>,
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnswerRequest {
    /// `a`..`d`. Required for an `mcq` card.
    #[serde(default)]
    pub choice_key: Option<String>,
    /// The typed answer. Required for a `recall` card; graded by code.
    #[serde(default)]
    pub text: Option<String>,
    pub client_id: String,
    /// 1..=5.
    pub confidence: Option<u8>,
    /// How long the learner took, measured by the client.
    pub latency_ms: Option<i64>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct NextDto {
    pub due: i64,
    pub interval_days: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct AnswerDto {
    pub correct: bool,
    /// Empty for a `recall` card.
    pub correct_key: String,
    /// The expected answer, set for a `recall` card.
    pub answer: Option<String>,
    pub explanation: String,
    /// The grade the server suggests (1..=4).
    pub rating: u8,
    /// What the suggested grade would schedule.
    pub next: NextDto,
    pub preview: Vec<PreviewDto>,
}

impl AnswerDto {
    /// `picked` is the chosen option for an `mcq` card; `text` the typed answer for a `recall` one.
    pub fn new(
        c: &FlashCard,
        picked: Option<usize>,
        text: Option<&str>,
        req: &AnswerRequest,
        now: i64,
    ) -> Self {
        let recall = c.kind == CardKind::Recall;
        let correct = if recall {
            review::grade_recall(c, text.unwrap_or_default())
        } else {
            picked == Some(usize::from(c.correct_index))
        };
        let rating = review::suggest_rating(correct, req.latency_ms, req.confidence);
        let after = lp_core::fsrs::review(&c.card, now, rating);
        Self {
            correct,
            correct_key: if recall {
                String::new()
            } else {
                review::option_key(usize::from(c.correct_index)).to_string()
            },
            answer: recall.then(|| c.options[usize::from(c.correct_index)].clone()),
            explanation: c.rationale.clone(),
            rating: rating as u8,
            next: NextDto {
                due: after.due,
                interval_days: ((after.due - now) as f64 / MS_DAY * 100.0).round() / 100.0,
            },
            preview: preview(c, now),
        }
    }
}

#[derive(Clone, Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GradeRequest {
    /// 1 again, 2 hard, 3 good, 4 easy.
    pub rating: u8,
    /// Retries with the same id return the first result instead of grading twice.
    pub client_id: String,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct GradeDto {
    pub card: CardBriefDto,
    pub preview: Vec<PreviewDto>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct StatsDto {
    pub due_now: usize,
    pub due_today: usize,
    pub new_today: usize,
    pub streak: u32,
    pub reviewed_today: usize,
    /// Share of recent reviews that were not `Again`, `0..=1`; null with no data.
    pub retention30d: Option<f64>,
    /// Cards due per UTC day, starting `forecast_start`; day 0 includes overdue cards.
    pub forecast: Vec<u32>,
    pub forecast_start: i64,
    /// The server clock the stats were computed at (epoch ms).
    pub now: i64,
}

impl StatsDto {
    pub fn new(s: review::Stats, now: i64) -> Self {
        Self {
            due_now: s.due_now,
            due_today: s.due_today,
            new_today: s.new_today,
            streak: s.streak,
            reviewed_today: s.reviewed_today,
            retention30d: s.retention_30d,
            forecast: s.forecast.to_vec(),
            forecast_start: s.forecast_start,
            now,
        }
    }
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct CardsQuery {
    /// `new`, `learning`, `review` or `relearning`.
    pub state: Option<String>,
    pub q: Option<String>,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CardPageDto {
    pub items: Vec<CardBriefDto>,
    pub next_cursor: Option<String>,
    pub total: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewLogDto {
    pub ts: i64,
    /// 1..=4.
    pub rating: u8,
    pub device_id: String,
}

impl From<FsrsReviewLog> for ReviewLogDto {
    fn from(r: FsrsReviewLog) -> Self {
        Self {
            ts: r.ts,
            rating: r.rating as u8,
            device_id: r.device_id.to_string(),
        }
    }
}

/// A card with everything, for the browser's detail drawer. Includes the answer.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase")]
pub struct CardDetailDto {
    #[serde(flatten)]
    pub card: CardBriefDto,
    pub options: Vec<ChoiceDto>,
    pub answer_mode: AnswerMode,
    pub correct_key: String,
    pub rationale: String,
    pub stability: f64,
    pub difficulty: f64,
    pub history: Vec<ReviewLogDto>,
    pub preview: Vec<PreviewDto>,
}

impl CardDetailDto {
    pub fn new(
        c: &FlashCard,
        decision: Option<&str>,
        history: Vec<FsrsReviewLog>,
        now: i64,
    ) -> Self {
        Self {
            card: CardBriefDto::new(c, decision, now),
            options: choices(c),
            answer_mode: AnswerMode::of(c),
            correct_key: review::option_key(usize::from(c.correct_index)).to_string(),
            rationale: c.rationale.clone(),
            stability: c.card.stability,
            difficulty: c.card.difficulty,
            history: history.into_iter().map(Into::into).collect(),
            preview: preview(c, now),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
pub struct SyncDto {
    pub cards: usize,
    pub reviews: usize,
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_core::fsrs::Card;

    fn card() -> FlashCard {
        FlashCard {
            id: "c1".into(),
            front: "Q?".into(),
            options: ["w".into(), "x".into(), "RIGHT".into(), "z".into()],
            correct_index: 2,
            rationale: "because it is".into(),
            kind: Default::default(),
            must_name: Vec::new(),
            card: Card::new(10),
        }
    }

    #[test]
    fn queue_cards_withhold_the_answer() {
        let dto = QueueCardDto::new(&card(), Some("d1"));
        let j = serde_json::to_string(&dto).unwrap();
        for leak in ["correct", "rationale", "because it is"] {
            assert!(!j.contains(leak), "{leak} leaked: {j}");
        }
        assert_eq!(dto.options[2].key, "c");
        assert_eq!(dto.kind, "decision");
        assert_eq!(QueueCardDto::new(&card(), None).kind, "card");
    }

    #[test]
    fn answer_marks_correctness_and_suggests_a_grade() {
        let req = |key: &str, ms| AnswerRequest {
            choice_key: Some(key.into()),
            text: None,
            client_id: "u".into(),
            confidence: None,
            latency_ms: Some(ms),
        };
        let now = 1_000_000;
        let ok = AnswerDto::new(&card(), Some(2), None, &req("c", 2_000), now);
        assert!(ok.correct);
        assert_eq!((ok.correct_key.as_str(), ok.rating), ("c", 4));
        assert_eq!(ok.explanation, "because it is");
        assert_eq!(ok.preview.len(), 4);
        assert_eq!(ok.preview[0].label, "again");
        let miss = AnswerDto::new(&card(), Some(0), None, &req("a", 2_000), now);
        assert!(!miss.correct);
        assert_eq!(miss.rating, 1);
        assert!(miss.next.due < ok.next.due);
        let j = serde_json::to_value(&ok).unwrap();
        assert!(j["next"]["intervalDays"].is_number());
    }

    #[test]
    fn recall_card_is_typed_and_graded_by_rubric_in_code() {
        let mut c = card();
        c.kind = CardKind::Recall;
        c.must_name = vec!["mitochondria".into()];
        let q = QueueCardDto::new(&c, None);
        assert_eq!(q.answer_mode, AnswerMode::Recall);
        assert!(q.options.is_empty());
        let req = AnswerRequest {
            choice_key: None,
            text: Some("it is the mitochondria".into()),
            client_id: "u".into(),
            confidence: None,
            latency_ms: None,
        };
        let ok = AnswerDto::new(&c, None, req.text.as_deref(), &req, 1_000);
        assert!(ok.correct);
        assert_eq!(ok.answer.as_deref(), Some("RIGHT"));
        let miss = AnswerDto::new(&c, None, Some("the nucleus"), &req, 1_000);
        assert!(!miss.correct);
        assert_eq!(miss.rating, 1);
        // An old card (no kind) still answers as multiple choice.
        assert_eq!(
            QueueCardDto::new(&card(), None).answer_mode,
            AnswerMode::Mcq
        );
    }

    #[test]
    fn stats_dto_serializes_the_forecast_as_numbers() {
        let s = review::Stats {
            due_now: 1,
            due_today: 2,
            new_today: 0,
            streak: 3,
            reviewed_today: 4,
            retention_30d: Some(0.9),
            forecast: [1; review::FORECAST_DAYS],
            forecast_start: 5,
        };
        let j = serde_json::to_value(StatsDto::new(s, 9)).unwrap();
        assert_eq!(j["retention30d"], 0.9);
        assert_eq!(j["forecast"].as_array().unwrap().len(), 14);
        assert_eq!(j["forecastStart"], 5);
    }
}
