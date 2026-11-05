use super::bout::Ready;
use super::schedule::SlotState;
use super::*;

const M: i64 = MS_PER_MINUTE;
const READY: Ready = Ready {
    phone_away: true,
    tabs_closed: true,
    focus_seconds: 30,
};

fn svc(d: &tempfile::TempDir) -> LearningService {
    LearningService::open(d.path())
}

/// One bout of `mins` minutes with a heartbeat each minute.
fn run_bout(s: &LearningService, start: i64, mins: i64) -> String {
    let id = s.start_bout(start, "c", READY).unwrap();
    for m in 1..=mins {
        s.heartbeat(start + m * M, &id).unwrap();
    }
    s.end_bout(start + mins * M, &id).unwrap();
    id
}

const T0: i64 = 10 * 3_600_000; // 10:00 UTC on day 0

#[test]
fn bout_inside_the_two_hour_gap_is_refused() {
    let d = tempfile::tempdir().unwrap();
    let s = svc(&d);
    run_bout(&s, T0, 30);
    let end = T0 + 30 * M;
    let err = s.start_bout(end + 60 * M, "c", READY).unwrap_err();
    assert_eq!(
        err,
        LearningError::Refused(Refusal::Gap {
            remaining_minutes: 60
        })
    );
    assert!(s.start_bout(end + 120 * M, "c", READY).is_ok());
}

#[test]
fn override_lifts_the_gap_once_and_is_logged() {
    let d = tempfile::tempdir().unwrap();
    let s = svc(&d);
    run_bout(&s, T0, 30);
    let now = T0 + 40 * M;
    assert!(s.start_bout(now, "c", READY).is_err());
    s.override_limits(now, "exam tomorrow").unwrap();
    assert_eq!(
        s.override_limits(now, "again"),
        Err(LearningError::OverrideUsed)
    );
    assert!(s.start_bout(now, "c", READY).is_ok());
    let logged = s
        .store
        .bouts()
        .unwrap()
        .iter()
        .filter(|e| matches!(e, lp_store::BoutEvent::Override { .. }))
        .count();
    assert_eq!(logged, 1);
    // A new day has a fresh override.
    assert!(s.override_limits(now + 24 * 60 * M, "x").is_ok());
}

#[test]
fn heartbeat_restores_the_clock_after_restart() {
    let d = tempfile::tempdir().unwrap();
    let s = svc(&d);
    let id = s.start_bout(T0, "c", READY).unwrap();
    for m in 1..=20 {
        s.heartbeat(T0 + m * M, &id).unwrap();
    }
    drop(s);
    let s2 = svc(&d);
    let c = s2.clock(T0 + 20 * M).unwrap();
    assert_eq!(c.bout.as_deref(), Some(id.as_str()));
    assert_eq!(
        c.status,
        lp_core::method::bout::BoutStatus::Active { active_minutes: 20 }
    );
    // Starting again returns the same open bout rather than a new one.
    assert_eq!(s2.start_bout(T0 + 21 * M, "c", READY).unwrap(), id);
}

#[test]
fn daily_limit_spans_bouts_and_ready_gate_is_required() {
    let d = tempfile::tempdir().unwrap();
    let s = svc(&d);
    s.set_setting("gap_between_bouts_minutes", 0).unwrap();
    s.set_setting("daily_limit_minutes", 60).unwrap();
    run_bout(&s, T0, 40);
    let now = T0 + 41 * M;
    let not_ready = Ready {
        phone_away: false,
        ..READY
    };
    assert_eq!(
        s.start_bout(now, "c", not_ready),
        Err(LearningError::Refused(Refusal::NotReady))
    );
    let id = s.start_bout(now, "c", READY).unwrap();
    let mut c = s.clock(now).unwrap();
    for m in 1..=25 {
        c = s.heartbeat(now + m * M, &id).unwrap();
    }
    assert_eq!(
        c.status,
        lp_core::method::bout::BoutStatus::DailyLimitReached
    );
    assert!(s.set_setting("nope", 1).is_err());
}

#[test]
fn checkin_schedule_and_profile() {
    let d = tempfile::tempdir().unwrap();
    let s = svc(&d);
    assert!(s.needs_checkin(T0).unwrap());
    assert!(s.record_checkin(T0, 5.0, "").unwrap());
    assert!(!s.needs_checkin(T0).unwrap());
    assert!(s.record_checkin(T0, 30.0, "").is_err());
    assert_eq!(s.sleep_summary(T0).unwrap().on_target_pct, Some(0));

    assert_eq!(
        s.suggest_slots("07:00").unwrap(),
        ["10:00", "13:30", "17:00"]
    );
    s.set_schedule("07:00", vec!["10:00".into(), "13:30".into()])
        .unwrap();
    assert!(s.set_schedule("7am", vec![]).is_err());
    run_bout(&s, T0 + 5 * M, 10);
    let rec = s.track_slots(T0 + 5 * 3_600_000, 1).unwrap();
    assert_eq!(rec[0].state, SlotState::Kept);
    assert_eq!(rec[1].state, SlotState::Missed);

    assert!(s.set_why(T0, "c", "  ").is_err());
    s.set_why(T0, "c", "to read Borges").unwrap();
    s.set_profile(T0, "pace", "slow").unwrap();
    s.set_profile(T0 + 1, "pace", "fast").unwrap();
    assert_eq!(s.why("c").unwrap().as_deref(), Some("to read Borges"));
    assert_eq!(s.profile().unwrap().len(), 1);
    assert_eq!(s.profile_packet().unwrap(), "- pace: fast\n");
}
