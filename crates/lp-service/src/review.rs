//! FSRS grading, the due queue, cross-device sync, streak and interval labels.

use lp_core::fsrs::{self, Rating, State};
use lp_core::ports::{FlashCard, FsrsReviewLog, StoreError, StoreResult, TrackerStore};
use lp_store::SqliteTrackerStore;

const MS_DAY: i64 = 86_400_000;
/// Days of per-day due counts in the forecast.
pub const FORECAST_DAYS: usize = 14;

/// Grades a typed answer to a recall card in code, no model: the `must_name` rubric, else the
/// exact answer (accents and case ignored). Anything else is wrong, so wrong is never promoted.
pub fn grade_recall(card: &FlashCard, text: &str) -> bool {
    use lp_core::course::{match_answer, Item, ItemKind, Match};
    use lp_core::method::recall::{rubric_check, RubricResult};
    let item = Item {
        kind: ItemKind::Recall,
        answer: card
            .options
            .get(usize::from(card.correct_index))
            .cloned()
            .unwrap_or_default(),
        must_name: card.must_name.clone(),
        ..Item::default()
    };
    rubric_check(&item, text) == RubricResult::Match
        || matches!(
            match_answer(&item, text),
            Match::Exact | Match::ExactButAccent
        )
}

pub fn due_cards(t: &dyn TrackerStore, now: i64) -> StoreResult<Vec<FlashCard>> {
    let mut due: Vec<FlashCard> = t
        .cards()?
        .into_iter()
        .filter(|c| c.card.due <= now)
        .collect();
    due.sort_by_key(|c| c.card.due);
    Ok(due)
}

/// Replays the merged review log plus this review to get the card, then commits card and log in one
/// transaction. The sync line is written last; a crash before it is repaired by the next `sync`.
pub fn grade(
    t: &dyn TrackerStore,
    device: &str,
    card_id: &str,
    rating: Rating,
    now: i64,
) -> StoreResult<()> {
    let mut card = t
        .card(card_id)?
        .ok_or_else(|| StoreError::NotFound(card_id.into()))?;
    let log = FsrsReviewLog {
        card_id: card.id.clone(),
        ts: now,
        rating,
        device_id: device.into(),
    };
    let mut reviews = t.reviews()?;
    reviews.push(log.clone());
    card.card = replayed(&reviews, card_id);
    t.commit_review(&card, &log)?;
    let mut line = serde_json::to_value(&log).map_err(|e| StoreError::Backend(e.to_string()))?;
    line["kind"] = "review".into();
    t.append_sync(device, &line)
}

fn replayed(reviews: &[FsrsReviewLog], card_id: &str) -> fsrs::Card {
    let events: Vec<_> = reviews
        .iter()
        .filter(|r| r.card_id == card_id)
        .map(|r| fsrs::ReviewEvent {
            id: r.device_id.to_string(),
            ts: r.ts,
            rating: r.rating,
        })
        .collect();
    fsrs::replay(&events)
}

/// Startup sync: merge the other devices' logs, then recompute every reviewed card from the
/// merged review log (PLAN 4.4). Cards without reviews keep their stored state.
pub fn sync(t: &SqliteTrackerStore, device: &str) -> StoreResult<()> {
    t.import_sync(device)?;
    let reviews = t.reviews()?;
    for mut c in t.cards()? {
        if reviews.iter().any(|r| r.card_id == c.id) {
            let card = replayed(&reviews, &c.id);
            if card != c.card {
                c.card = card;
                t.upsert_card(&c)?;
            }
        }
    }
    Ok(())
}

/// Ships every card to the sync log so other devices can schedule it.
pub fn export_cards(t: &dyn TrackerStore, device: &str) -> StoreResult<()> {
    for c in t.cards()? {
        let mut line = serde_json::to_value(&c).map_err(|e| StoreError::Backend(e.to_string()))?;
        line["kind"] = "card".into();
        t.append_sync(device, &line)?;
    }
    Ok(())
}

/// Consecutive days with a review, counting back from today (or yesterday if none yet today).
pub fn streak(t: &dyn TrackerStore, now: i64) -> u32 {
    let mut days: Vec<i64> = t
        .reviews()
        .unwrap_or_default()
        .iter()
        .map(|r| r.ts.div_euclid(MS_DAY))
        .collect();
    days.sort_unstable();
    days.dedup();
    let today = now.div_euclid(MS_DAY);
    let mut want = if days.last() == Some(&today) {
        today
    } else {
        today - 1
    };
    let mut n = 0;
    while days.binary_search(&want).is_ok() {
        n += 1;
        want -= 1;
    }
    n
}

