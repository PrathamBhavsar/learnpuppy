//! F08: the fact-check background job. Not gated by `check_action` — a claim's verdict never
//! blocks study. `method::next_step` already skips any lesson with a `Contradicted` claim, so
//! this job's only duty is to turn `Unverified` claims into a real verdict and save the result;
//! a contradicted claim then hides its lesson's items on the very next `next_step` call, with
//! no further wiring needed here.

use lp_agent::workbench::Web;
use lp_cli_driver::engine::LlmEngine;
use lp_core::course::{ClaimVerdict, Course};
use lp_core::ports::StoreResult;
use lp_store::FsCourseStore;

/// Verifies every still-`Unverified` claim in `course` in place. Returns whether anything
/// changed, so the caller only saves when it did.
pub async fn verify_course<E: LlmEngine, W: Web>(engine: &E, web: &W, course: &mut Course) -> bool {
    let mut changed = false;
    for lesson in &mut course.lessons {
        for claim in &mut lesson.claims {
            if claim.verdict != ClaimVerdict::Unverified {
                continue;
            }
            *claim = lp_agent::course::verify::verify_claim(engine, web, &claim.text).await;
            changed = true;
        }
    }
    changed
}

/// Runs the job for one stored course and saves it only if anything changed.
pub async fn run_verify_job<E: LlmEngine, W: Web>(
    store: &FsCourseStore,
    engine: &E,
    web: &W,
    id: &str,
) -> StoreResult<bool> {
    let mut course = store.load_course(id)?;
    let changed = verify_course(engine, web, &mut course).await;
    if changed {
        store.save_course(&course)?;
    }
    Ok(changed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use lp_agent::workbench::Hit;
    use lp_cli_driver::engine::FauxEngine;
    use lp_core::course::{Claim, Lesson};
    use lp_core::method::mastery::fold_mastery;
    use lp_core::method::next_step;
    use lp_core::method::Settings;
    use lp_core::method::Step;

    struct FauxWeb(Result<String, String>);

    #[async_trait]
    impl Web for FauxWeb {
        async fn search(&self, _q: &str) -> Result<Vec<Hit>, String> {
            Ok(Vec::new())
        }
        async fn fetch(&self, _url: &str) -> Result<String, String> {
            self.0.clone()
        }
    }

    fn course_with_a_claim() -> Course {
        Course {
            id: "c".into(),
            method_version: 1,
            plan_confirmed_at: Some(1),
            lessons: vec![Lesson {
                id: "l00".into(),
                claims: vec![Claim {
                    text: "The leader sends heartbeats.".into(),
                    verdict: ClaimVerdict::Unverified,
                    source: None,
                }],
                ..Lesson::default()
            }],
            ..Course::default()
        }
    }

    #[tokio::test]
    async fn an_unverified_claim_becomes_contradicted_and_hides_its_lesson() {
        let mut course = course_with_a_claim();
        let engine = FauxEngine::new();
        engine.faux_text(
            r#"{"source":"https://example.test/raft","quote":"the leader never sends heartbeats","verdict":"contradicted"}"#,
        );
        let web = FauxWeb(Ok(
            "The leader never sends heartbeats in this broken system.".into(),
        ));
        let changed = verify_course(&engine, &web, &mut course).await;
        assert!(changed);
        assert_eq!(
            course.lessons[0].claims[0].verdict,
            ClaimVerdict::Contradicted
        );

        // The gate's own next_step now skips the lesson outright: its items never appear.
        let mastery = fold_mastery(&course, &[]);
        assert_eq!(
            next_step(&course, &[], &mastery, &Settings::default(), &[], false),
            Step::Close
        );
    }

    #[tokio::test]
    async fn an_already_verified_claim_is_left_alone() {
        let mut course = course_with_a_claim();
        course.lessons[0].claims[0].verdict = ClaimVerdict::Confirmed;
        let engine = FauxEngine::new();
        let web = FauxWeb(Ok(String::new()));
        let changed = verify_course(&engine, &web, &mut course).await;
        assert!(!changed);
        assert_eq!(course.lessons[0].claims[0].verdict, ClaimVerdict::Confirmed);
    }

    #[tokio::test]
    async fn run_verify_job_saves_only_when_something_changed() {
        let d = tempfile::tempdir().unwrap();
        let store = FsCourseStore::new(d.path());
        store.save_course(&course_with_a_claim()).unwrap();
        let engine = FauxEngine::new();
        engine.faux_text(r#"{"source":"","quote":"","verdict":"confirmed"}"#);
        let web = FauxWeb(Ok(String::new()));
        let changed = run_verify_job(&store, &engine, &web, "c").await.unwrap();
        assert!(changed);
        let saved = store.load_course("c").unwrap();
        assert_eq!(saved.lessons[0].claims[0].verdict, ClaimVerdict::Unknown);
    }
}
