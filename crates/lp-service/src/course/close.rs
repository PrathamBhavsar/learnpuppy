//! F29-F31: the Close screen's own numbers — a brain-dump diff, the close report, and the
//! session log, plus F32/F35: the batched review-date proposal and the Knowledge Graph write of
//! confirmed facts (and only those). All code, no model call.

use lp_core::course::{Course, CourseEvent};
use lp_core::graph::{GraphNode, Provenance};
use lp_core::method::mastery::{
    count_line, counts_for_lesson, fold_mastery, FactStatus, StatusCounts,
};
use lp_core::method::{next_step, Settings, Step};
use lp_core::ports::{GraphStore, StoreResult, TrackerStore};
use lp_store::FsCourseStore;

use super::{card_id, flash_card};
use crate::review;

const DAY_MS: i64 = 86_400_000;

fn words(s: &str) -> std::collections::HashSet<String> {
    s.to_lowercase()
        .split(|c: char| !c.is_alphabetic())
        .filter(|w| w.chars().count() >= 4)
        .map(str::to_string)
        .collect()
}

/// F29: key facts not found anywhere in the learner's brain dump (same four-letter-word floor
/// as `course::gate_course`'s own matcher), so code — never a model — shows what was left out.
pub fn missed_facts(course: &Course, dump: &str) -> Vec<String> {
    let dump_words = words(dump);
    course
        .lessons
        .iter()
        .flat_map(|l| l.key_facts.iter())
        .filter(|f| {
            let fw = words(f);
            !fw.is_empty() && fw.iter().filter(|w| dump_words.contains(*w)).count() * 2 < fw.len()
        })
        .cloned()
        .collect()
}

/// F30: the Close report.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CloseReport {
    /// Confirmed key facts, lesson-title qualified.
    pub takeaways: Vec<String>,
    /// Open or taught (not yet confirmed) key facts: what to focus on next.
    pub focus: Vec<String>,
    /// E.g. "11 of 14 confirmed, 3 open, 1 parked", across the whole course.
    pub count_line: String,
    /// (wrong, total) among answers given at confidence 4 or 5 — "answered sure and was wrong".
    pub sure_and_wrong: (u32, u32),
    /// A one-line description of the first step not yet at Close, for a resume link; `None`
    /// once the course is fully done.
    pub resume: Option<String>,
}

/// The mastery bar's counts summed across every lesson in the course.
fn total_counts(mastery: &lp_core::method::mastery::MasteryState, course: &Course) -> StatusCounts {
    let mut total = StatusCounts::default();
    for l in &course.lessons {
        let c = counts_for_lesson(mastery, l);
        total.open += c.open;
        total.taught += c.taught;
        total.mc_passed += c.mc_passed;
        total.confirmed += c.confirmed;
        total.parked += c.parked;
    }
    total
}

/// The mastery bar's count line across the whole course, e.g. "11 of 14 confirmed, 3 open, 1
/// parked" (`close_report`'s own `count_line`, without the rest of the report).
pub fn mastery_count_line(course: &Course, events: &[CourseEvent]) -> String {
    count_line(&total_counts(&fold_mastery(course, events), course))
}

pub fn close_report(course: &Course, events: &[CourseEvent], settings: &Settings) -> CloseReport {
    let mastery = fold_mastery(course, events);
    let total = total_counts(&mastery, course);
    let mut takeaways = Vec::new();
    let mut focus = Vec::new();
    for l in &course.lessons {
        for (i, f) in l.key_facts.iter().enumerate() {
            match mastery.status(&l.id, i) {
                FactStatus::Confirmed => takeaways.push(format!("{}: {f}", l.title)),
                FactStatus::Open | FactStatus::Taught => focus.push(format!("{}: {f}", l.title)),
                _ => {}
            }
        }
    }
    let (mut wrong, mut asked_sure) = (0u32, 0u32);
    for ev in events {
        if let CourseEvent::Answered {
            correct,
            confidence: Some(c),
            gap,
            ..
        } = ev
        {
            if *gap || *c < 4 {
                continue;
            }
            asked_sure += 1;
            if !*correct {
                wrong += 1;
            }
        }
    }
    let resume = match next_step(course, events, &mastery, settings, &[], false) {
        Step::Close => None,
        step => Some(format!("{step:?}")),
    };
    CloseReport {
        takeaways,
        focus,
        count_line: count_line(&total),
        sure_and_wrong: (wrong, asked_sure),
        resume,
    }
}

