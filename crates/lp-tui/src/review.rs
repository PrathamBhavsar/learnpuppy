//! FSRS grading and the due queue, shared by the Digest quiz and `learnpuppy review`.

use lp_core::fsrs::{self, Rating};
use lp_core::ports::{FlashCard, FsrsReviewLog, StoreError, StoreResult, TrackerStore};

/// Replication log name for this machine.
pub const DEVICE: &str = "local";
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

/// Reschedules `card_id`, then records the review in SQLite and the sync log.
pub fn grade(t: &dyn TrackerStore, card_id: &str, rating: Rating, now: i64) -> StoreResult<()> {
    let mut card = t
        .cards()?
        .into_iter()
        .find(|c| c.id == card_id)
        .ok_or_else(|| StoreError::NotFound(card_id.into()))?;
    card.card = fsrs::review(&card.card, now, rating);
    let log = FsrsReviewLog {
        card_id: card.id.clone(),
        ts: now,
        rating,
        device_id: DEVICE.into(),
    };
    t.commit_review(&card, &log)?;
    let mut line = serde_json::to_value(&log).map_err(|e| StoreError::Backend(e.to_string()))?;
    line["kind"] = "review".into();
    t.append_sync(DEVICE, &line)
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

        grade(&t, "c1", Rating::Good, now).unwrap();
        assert!(due_cards(&t, now).unwrap().is_empty());
        assert_eq!(t.reviews().unwrap().len(), 1);
        assert_eq!(streak(&t, now), 1);
        assert_eq!(streak(&t, now + MS_DAY), 1);
        assert_eq!(streak(&t, now + 2 * MS_DAY), 0);
        let sync = std::fs::read_to_string(d.path().join("sync-local.jsonl")).unwrap();
        assert!(sync.contains("\"kind\":\"review\"") && sync.contains("c1"));
        assert!(matches!(
            grade(&t, "nope", Rating::Good, now),
            Err(StoreError::NotFound(_))
        ));
    }
}