/// A compact label for an interval from `lp_core::fsrs::preview`: `1m`, `3h`, `5d`.
pub fn format_interval_ms(ms: i64) -> String {
    let mins = (ms.max(60_000) + 30_000) / 60_000;
    if mins < 60 {
        format!("{mins}m")
    } else if mins < 60 * 24 {
        format!("{}h", (mins + 30) / 60)
    } else {
        format!("{}d", (mins + 720) / (60 * 24))
    }
}

/// How well a card is known, `0.0..=1.0`, from its FSRS state, stability and reps.
pub fn mastery(card: &fsrs::Card) -> f64 {
    match card.state {
        State::New => 0.15,
        State::Learning => (0.35 + f64::from(card.reps) * 0.08).clamp(0.25, 0.65),
        State::Relearning => (0.30 + (card.stability / 30.0) * 0.25).clamp(0.20, 0.55),
        State::Review => (0.55 + (card.stability / 25.0) * 0.45).clamp(0.55, 0.98),
    }
}

pub fn state_name(s: State) -> &'static str {
    match s {
        State::New => "new",
        State::Learning => "learning",
        State::Review => "review",
        State::Relearning => "relearning",
    }
}

pub fn parse_state(s: &str) -> Option<State> {
    [
        State::New,
        State::Learning,
        State::Review,
        State::Relearning,
    ]
    .into_iter()
    .find(|st| state_name(*st) == s)
}

/// The answer key of option `i` (`a`..`d`).
pub fn option_key(i: usize) -> char {
    char::from(b'a' + (i % 4) as u8)
}

/// The option index a key (`a`..`d`, any case) names.
pub fn option_index(key: &str) -> Option<usize> {
    match key.trim().to_ascii_lowercase().as_bytes() {
        [c @ b'a'..=b'd'] => Some(usize::from(c - b'a')),
        _ => None,
    }
}

/// The grade the server proposes after a card is answered: a miss is `Again`; a hit is `Easy`
/// when quick, `Good` at a normal pace, `Hard` when slow; low confidence (`1..=2` of 5) caps a
/// hit at `Hard`.
pub fn suggest_rating(correct: bool, latency_ms: Option<i64>, confidence: Option<u8>) -> Rating {
    if !correct {
        return Rating::Again;
    }
    let by_pace = match latency_ms {
        None => Rating::Good,
        Some(ms) if ms <= 5_000 => Rating::Easy,
        Some(ms) if ms <= 20_000 => Rating::Good,
        Some(_) => Rating::Hard,
    };
    match confidence {
        Some(c) if c <= 2 => by_pace.min(Rating::Hard),
        _ => by_pace,
    }
}

pub fn rating_from_int(n: u8) -> Option<Rating> {
    match n {
        1 => Some(Rating::Again),
        2 => Some(Rating::Hard),
        3 => Some(Rating::Good),
        4 => Some(Rating::Easy),
        _ => None,
    }
}

/// The oldest `limit` due cards at `now`, and how many are due in total.
pub fn queue(t: &dyn TrackerStore, now: i64, limit: usize) -> StoreResult<(Vec<FlashCard>, usize)> {
    let mut due = due_cards(t, now)?;
    let total = due.len();
    due.truncate(limit);
    Ok((due, total))
}

/// Cards in `state` whose front or options contain `query` (case-insensitive), soonest due first.
pub fn browse(
    t: &dyn TrackerStore,
    state: Option<State>,
    query: Option<&str>,
) -> StoreResult<Vec<FlashCard>> {
    let q = query.map(str::to_lowercase).filter(|q| !q.is_empty());
    let mut cards: Vec<FlashCard> = t
        .cards()?
        .into_iter()
        .filter(|c| state.is_none_or(|s| c.card.state == s))
        .filter(|c| {
            q.as_ref().is_none_or(|q| {
                c.front.to_lowercase().contains(q)
                    || c.options.iter().any(|o| o.to_lowercase().contains(q))
            })
        })
        .collect();
    cards.sort_by(|a, b| a.card.due.cmp(&b.card.due).then_with(|| a.id.cmp(&b.id)));
    Ok(cards)
}

/// A card's review log, oldest first.
pub fn history(t: &dyn TrackerStore, card_id: &str) -> StoreResult<Vec<FsrsReviewLog>> {
    let mut log: Vec<FsrsReviewLog> = t
        .reviews()?
        .into_iter()
        .filter(|r| r.card_id == card_id)
        .collect();
    log.sort_by_key(|r| r.ts);
    Ok(log)
}