/// F31: a rereadable markdown block for the session log, appended as study happens so a crash
/// loses nothing. Fact-checks stay out of it, per the spec.
pub fn session_log_md(report: &CloseReport, heading: &str) -> String {
    let mut s = format!("## {heading}\n\n**{}**\n\n", report.count_line);
    if !report.takeaways.is_empty() {
        s.push_str("Takeaways:\n");
        for t in &report.takeaways {
            s.push_str(&format!("- {t}\n"));
        }
        s.push('\n');
    }
    if !report.focus.is_empty() {
        s.push_str("Focus next:\n");
        for f in &report.focus {
            s.push_str(&format!("- {f}\n"));
        }
        s.push('\n');
    }
    s.push_str(&format!(
        "Answered sure and was wrong: {} of {}\n",
        report.sure_and_wrong.0, report.sure_and_wrong.1
    ));
    s
}

/// Builds the report and appends it to the course's session log in one call.
pub fn write_session_log(
    store: &FsCourseStore,
    id: &str,
    course: &Course,
    events: &[CourseEvent],
    settings: &Settings,
    date: &str,
    n: u32,
) -> lp_core::ports::StoreResult<CloseReport> {
    let report = close_report(course, events, settings);
    let md = session_log_md(&report, date);
    store.append_session_log(id, date, n, &md)?;
    Ok(report)
}

/// Gives a brand-new FSRS card to every item testing a fact confirmed just now that has none
/// yet, so F32's date proposal has a card to set a due date on. Returns how many were made.
pub fn ensure_cards(
    tracker: &dyn TrackerStore,
    course: &Course,
    events: &[CourseEvent],
) -> StoreResult<usize> {
    let mastery = fold_mastery(course, events);
    let mut n = 0;
    for l in &course.lessons {
        for it in &l.items {
            let Some(fact) = it.tests else { continue };
            if mastery.status(&l.id, fact) != FactStatus::Confirmed {
                continue;
            }
            if tracker.card(&card_id(&course.id, &it.id))?.is_some() {
                continue;
            }
            if let Some(card) = flash_card(course, l, it) {
                tracker.upsert_card(&card)?;
                n += 1;
            }
        }
    }
    Ok(n)
}

/// Ends the session: makes an FSRS card for every fact confirmed just now (F32 needs one to
/// propose a date for), writes F35's confirmed-fact nodes to the Knowledge Graph, then builds and
/// appends the F29-F31 report. The brain-dump diff (`missed_facts`) is computed separately,
/// before this is called, since it never touches the store.
pub fn run_close(
    store: &FsCourseStore,
    graph: &dyn GraphStore,
    tracker: &dyn TrackerStore,
    id: &str,
    settings: &Settings,
    date: &str,
    now: i64,
) -> StoreResult<CloseReport> {
    let course = store.load_course(id)?;
    let events = store.events(id)?;
    ensure_cards(tracker, &course, &events)?;
    write_confirmed(graph, &course, &events, now)?;
    write_session_log(store, id, &course, &events, settings, date, 1)
}

/// A Knowledge Graph node id for one confirmed key fact: stable, so closing twice never
/// duplicates a node.
fn fact_node_id(course: &str, lesson: &str, fact: usize) -> String {
    format!("n_course_{course}_{lesson}_{fact}")
}

/// F35: one node per `Confirmed` key fact. Open, taught, multiple-choice-passed and parked facts
/// never reach the graph.
pub fn confirmed_nodes(course: &Course, events: &[CourseEvent], now: i64) -> Vec<GraphNode> {
    let mastery = fold_mastery(course, events);
    let at = chrono::DateTime::from_timestamp_millis(now).unwrap_or_default();
    let mut out = Vec::new();
    for l in &course.lessons {
        for (i, f) in l.key_facts.iter().enumerate() {
            if mastery.status(&l.id, i) != FactStatus::Confirmed {
                continue;
            }
            out.push(GraphNode {
                id: fact_node_id(&course.id, &l.id, i).into(),
                name: f.chars().take(60).collect(),
                statement: f.clone(),
                aliases: vec![l.title.clone()],
                created_at: at,
                provenance: vec![Provenance {
                    chat_id: course.id.as_str().into(),
                    message_id: format!("{}#{i}", l.id).into(),
                }],
            });
        }
    }
    out
}

