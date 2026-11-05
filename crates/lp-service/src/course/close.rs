//! F29-F31: the Close screen's own numbers — a brain-dump diff, the close report, and the
//! session log. All code, no model call. FSRS card creation and the Knowledge Graph write are
//! M11's (`features/graph/`, `close.rs`'s own next layer); this only reports and logs what
//! happened in the course's stored event log.

use lp_core::course::{Course, CourseEvent};
use lp_core::method::mastery::{
    count_line, counts_for_lesson, fold_mastery, FactStatus, StatusCounts,
};
use lp_core::method::{next_step, Settings, Step};
use lp_store::FsCourseStore;

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

pub fn close_report(course: &Course, events: &[CourseEvent], settings: &Settings) -> CloseReport {
    let mastery = fold_mastery(course, events);
    let mut total = StatusCounts::default();
    let mut takeaways = Vec::new();
    let mut focus = Vec::new();
    for l in &course.lessons {
        let c = counts_for_lesson(&mastery, l);
        total.open += c.open;
        total.taught += c.taught;
        total.mc_passed += c.mc_passed;
        total.confirmed += c.confirmed;
        total.parked += c.parked;
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
}