/// What grading a card each way would schedule: `[again, hard, good, easy]`.
pub fn preview_entries(card: &fsrs::Card, now: i64) -> [(Rating, i64, String); 4] {
    let ivls = fsrs::preview(Some(card), now);
    let ratings = [Rating::Again, Rating::Hard, Rating::Good, Rating::Easy];
    std::array::from_fn(|i| (ratings[i], ivls[i], format_interval_ms(ivls[i])))
}

/// F32: the Close screen's batched date proposal for facts confirmed just now, before any card
/// for them exists yet — each from a brand-new card's own `Good` preview, so the first test
/// lands well within a day. The learner may change any of these; this is only the default.
pub fn propose_dates(items: &[String], now: i64) -> Vec<(String, i64)> {
    let good = fsrs::preview(None, now)[2]; // [again, hard, good, easy] offsets, 0-indexed
    items.iter().map(|id| (id.clone(), now + good)).collect()
}

/// The numbers on the review header.
#[derive(Clone, Debug, PartialEq)]
pub struct Stats {
    pub due_now: usize,
    /// Due by the end of the UTC day, overdue included.
    pub due_today: usize,
    /// Cards first reviewed today.
    pub new_today: usize,
    pub streak: u32,
    pub reviewed_today: usize,
    /// Share of reviews in the last 30 days (excluding each card's first) that were not `Again`.
    pub retention_30d: Option<f64>,
    /// Cards due per UTC day from today; day 0 includes everything overdue.
    pub forecast: [u32; FORECAST_DAYS],
    /// Start of the UTC day the forecast begins on (epoch ms).
    pub forecast_start: i64,
}

