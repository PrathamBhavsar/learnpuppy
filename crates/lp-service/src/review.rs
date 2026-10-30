//! FSRS grading, the due queue, cross-device sync, streak and interval labels.

use lp_core::fsrs::{self, Rating, State};
use lp_core::ports::{FlashCard, FsrsReviewLog, StoreError, StoreResult, TrackerStore};
use lp_store::SqliteTrackerStore;

const MS_DAY: i64 = 86_400_000;

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
}