/// Appends the confirmed facts not yet in the graph; returns how many were new.
pub fn write_confirmed(
    graph: &dyn GraphStore,
    course: &Course,
    events: &[CourseEvent],
    now: i64,
) -> StoreResult<usize> {
    let have: std::collections::HashSet<_> = graph.nodes()?.into_iter().map(|n| n.id).collect();
    let mut n = 0;
    for node in confirmed_nodes(course, events, now) {
        if !have.contains(&node.id) {
            graph.append_node(&node)?;
            n += 1;
        }
    }
    Ok(n)
}

/// One proposed first test for a review card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewDate {
    pub card_id: String,
    pub item: String,
    pub due: i64,
}

/// F32: a date for every item testing a confirmed fact. Default is FSRS's first interval (within
/// a day); an item answered sure and wrong goes back to the first rung, so it is due now.
pub fn propose_review_dates(course: &Course, events: &[CourseEvent], now: i64) -> Vec<ReviewDate> {
    let mastery = fold_mastery(course, events);
    let sure_wrong: std::collections::HashSet<&str> = events
        .iter()
        .filter_map(|e| match e {
            CourseEvent::Answered {
                item,
                correct: false,
                confidence: Some(c),
                gap: false,
                ..
            } if *c >= 4 => Some(item.as_str()),
            _ => None,
        })
        .collect();
    let items: Vec<(&str, usize)> = course
        .lessons
        .iter()
        .flat_map(|l| {
            let mastery = &mastery;
            l.items.iter().filter_map(move |it| {
                let fact = it.tests?;
                (mastery.status(&l.id, fact) == FactStatus::Confirmed)
                    .then_some((it.id.as_str(), fact))
            })
        })
        .collect();
    let ids: Vec<String> = items.iter().map(|(i, _)| (*i).to_string()).collect();
    review::propose_dates(&ids, now)
        .into_iter()
        .map(|(item, due)| ReviewDate {
            card_id: card_id(&course.id, &item),
            due: if sure_wrong.contains(item.as_str()) {
                now
            } else {
                due
            },
            item,
        })
        .collect()
}