pub fn stats(t: &dyn TrackerStore, now: i64) -> StoreResult<Stats> {
    let cards = t.cards()?;
    let today = now.div_euclid(MS_DAY);
    let mut s = Stats {
        due_now: 0,
        due_today: 0,
        new_today: 0,
        streak: streak(t, now),
        reviewed_today: 0,
        retention_30d: None,
        forecast: [0; FORECAST_DAYS],
        forecast_start: today * MS_DAY,
    };
    for c in &cards {
        if c.card.due <= now {
            s.due_now += 1;
        }
        let day = c.card.due.div_euclid(MS_DAY);
        if day <= today {
            s.due_today += 1;
        }
        let ahead = usize::try_from((day - today).max(0)).unwrap_or(usize::MAX);
        if let Some(slot) = s.forecast.get_mut(ahead) {
            *slot += 1;
        }
    }
    let mut reviews = t.reviews()?;
    reviews.sort_by_key(|r| r.ts);
    let mut seen = std::collections::HashSet::new();
    let (mut kept, mut total) = (0usize, 0usize);
    for r in &reviews {
        let first = seen.insert(r.card_id.clone());
        let day = r.ts.div_euclid(MS_DAY);
        if day == today {
            s.reviewed_today += 1;
            if first {
                s.new_today += 1;
            }
        }
        if !first && r.ts > now - 30 * MS_DAY && r.ts <= now {
            total += 1;
            if r.rating != Rating::Again {
                kept += 1;
            }
        }
    }
    s.retention_30d = (total > 0).then(|| kept as f64 / total as f64);
    Ok(s)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use lp_core::fsrs::Card;
    use lp_store::SqliteTrackerStore;

    pub fn card(id: &str, due: i64) -> FlashCard {
        FlashCard {
            id: id.into(),
            front: "Which owns?".into(),
            options: ["a".into(), "b".into(), "c".into(), "d".into()],
            correct_index: 2,
            rationale: "because".into(),
            kind: Default::default(),
            must_name: Vec::new(),
            card: Card::new(due),
        }
    }

    #[test]
    fn grading_reschedules_logs_and_extends_streak() {
        let d = tempfile::tempdir().unwrap();
        let t = SqliteTrackerStore::open(d.path()).unwrap();
        t.upsert_card(&card("c1", 0)).unwrap();
        t.upsert_card(&card("c2", i64::MAX)).unwrap();
        let now = 10 * MS_DAY;
        assert_eq!(due_cards(&t, now).unwrap().len(), 1);
        assert_eq!(streak(&t, now), 0);

        grade(&t, "dev1", "c1", Rating::Good, now).unwrap();
        assert!(due_cards(&t, now).unwrap().is_empty());
        assert_eq!(t.reviews().unwrap().len(), 1);
        assert_eq!(streak(&t, now), 1);
        assert_eq!(streak(&t, now + MS_DAY), 1);
        assert_eq!(streak(&t, now + 2 * MS_DAY), 0);
        let sync = std::fs::read_to_string(d.path().join("sync-dev1.jsonl")).unwrap();
        assert!(sync.contains("\"kind\":\"review\"") && sync.contains("c1"));
        assert!(matches!(
            grade(&t, "dev1", "nope", Rating::Good, now),
            Err(StoreError::NotFound(_))
        ));
    }

    #[test]
    fn sync_merges_other_device_and_repairs_a_half_written_grade() {
        let d = tempfile::tempdir().unwrap();
        let (a, b) = (d.path().join("a"), d.path().join("b"));
        let (ta, tb) = (
            SqliteTrackerStore::open(&a).unwrap(),
            SqliteTrackerStore::open(&b).unwrap(),
        );
        for t in [&ta, &tb] {
            t.upsert_card(&card("c1", 0)).unwrap();
        }
        let now = 10 * MS_DAY;
        grade(&tb, "devb", "c1", Rating::Easy, now).unwrap();
        // device A receives B's log (Syncthing) and syncs.
        std::fs::copy(b.join("sync-devb.jsonl"), a.join("sync-devb.jsonl")).unwrap();
        sync(&ta, "deva").unwrap();
        assert_eq!(ta.reviews().unwrap().len(), 1);
        assert_eq!(ta.cards().unwrap()[0].card, tb.cards().unwrap()[0].card);
        // crash after the log write but before the card write
        ta.append_review(&FsrsReviewLog {
            card_id: "c1".into(),
            ts: now + MS_DAY,
            rating: Rating::Again,
            device_id: "deva".into(),
        })
        .unwrap();
        sync(&ta, "deva").unwrap();
        let want = replayed(&ta.reviews().unwrap(), "c1");
        assert_eq!(ta.cards().unwrap()[0].card, want);
        export_cards(&ta, "deva").unwrap();
        assert!(std::fs::read_to_string(a.join("sync-deva.jsonl"))
            .unwrap()
            .contains("\"kind\":\"card\""));
    }

    #[test]
    fn propose_dates_proposes_one_date_per_item_within_a_day() {
        let now = 1_767_258_000_000;
        let out = propose_dates(&["l00-i0".to_string(), "l00-i1".to_string()], now);
        assert_eq!(
            out.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
            vec!["l00-i0".to_string(), "l00-i1".to_string()]
        );
        for (_, due) in &out {
            assert!(*due > now && *due - now < MS_DAY);
        }
        assert_eq!(out[0].1, out[1].1);
    }

    #[test]
    fn interval_labels_and_preview_are_deterministic() {
        let now = 1_767_258_000_000;
        let ivls = fsrs::preview(Some(&Card::new(now)), now);
        assert_eq!(format_interval_ms(ivls[0]), "1m");
        assert_eq!(format_interval_ms(ivls[1]), "6m");
        assert_eq!(format_interval_ms(ivls[2]), "10m");
        assert!(ivls[3] >= MS_DAY);
        assert_eq!(format_interval_ms(0), "1m");
        assert_eq!(format_interval_ms(3 * 3_600_000), "3h");
        assert_eq!(format_interval_ms(5 * MS_DAY), "5d");
    }

    #[test]
    fn mastery_grows_with_review_state() {
        let mut c = Card::new(0);
        assert!((mastery(&c) - 0.15).abs() < f64::EPSILON);
        c.state = State::Review;
        c.stability = 1000.0;
        assert!((mastery(&c) - 0.98).abs() < f64::EPSILON);
        c.state = State::Learning;
        c.reps = 1;
        let m = mastery(&c);
        assert!((0.25..=0.65).contains(&m));
    }

    #[test]
    fn stats_count_due_new_reviewed_retention_and_forecast() {
        let d = tempfile::tempdir().unwrap();
        let t = SqliteTrackerStore::open(d.path()).unwrap();
        let now = 100 * MS_DAY + 3_600_000;
        // overdue, due later today, due in 3 days, due in 40 days (outside the forecast).
        for (id, due) in [
            ("c0", now - MS_DAY),
            ("c1", now + 3_600_000),
            ("c2", now + 3 * MS_DAY),
            ("c3", now + 40 * MS_DAY),
        ] {
            t.upsert_card(&card(id, due)).unwrap();
        }
        let s = stats(&t, now).unwrap();
        assert_eq!((s.due_now, s.due_today), (1, 2));
        assert_eq!(s.forecast[0], 2);
        assert_eq!(s.forecast[3], 1);
        assert_eq!(s.forecast.iter().sum::<u32>(), 3);
        assert_eq!(s.forecast_start, 100 * MS_DAY);
        assert_eq!((s.new_today, s.reviewed_today, s.streak), (0, 0, 0));
        assert_eq!(s.retention_30d, None);

        // c0: first review yesterday, then Again and Good today. c1: first review today.
        let log = |id: &str, ts: i64, rating: Rating| FsrsReviewLog {
            card_id: id.into(),
            ts,
            rating,
            device_id: "dev".into(),
        };
        for r in [
            log("c0", now - MS_DAY, Rating::Good),
            log("c0", now - 600_000, Rating::Again),
            log("c0", now - 300_000, Rating::Good),
            log("c1", now - 100_000, Rating::Easy),
        ] {
            t.append_review(&r).unwrap();
        }
        let s = stats(&t, now).unwrap();
        assert_eq!((s.reviewed_today, s.new_today, s.streak), (3, 1, 2));
        // Non-first reviews: Again, Good => 50%.
        assert_eq!(s.retention_30d, Some(0.5));
    }

    #[test]
    fn rating_suggestion_follows_correctness_pace_and_confidence() {
        assert_eq!(suggest_rating(false, Some(1), Some(5)), Rating::Again);
        assert_eq!(suggest_rating(true, None, None), Rating::Good);
        assert_eq!(suggest_rating(true, Some(3_000), None), Rating::Easy);
        assert_eq!(suggest_rating(true, Some(12_000), Some(4)), Rating::Good);
        assert_eq!(suggest_rating(true, Some(60_000), None), Rating::Hard);
        assert_eq!(suggest_rating(true, Some(1_000), Some(2)), Rating::Hard);
        assert_eq!(rating_from_int(3), Some(Rating::Good));
        assert_eq!(rating_from_int(0), None);
        assert_eq!(rating_from_int(5), None);
    }

    #[test]
    fn keys_browse_and_history() {
        assert_eq!((option_key(0), option_key(3)), ('a', 'd'));
        assert_eq!(option_index("C"), Some(2));
        assert_eq!(option_index("e"), None);
        assert_eq!(option_index("ab"), None);
        assert_eq!(parse_state("relearning"), Some(State::Relearning));
        assert_eq!(parse_state("old"), None);

        let d = tempfile::tempdir().unwrap();
        let t = SqliteTrackerStore::open(d.path()).unwrap();
        let mut other = card("c2", 5);
        other.front = "Borrow rules?".into();
        other.card.state = State::Review;
        t.upsert_card(&card("c1", 9)).unwrap();
        t.upsert_card(&other).unwrap();
        let ids = |v: Vec<FlashCard>| v.into_iter().map(|c| c.id.to_string()).collect::<Vec<_>>();
        assert_eq!(ids(browse(&t, None, None).unwrap()), ["c2", "c1"]);
        assert_eq!(ids(browse(&t, Some(State::Review), None).unwrap()), ["c2"]);
        assert_eq!(ids(browse(&t, None, Some("BORROW")).unwrap()), ["c2"]);
        assert!(browse(&t, None, Some("zzz")).unwrap().is_empty());

        grade(&t, "dev", "c1", Rating::Good, 1000).unwrap();
        grade(&t, "dev", "c1", Rating::Again, 2000).unwrap();
        let h = history(&t, "c1").unwrap();
        assert_eq!(h.iter().map(|r| r.ts).collect::<Vec<_>>(), [1000, 2000]);
        let (q, total) = queue(&t, 10, 1).unwrap();
        assert_eq!((q.len(), total), (1, 1));
        let p = preview_entries(&Card::new(0), 0);
        assert_eq!(p[0].0, Rating::Again);
        assert_eq!(p[0].2, "1m");
    }

    #[test]
    fn recall_cards_grade_by_rubric_or_exact_answer_only() {
        let mut c = card("r", 0);
        c.kind = lp_core::ports::CardKind::Recall;
        c.options = [
            "Krebs cycle".into(),
            String::new(),
            String::new(),
            String::new(),
        ];
        c.correct_index = 0;
        c.must_name = vec!["acetyl".into(), "citrate".into()];
        assert!(grade_recall(
            &c,
            "Acetyl-CoA joins oxaloacetate to make citrate"
        ));
        assert!(!grade_recall(&c, "acetyl only"));
        assert!(grade_recall(&c, "krebs cycle"), "exact answer also passes");
        assert!(!grade_recall(&c, ""));
        c.must_name.clear();
        assert!(
            !grade_recall(&c, "acetyl citrate"),
            "no rubric: never a rubric pass"
        );
    }
}
