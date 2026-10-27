//! FSRS v5 scheduler, card shape validation, 20k-token compaction cutfinder, and token/cost accounting (T12).
//!
//! The scheduler is a port of the `ts-fsrs` build `tracker/server/digest/fsrs.ts` replays with
//! (fuzz off, short-term on, retention 0.9, default weights), so both devices derive identical schedules.
use std::sync::LazyLock;

use regex::Regex;
use serde::{Deserialize, Serialize};
use thiserror::Error;

// ---------------------------------------------------------------- scheduler

const REQUEST_RETENTION: f64 = 0.9;
/// ts-fsrs default; tracker never overrides it.
const MAX_INTERVAL_DAYS: f64 = 36500.0;
const S_MIN: f64 = 1e-3;
const S_MAX: f64 = 36500.0;
const MS_DAY: i64 = 86_400_000;
const MS_MIN: i64 = 60_000;
const LEARNING_STEPS: [i64; 2] = [1, 10];
const RELEARNING_STEPS: [i64; 1] = [10];

const W: [f64; 21] = [
    0.212, 1.2931, 2.3065, 8.2956, 6.4133, 0.8334, 3.0194, 1e-3, 1.8722, 0.1666, 0.796, 1.4835,
    0.0614, 0.2629, 1.6483, 0.6014, 1.8729, 0.5425, 0.0912, 0.0658, 0.1542,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Rating {
    Again = 1,
    Hard = 2,
    Good = 3,
    Easy = 4,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum State {
    New = 0,
    Learning = 1,
    Review = 2,
    Relearning = 3,
}

/// Times are unix milliseconds.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Card {
    pub due: i64,
    pub stability: f64,
    pub difficulty: f64,
    pub reps: u32,
    pub lapses: u32,
    pub learning_steps: u32,
    pub state: State,
    pub last_review: Option<i64>,
}

impl Card {
    pub fn new(due: i64) -> Self {
        Card {
            due,
            stability: 0.0,
            difficulty: 0.0,
            reps: 0,
            lapses: 0,
            learning_steps: 0,
            state: State::New,
            last_review: None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReviewEvent {
    pub id: String,
    pub ts: i64,
    pub rating: Rating,
}

fn round_to(x: f64, decimals: i32) -> f64 {
    let f = 10f64.powi(decimals);
    (x * f + 0.5).floor() / f
}

fn decay_factor() -> (f64, f64) {
    let decay = -W[20];
    (decay, round_to((0.9f64.ln() / decay).exp() - 1.0, 8))
}

fn forgetting_curve(elapsed_days: f64, stability: f64) -> f64 {
    let (decay, factor) = decay_factor();
    round_to((1.0 + factor * elapsed_days / stability).powf(decay), 8)
}

fn next_interval(stability: f64) -> i64 {
    let (decay, factor) = decay_factor();
    let modifier = round_to((REQUEST_RETENTION.powf(1.0 / decay) - 1.0) / factor, 8);
    (stability * modifier).round().clamp(1.0, MAX_INTERVAL_DAYS) as i64
}

fn init_difficulty(g: Rating) -> f64 {
    round_to(W[4] - ((g as i32 - 1) as f64 * W[5]).exp() + 1.0, 8)
}

fn next_difficulty(d: f64, g: Rating) -> f64 {
    let delta = -W[6] * (g as i32 - 3) as f64;
    let next = d + round_to(delta * (10.0 - d) / 9.0, 8);
    round_to(
        W[7] * init_difficulty(Rating::Easy) + (1.0 - W[7]) * next,
        8,
    )
    .clamp(1.0, 10.0)
}

fn recall_stability(d: f64, s: f64, r: f64, g: Rating) -> f64 {
    let hard = if g == Rating::Hard { W[15] } else { 1.0 };
    let easy = if g == Rating::Easy { W[16] } else { 1.0 };
    let v = s
        * (1.0
            + W[8].exp()
                * (11.0 - d)
                * s.powf(-W[9])
                * (((1.0 - r) * W[10]).exp() - 1.0)
                * hard
                * easy);
    round_to(v.clamp(S_MIN, S_MAX), 8)
}

fn forget_stability(d: f64, s: f64, r: f64) -> f64 {
    let v = W[11] * d.powf(-W[12]) * ((s + 1.0).powf(W[13]) - 1.0) * ((1.0 - r) * W[14]).exp();
    round_to(v.clamp(S_MIN, S_MAX), 8)
}

fn short_term_stability(s: f64, g: Rating) -> f64 {
    let sinc = s.powf(-W[19]) * (W[17] * ((g as i32 - 3) as f64 + W[18])).exp();
    let sinc = if g >= Rating::Hard {
        sinc.max(1.0)
    } else {
        sinc
    };
    round_to((s * sinc).clamp(S_MIN, S_MAX), 8)
}

/// New (difficulty, stability) after rating `g` `t` whole days after the last review.
fn next_state(card: &Card, t: i64, g: Rating, r: Option<f64>) -> (f64, f64) {
    let (d, s) = (card.difficulty, card.stability);
    if d == 0.0 && s == 0.0 {
        return (
            init_difficulty(g).clamp(1.0, 10.0),
            W[g as usize - 1].max(0.1),
        );
    }
    let r = r.unwrap_or_else(|| forgetting_curve(t as f64, s));
    let new_s = if t == 0 {
        short_term_stability(s, g)
    } else if g == Rating::Again {
        let after_fail = forget_stability(d, s, r);
        round_to(s / (W[17] * W[18]).exp(), 8).clamp(S_MIN, after_fail)
    } else {
        recall_stability(d, s, r, g)
    };
    (next_difficulty(d, g), new_s)
}

/// `(minutes, next_step)`; `None` graduates the card to Review.
fn learning_step(state: State, cur: u32, g: Rating) -> Option<(i64, u32)> {
    let steps: &[i64] = match state {
        State::Review | State::Relearning => &RELEARNING_STEPS,
        _ => &LEARNING_STEPS,
    };
    let cur = cur as usize;
    if cur >= steps.len() {
        return None;
    }
    if state == State::Review {
        return (g == Rating::Again).then_some((steps[cur], 0));
    }
    match g {
        Rating::Again => Some((steps[0], 0)),
        Rating::Hard => {
            let m = if steps.len() == 1 {
                (steps[0] as f64 * 1.5).round()
            } else {
                ((steps[0] + steps[1]) as f64 / 2.0).round()
            };
            Some((m as i64, cur as u32))
        }
        Rating::Good => steps.get(cur + 1).map(|&m| (m, cur as u32 + 1)),
        Rating::Easy => None,
    }
}

fn apply_steps(next: &mut Card, prev: &Card, g: Rating, to: State, now: i64) {
    match learning_step(prev.state, prev.learning_steps, g) {
        Some((m, step)) => {
            next.learning_steps = step;
            next.state = to;
            next.due = now + m * MS_MIN;
        }
        None => {
            next.learning_steps = 0;
            next.state = State::Review;
            next.due = now + next_interval(next.stability) * MS_DAY;
        }
    }
}

fn utc_day(ms: i64) -> i64 {
    ms.div_euclid(MS_DAY)
}

/// One FSRS step: the card after a review of rating `g` at `now`.
pub fn review(card: &Card, now: i64, g: Rating) -> Card {
    let elapsed = match (card.state, card.last_review) {
        (State::New, _) | (_, None) => 0,
        (_, Some(last)) => utc_day(now) - utc_day(last),
    };
    let mut next = card.clone();
    next.last_review = Some(now);
    next.reps += 1;
    if card.state != State::Review {
        let to = if card.state == State::New {
            State::Learning
        } else {
            card.state
        };
        (next.difficulty, next.stability) = next_state(card, elapsed, g, None);
        apply_steps(&mut next, card, g, to, now);
        return next;
    }
    let r = forgetting_curve(elapsed as f64, card.stability);
    if g == Rating::Again {
        (next.difficulty, next.stability) = next_state(card, elapsed, g, Some(r));
        apply_steps(&mut next, card, g, State::Relearning, now);
        next.lapses += 1;
        return next;
    }
    let ivl = |g| next_interval(next_state(card, elapsed, g, Some(r)).1);
    let hard = ivl(Rating::Hard);
    let good = ivl(Rating::Good);
    let hard = hard.min(good);
    let good = good.max(hard + 1);
    let days = match g {
        Rating::Hard => hard,
        Rating::Good => good,
        _ => ivl(Rating::Easy).max(good + 1),
    };
    (next.difficulty, next.stability) = next_state(card, elapsed, g, Some(r));
    next.state = State::Review;
    next.learning_steps = 0;
    next.due = now + days * MS_DAY;
    next
}

/// Rebuild a card from its review log; events are ordered by `(ts, id)` so device merge order never matters.
pub fn replay(events: &[ReviewEvent]) -> Card {
    let mut sorted: Vec<&ReviewEvent> = events.iter().collect();
    sorted.sort_by(|a, b| (a.ts, &a.id).cmp(&(b.ts, &b.id)));
    sorted
        .into_iter()
        .fold(Card::new(0), |c, e| review(&c, e.ts, e.rating))
}

/// Ms until due for Again/Hard/Good/Easy, used to label the grading buttons.
pub fn preview(card: Option<&Card>, now: i64) -> [i64; 4] {
    let base = card.cloned().unwrap_or_else(|| Card::new(now));
    [Rating::Again, Rating::Hard, Rating::Good, Rating::Easy]
        .map(|g| review(&base, now, g).due - now)
}

// ---------------------------------------------------------------- card shape

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardDraft {
    pub front: String,
    pub right: String,
    pub wrong: [String; 3],
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CardShapeError {
    #[error("question depends on context the quiz never shows")]
    ContextDependent,
    #[error("empty option")]
    EmptyOption,
    #[error("options are not distinct")]
    DuplicateOptions,
    #[error("correct option is a length giveaway")]
    LengthTell,
}

static CONTEXT_DEPENDENT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(in this (project|session|repo|codebase|commit)|why did (we|you|the user)|this (commit|change|diff)|the (commit|diff|evidence|decision notes?|notes)|following this|the spec says)\b",
    )
    .expect("static regex")
});

/// Hard quality gate for generated multiple-choice cards. A fenced code block counts as stating the setup inline.
pub fn validate_card_shape(card: &CardDraft) -> Result<(), CardShapeError> {
    if CONTEXT_DEPENDENT.is_match(&card.front) && !card.front.contains("```") {
        return Err(CardShapeError::ContextDependent);
    }
    let opts = std::iter::once(&card.right).chain(&card.wrong);
    if opts.clone().any(|o| o.trim().is_empty()) {
        return Err(CardShapeError::EmptyOption);
    }
    let mut seen: Vec<String> = opts.map(|o| o.trim().to_lowercase()).collect();
    seen.sort();
    seen.dedup();
    if seen.len() != 4 {
        return Err(CardShapeError::DuplicateOptions);
    }
    let mean = card.wrong.iter().map(|o| o.chars().count()).sum::<usize>() as f64 / 3.0;
    if card.right.chars().count() as f64 > 1.6 * mean + 10.0 {
        return Err(CardShapeError::LengthTell);
    }
    Ok(())
}

// ---------------------------------------------------------------- compaction

pub const RETAINED_TAIL_TOKENS: u64 = 20_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MsgKind {
    Other,
    QuizCall,
    QuizAnswer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CompactMsg {
    pub kind: MsgKind,
    pub tokens: u64,
}

/// Index where the retained tail starts: messages `..cut` get summarized. The tail holds at least
/// `RETAINED_TAIL_TOKENS`, and the cut snaps backward so a `[quiz_call, quiz_answer]` pair is never split.
pub fn compaction_cut(msgs: &[CompactMsg]) -> usize {
    let mut cut = msgs.len();
    let mut tail = 0u64;
    while cut > 0 && tail < RETAINED_TAIL_TOKENS {
        cut -= 1;
        tail = tail.saturating_add(msgs[cut].tokens);
    }
    if tail < RETAINED_TAIL_TOKENS {
        return 0;
    }
    while cut > 0 && msgs[cut].kind == MsgKind::QuizAnswer {
        cut -= 1;
    }
    cut
}

// ---------------------------------------------------------------- usage

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnUsage {
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cache_read_tokens: u64,
    pub cache_write_tokens: u64,
    pub reasoning_tokens: u64,
}

impl TurnUsage {
    fn plus(self, o: &TurnUsage) -> TurnUsage {
        TurnUsage {
            input_tokens: self.input_tokens.saturating_add(o.input_tokens),
            output_tokens: self.output_tokens.saturating_add(o.output_tokens),
            cache_read_tokens: self.cache_read_tokens.saturating_add(o.cache_read_tokens),
            cache_write_tokens: self.cache_write_tokens.saturating_add(o.cache_write_tokens),
            reasoning_tokens: self.reasoning_tokens.saturating_add(o.reasoning_tokens),
        }
    }
}

/// Fold the append-only `turns` and `extractions` logs of a job into one total.
pub fn accumulate_job_usage(turns: &[TurnUsage], extractions: &[TurnUsage]) -> TurnUsage {
    turns
        .iter()
        .chain(extractions)
        .fold(TurnUsage::default(), TurnUsage::plus)
}

#[cfg(test)]
mod tests {
    use super::*;

    const T0: i64 = 1_767_258_000_000; // 2026-01-01T09:00:00Z
    const M: i64 = MS_MIN;
    const D: i64 = MS_DAY;

    fn run(seq: &[(i64, Rating)]) -> Card {
        let ev: Vec<_> = seq
            .iter()
            .enumerate()
            .map(|(i, &(o, rating))| ReviewEvent {
                id: i.to_string(),
                ts: T0 + o,
                rating,
            })
            .collect();
        replay(&ev)
    }

    /// Expected values come from `replay` in tracker/server/digest/fsrs.ts (ts-fsrs 5.4.2) on the same inputs.
    fn check(
        c: Card,
        (due, s, d): (i64, f64, f64),
        (reps, lapses, state): (u32, u32, State),
        last: i64,
    ) {
        assert_eq!(c.due - T0, due);
        assert!((c.stability - s).abs() < 1e-8, "s {}", c.stability);
        assert!((c.difficulty - d).abs() < 1e-8, "d {}", c.difficulty);
        assert_eq!((c.reps, c.lapses, c.state), (reps, lapses, state));
        assert_eq!(c.last_review, Some(T0 + last));
    }

    use Rating::{Again, Easy, Good, Hard};

    #[test]
    fn replay_matches_tracker_single_good() {
        check(
            run(&[(0, Good)]),
            (600_000, 2.3065, 2.11810397),
            (1, 0, State::Learning),
            0,
        );
    }

    #[test]
    fn replay_matches_tracker_graduation_and_review() {
        let c = run(&[
            (0, Good),
            (10 * M, Good),
            (D, Good),
            (5 * D, Good),
            (20 * D, Easy),
        ]);
        check(
            c,
            (11_404_800_000, 112.22134984, 1.0),
            (5, 0, State::Review),
            1_728_000_000,
        );
    }

    #[test]
    fn replay_matches_tracker_lapse_and_relearn() {
        let seq = [
            (0, Again),
            (M, Hard),
            (2 * M, Good),
            (3 * D, Good),
            (10 * D, Again),
            (10 * D + 10 * M, Good),
            (12 * D, Hard),
        ];
        check(
            run(&seq),
            (1_209_600_000, 1.56558288, 9.4379901),
            (7, 1, State::Review),
            1_036_800_000,
        );
    }

    #[test]
    fn replay_matches_tracker_same_day_relapse() {
        let seq = [
            (0, Easy),
            (3 * D, Good),
            (3 * D + 3_600_000, Again),
            (9 * D, Good),
        ];
        check(
            run(&seq),
            (2_160_000_000, 16.07687853, 7.01519095),
            (4, 1, State::Review),
            777_600_000,
        );
    }

    #[test]
    fn replay_orders_by_ts_then_id() {
        let a = ReviewEvent {
            id: "a".into(),
            ts: T0,
            rating: Good,
        };
        let b = ReviewEvent {
            id: "b".into(),
            ts: T0 + D,
            rating: Again,
        };
        assert_eq!(replay(&[b.clone(), a.clone()]), replay(&[a, b]));
    }

    #[test]
    fn preview_labels_new_card_steps() {
        assert_eq!(preview(None, T0)[..3], [M, 6 * M, 10 * M]);
    }

    fn draft(front: &str, right: &str, wrong: [&str; 3]) -> CardDraft {
        CardDraft {
            front: front.into(),
            right: right.into(),
            wrong: wrong.map(Into::into),
        }
    }

    #[test]
    fn card_shape_gates() {
        let ok = draft(
            "What does `Rc::clone` copy?",
            "the pointer",
            ["the heap value", "the vtable", "nothing at all"],
        );
        assert_eq!(validate_card_shape(&ok), Ok(()));
        let ctx = draft(
            "Why did we pick sled in this project?",
            "a",
            ["b", "c", "d"],
        );
        assert_eq!(
            validate_card_shape(&ctx),
            Err(CardShapeError::ContextDependent)
        );
        let inline = draft(
            "In this commit ```fn f(){}``` what breaks?",
            "a",
            ["b", "c", "d"],
        );
        assert_eq!(validate_card_shape(&inline), Ok(()));
        let dup = draft("q", "Same", ["same ", "c", "d"]);
        assert_eq!(
            validate_card_shape(&dup),
            Err(CardShapeError::DuplicateOptions)
        );
        let empty = draft("q", "a", ["b", " ", "d"]);
        assert_eq!(
            validate_card_shape(&empty),
            Err(CardShapeError::EmptyOption)
        );
        let long = draft("q", &"x".repeat(40), ["aaaa", "bbbb", "cccc"]);
        assert_eq!(validate_card_shape(&long), Err(CardShapeError::LengthTell));
        let edge = draft("q", &"x".repeat(14), ["aaaa", "bbbb", "cccc"]); // 1.6*4+10 = 16.4
        assert_eq!(validate_card_shape(&edge), Ok(()));
    }

    fn m(kind: MsgKind, tokens: u64) -> CompactMsg {
        CompactMsg { kind, tokens }
    }

    #[test]
    fn cut_keeps_20k_tail() {
        let msgs = vec![m(MsgKind::Other, 10_000); 5];
        assert_eq!(compaction_cut(&msgs), 3); // last two = 20k exactly
        assert_eq!(compaction_cut(&msgs[..2]), 0); // exactly 20k total: all retained
        assert_eq!(compaction_cut(&msgs[..1]), 0);
        assert_eq!(compaction_cut(&[]), 0);
    }

    #[test]
    fn cut_never_splits_quiz_pair() {
        let msgs = [
            m(MsgKind::Other, 5_000),
            m(MsgKind::QuizCall, 100),
            m(MsgKind::QuizAnswer, 20_000),
            m(MsgKind::Other, 100),
        ];
        // the raw cut lands on the answer (index 2); snap back to include the call
        assert_eq!(compaction_cut(&msgs), 1);
    }

    #[test]
    fn usage_sums_turns_and_extractions() {
        let t = TurnUsage {
            input_tokens: 3,
            output_tokens: 2,
            cache_read_tokens: 1,
            cache_write_tokens: 4,
            reasoning_tokens: 5,
        };
        let e = TurnUsage {
            input_tokens: u64::MAX,
            ..TurnUsage::default()
        };
        let total = accumulate_job_usage(&[t, t], &[e]);
        assert_eq!(total.input_tokens, u64::MAX);
        assert_eq!((total.output_tokens, total.reasoning_tokens), (4, 10));
        assert_eq!(accumulate_job_usage(&[], &[]), TurnUsage::default());
    }
}