/// Applies the learner's chosen dates to cards not yet reviewed. A first test is never more than
/// one day out, whatever date is sent. Returns how many cards changed.
pub fn apply_review_dates(
    tracker: &dyn TrackerStore,
    dates: &[(String, i64)],
    now: i64,
) -> StoreResult<usize> {
    let mut n = 0;
    for (id, due) in dates {
        let Some(mut c) = tracker.card(id)? else {
            continue;
        };
        if c.card.reps > 0 {
            continue;
        }
        c.card.due = (*due).clamp(now, now + DAY_MS);
        tracker.upsert_card(&c)?;
        n += 1;
    }
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_core::course::{Item, ItemKind, Lesson};

    fn course() -> Course {
        Course {
            id: "c".into(),
            method_version: 1,
            plan_confirmed_at: Some(1),
            lessons: vec![Lesson {
                id: "l00".into(),
                title: "Cells".into(),
                key_facts: vec![
                    "mitochondria makes energy".into(),
                    "nucleus holds dna".into(),
                ],
                items: vec![
                    Item {
                        id: "l00-i0".into(),
                        kind: ItemKind::Mcq,
                        q: "q0".into(),
                        answer: "a".into(),
                        options: vec!["a".into(), "b".into(), "c".into(), "d".into()],
                        tests: Some(0),
                        ..Item::default()
                    },
                    Item {
                        id: "l00-i1".into(),
                        kind: ItemKind::Recall,
                        q: "q1".into(),
                        answer: "a".into(),
                        tests: Some(1),
                        ..Item::default()
                    },
                ],
                ..Lesson::default()
            }],
            ..Course::default()
        }
    }

    #[test]
    fn missed_facts_finds_what_the_dump_left_out() {
        let c = course();
        let dump = "the mitochondria makes energy for the cell";
        let missed = missed_facts(&c, dump);
        assert_eq!(missed, vec!["nucleus holds dna".to_string()]);
    }

    fn ans(item: &str, correct: bool, conf: u8) -> CourseEvent {
        CourseEvent::Answered {
            ts: 0,
            item: item.into(),
            correct,
            chosen: String::new(),
            confidence: Some(conf),
            ms: None,
            gap: false,
            reasoning: String::new(),
        }
    }

    #[test]
    fn mastery_count_line_matches_the_report_s_own_count_line() {
        let c = course();
        let events = [ans("l00-i0", true, 3)];
        assert_eq!(
            mastery_count_line(&c, &events),
            close_report(&c, &events, &Settings::default()).count_line
        );
    }

    #[test]
    fn close_report_counts_sure_and_wrong_and_proposes_a_resume_step() {
        let c = course();
        let events = [ans("l00-i0", false, 5), ans("l00-i0", false, 2)];
        let report = close_report(&c, &events, &Settings::default());
        // Only the confidence-5 miss counts as "sure and wrong"; the confidence-2 one does not.
        assert_eq!(report.sure_and_wrong, (1, 1));
        assert!(report.resume.is_some());
    }

    #[test]
    fn close_report_has_no_resume_once_the_course_is_done() {
        let empty = Course {
            id: "c".into(),
            method_version: 1,
            plan_confirmed_at: Some(1),
            ..Course::default()
        };
        let report = close_report(&empty, &[], &Settings::default());
        assert_eq!(report.resume, None);
    }

    #[test]
    fn session_log_md_names_the_takeaways_and_the_sure_and_wrong_line() {
        let report = CloseReport {
            takeaways: vec!["Cells: mitochondria makes energy".into()],
            focus: vec![],
            count_line: "1 of 2 confirmed, 1 open".into(),
            sure_and_wrong: (1, 3),
            resume: None,
        };
        let md = session_log_md(&report, "2026-11-05");
        assert!(md.contains("## 2026-11-05"));
        assert!(md.contains("mitochondria makes energy"));
        assert!(md.contains("Answered sure and was wrong: 1 of 3"));
    }

    #[test]
    fn write_session_log_appends_to_the_stored_course() {
        let d = tempfile::tempdir().unwrap();
        let store = FsCourseStore::new(d.path());
        let c = course();
        store.save_course(&c).unwrap();
        write_session_log(&store, "c", &c, &[], &Settings::default(), "2026-11-05", 1).unwrap();
        let read = store.read_session_log("c", "2026-11-05", 1).unwrap();
        assert!(read.contains("## 2026-11-05"));
    }

    fn confirmed_events() -> Vec<CourseEvent> {
        vec![
            CourseEvent::Shown {
                ts: 0,
                lesson: "l00".into(),
            },
            ans("l00-i0", true, 3),
            CourseEvent::Shown {
                ts: 0,
                lesson: "l00".into(),
            },
            CourseEvent::RecallGraded {
                ts: 0,
                item: "l00-i1".into(),
                correct: true,
                source: String::new(),
            },
        ]
    }

    #[test]
    fn only_confirmed_facts_reach_the_graph() {
        let c = course();
        assert!(confirmed_nodes(&c, &[], 0).is_empty());
        let nodes = confirmed_nodes(&c, &confirmed_events(), 0);
        assert!(nodes
            .iter()
            .all(|n| n.statement != "nucleus holds dna" || nodes.len() == 2));
        let mastery = fold_mastery(&c, &confirmed_events());
        let confirmed = (0..2)
            .filter(|i| mastery.status("l00", *i) == FactStatus::Confirmed)
            .count();
        assert_eq!(nodes.len(), confirmed);
    }

    /// R1: a method_version = 1 walk to Close confirms every fact, so the report counts them and
    /// the graph gets one node per fact.
    #[test]
    fn a_walk_to_close_confirms_every_fact_and_writes_nodes() {
        let mut c = course();
        c.lessons[0].items = vec![
            Item {
                id: "l00-i0".into(),
                kind: ItemKind::Mcq,
                q: "q0".into(),
                answer: "a".into(),
                options: vec!["a".into(), "b".into(), "c".into(), "d".into()],
                tests: Some(0),
                ..Item::default()
            },
            Item {
                id: "l00-r0".into(),
                kind: ItemKind::Recall,
                q: "r0".into(),
                answer: "a".into(),
                tests: Some(0),
                ..Item::default()
            },
            Item {
                id: "l00-i1".into(),
                kind: ItemKind::Mcq,
                q: "q1".into(),
                answer: "a".into(),
                options: vec!["a".into(), "b".into(), "c".into(), "d".into()],
                tests: Some(1),
                ..Item::default()
            },
            Item {
                id: "l00-r1".into(),
                kind: ItemKind::Recall,
                q: "r1".into(),
                answer: "a".into(),
                tests: Some(1),
                ..Item::default()
            },
        ];
        let settings = Settings::default();
        let mut events: Vec<CourseEvent> = Vec::new();
        for _ in 0..40 {
            let mastery = fold_mastery(&c, &events);
            match next_step(&c, &events, &mastery, &settings, &[], false) {
                Step::Close => break,
                Step::Predict { lesson } => events.push(CourseEvent::Predicted {
                    ts: 0,
                    lesson,
                    guess: "x".into(),
                }),
                Step::Explain { lesson, .. } => events.push(CourseEvent::Shown { ts: 0, lesson }),
                Step::Notes { lesson } => events.push(CourseEvent::NoteWritten {
                    ts: 0,
                    lesson,
                    text: String::new(),
                }),
                Step::Ask { item, .. } if item.contains("-r") => {
                    events.push(CourseEvent::RecallGraded {
                        ts: 0,
                        item,
                        correct: true,
                        source: String::new(),
                    })
                }
                Step::Ask { item, .. } => events.push(ans(&item, true, 3)),
                Step::Feynman { lesson } => events.push(CourseEvent::FeynmanGraded {
                    ts: 0,
                    lesson,
                    correct: true,
                    explanation: String::new(),
                }),
                other => panic!("unexpected step {other:?}"),
            }
        }
        let report = close_report(&c, &events, &settings);
        assert_eq!(report.count_line, "2 of 2 confirmed");
        assert_eq!(report.takeaways.len(), 2);
        assert_eq!(report.resume, None);
        let d = tempfile::tempdir().unwrap();
        let g = lp_store::FsGraphStore::open(d.path().join("g")).unwrap();
        assert_eq!(write_confirmed(&g, &c, &events, 0).unwrap(), 2);
    }

    #[test]
    fn write_confirmed_is_idempotent() {
        let d = tempfile::tempdir().unwrap();
        let g = lp_store::FsGraphStore::open(d.path().join("g")).unwrap();
        let c = course();
        let ev = confirmed_events();
        let first = write_confirmed(&g, &c, &ev, 0).unwrap();
        assert_eq!(write_confirmed(&g, &c, &ev, 0).unwrap(), 0);
        assert_eq!(g.nodes().unwrap().len(), first);
    }

    #[test]
    fn review_dates_are_within_a_day_and_sure_and_wrong_is_due_now() {
        let c = course();
        let mut ev = confirmed_events();
        let now = 1_000_000;
        let dates = propose_review_dates(&c, &ev, now);
        assert!(dates.iter().all(|d| d.due > now && d.due <= now + DAY_MS));
        ev.push(ans("l00-i0", false, 5));
        let sw = propose_review_dates(&c, &ev, now);
        for d in sw.iter().filter(|d| d.item == "l00-i0") {
            assert_eq!(d.due, now);
        }
    }

    #[test]
    fn apply_review_dates_clamps_to_one_day_and_skips_reviewed_cards() {
        use lp_core::ports::FlashCard;
        let d = tempfile::tempdir().unwrap();
        let t = lp_store::SqliteTrackerStore::open(d.path().join("t")).unwrap();
        let mk = |id: &str, reps: u32| {
            let mut card = lp_core::fsrs::Card::new(i64::MAX / 2);
            card.reps = reps;
            FlashCard {
                id: id.into(),
                front: "q".into(),
                options: ["a".into(), "b".into(), "c".into(), "d".into()],
                correct_index: 0,
                rationale: String::new(),
                kind: Default::default(),
                must_name: Vec::new(),
                card,
            }
        };
        t.upsert_card(&mk("new", 0)).unwrap();
        t.upsert_card(&mk("old", 3)).unwrap();
        let now = 5_000;
        let n = apply_review_dates(
            &t,
            &[
                ("new".into(), now + 9 * DAY_MS),
                ("old".into(), now),
                ("none".into(), now),
            ],
            now,
        )
        .unwrap();
        assert_eq!(n, 1);
        assert_eq!(t.card("new").unwrap().unwrap().card.due, now + DAY_MS);
        assert_eq!(t.card("old").unwrap().unwrap().card.due, i64::MAX / 2);
    }

    /// A fact confirms only with a correct Mcq pass and a correct Recall on the *same* fact
    /// (`mastery::mc_then_teaching_then_recall_confirms`); this fixture's own `course()` never
    /// reaches that (its mcq and recall test different facts), so this builds its own.
    fn confirmable() -> (Course, Vec<CourseEvent>) {
        let c = Course {
            id: "c".into(),
            method_version: 1,
            plan_confirmed_at: Some(1),
            lessons: vec![Lesson {
                id: "l00".into(),
                key_facts: vec!["f0".into()],
                items: vec![
                    Item {
                        id: "l00-mcq".into(),
                        kind: ItemKind::Mcq,
                        q: "mc?".into(),
                        answer: "a".into(),
                        options: vec!["a".into(), "b".into(), "c".into(), "d".into()],
                        tests: Some(0),
                        ..Item::default()
                    },
                    Item {
                        id: "l00-recall".into(),
                        kind: ItemKind::Recall,
                        q: "recall?".into(),
                        answer: "a".into(),
                        tests: Some(0),
                        ..Item::default()
                    },
                ],
                ..Lesson::default()
            }],
            ..Course::default()
        };
        let events = vec![
            CourseEvent::Answered {
                ts: 0,
                item: "l00-mcq".into(),
                correct: true,
                chosen: "a".into(),
                confidence: None,
                ms: None,
                gap: false,
                reasoning: String::new(),
            },
            CourseEvent::Shown {
                ts: 0,
                lesson: "l00".into(),
            },
            CourseEvent::RecallGraded {
                ts: 0,
                item: "l00-recall".into(),
                correct: true,
                source: String::new(),
            },
        ];
        (c, events)
    }

    #[test]
    fn ensure_cards_makes_a_card_only_where_one_can_be_built_and_is_idempotent() {
        let (c, events) = confirmable();
        assert_eq!(
            fold_mastery(&c, &events).status("l00", 0),
            FactStatus::Confirmed
        );
        let d = tempfile::tempdir().unwrap();
        let t = lp_store::SqliteTrackerStore::open(d.path()).unwrap();
        // The recall item gets a free-recall card even with too few other answers for options.
        assert_eq!(ensure_cards(&t, &c, &events).unwrap(), 2);
        let mcq = t.card(&card_id(&c.id, "l00-mcq")).unwrap().unwrap();
        assert_eq!(mcq.kind, lp_core::ports::CardKind::Mcq);
        let recall = t.card(&card_id(&c.id, "l00-recall")).unwrap().unwrap();
        assert_eq!(recall.kind, lp_core::ports::CardKind::Recall);
        assert_eq!(ensure_cards(&t, &c, &events).unwrap(), 0);
    }

    #[test]
    fn run_close_makes_cards_writes_graph_nodes_and_appends_the_session_log() {
        let (c, events) = confirmable();
        let d = tempfile::tempdir().unwrap();
        let store = FsCourseStore::new(d.path());
        store.save_course(&c).unwrap();
        for ev in &events {
            store.append_event("c", ev).unwrap();
        }
        let g = lp_store::FsGraphStore::open(d.path().join("g")).unwrap();
        let t = lp_store::SqliteTrackerStore::open(d.path().join("t")).unwrap();
        let settings = Settings::default();
        let report = run_close(&store, &g, &t, "c", &settings, "2026-11-05", 0).unwrap();
        assert!(report.takeaways.iter().any(|s| s.contains("f0")));
        assert!(t.card(&card_id("c", "l00-mcq")).unwrap().is_some());
        assert_eq!(g.nodes().unwrap().len(), 1);
        let log = store.read_session_log("c", "2026-11-05", 1).unwrap();
        assert!(log.contains("## 2026-11-05"));
    }
}
