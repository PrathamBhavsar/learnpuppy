//! Decision digest: distilled decisions with their project, the due quiz cards, the streak, and
//! folding a decision into the knowledge graph.

use std::collections::HashSet;
use std::path::Path;
use std::sync::Arc;

use anyhow::anyhow;
use lp_agent::extract::{Pipeline, SourceMessage};
use lp_cli_driver::engine::LlmEngine;
use lp_core::ports::{
    DecisionBasis, DecisionRecord, GraphStore, SessionCheckpoint, StoreResult, TrackerStore,
};
use tokio_util::sync::CancellationToken;

use crate::review;

/// One distilled decision as the digest lists it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Decision {
    pub id: String,
    pub session_id: String,
    /// Last path component of the session's working directory, `-` when unknown.
    pub project: String,
    /// The source CLI (`claude`, `codex`, `agy`), from the `<source>:<id>` session id.
    pub tool: String,
    pub title: String,
    pub what: String,
    pub why: String,
    pub stated: bool,
    /// `option: rejected because`, one per alternative.
    pub alternatives: Vec<String>,
    pub risk: String,
    pub commit: String,
    /// Already folded into the knowledge graph.
    pub folded: bool,
}

/// A due four-choice card.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DueCard {
    pub id: String,
    pub question: String,
    pub options: [String; 4],
    pub correct: usize,
    pub rationale: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Digest {
    pub decisions: Vec<Decision>,
    /// Cards due now, in review order.
    pub due: Vec<DueCard>,
    pub streak: u32,
}

fn project(t: &dyn TrackerStore, session_id: &str) -> StoreResult<String> {
    Ok(t.session(session_id)?
        .and_then(|s| {
            let name = Path::new(&s.cwd).file_name()?;
            Some(name.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "-".into()))
}

/// The digest at `now`.
pub fn digest(g: &dyn GraphStore, t: &dyn TrackerStore, now: i64) -> StoreResult<Digest> {
    let folded: HashSet<_> = g.checkpoints()?.into_iter().map(|c| c.chat_id).collect();
    let decisions = t
        .decisions()?
        .into_iter()
        .map(|d| {
            Ok(Decision {
                project: project(t, &d.session_id)?,
                // Session ids are `<source>:<id>`.
                tool: d.session_id.split(':').next().unwrap_or_default().into(),
                session_id: d.session_id.to_string(),
                title: d.title,
                what: d.what,
                why: d.why,
                stated: d.basis == DecisionBasis::Stated,
                alternatives: d
                    .alternatives
                    .iter()
                    .map(|a| format!("{}: {}", a.option, a.rejected_because))
                    .collect(),
                risk: d.risk,
                commit: d.commit_sha.unwrap_or_default(),
                folded: folded.contains(&d.id),
                id: d.id.to_string(),
            })
        })
        .collect::<StoreResult<_>>()?;
    let due = review::due_cards(t, now)?
        .into_iter()
        .map(|c| DueCard {
            id: c.id.to_string(),
            question: c.front,
            options: c.options,
            correct: usize::from(c.correct_index),
            rationale: c.rationale,
        })
        .collect();
    Ok(Digest {
        decisions,
        due,
        streak: review::streak(t, now),
    })
}

/// The text the extractor reads for one decision.
pub fn decision_text(d: &DecisionRecord) -> String {
    let alts: Vec<String> = d
        .alternatives
        .iter()
        .map(|a| format!("{} (rejected: {})", a.option, a.rejected_because))
        .collect();
    format!(
        "{}\n\nWhat: {}\n\nWhy: {}\n\nAlternatives: {}\n\nRisk: {}",
        d.title,
        d.what,
        d.why,
        alts.join("; "),
        d.risk
    )
}

/// Folds one decision into the knowledge graph through Extractor -> Auditor -> Merger, then
/// records a checkpoint so the digest marks it folded.
pub async fn fold_decision<E, G>(
    engine: Arc<E>,
    graph: Arc<G>,
    t: &dyn TrackerStore,
    id: &str,
    now: i64,
    cancel: CancellationToken,
) -> anyhow::Result<()>
where
    E: LlmEngine + 'static,
    G: GraphStore + 'static,
{
    let d = t
        .decisions()?
        .into_iter()
        .find(|d| d.id == id)
        .ok_or_else(|| anyhow!("unknown decision {id}"))?;
    let msg = SourceMessage {
        id: "decision".into(),
        text: decision_text(&d),
    };
    Pipeline::new(engine, graph.clone())
        .extract(&d.id, &[msg], cancel)
        .await?;
    graph.append_checkpoint(&SessionCheckpoint {
        chat_id: d.id,
        folded_at: now / 1000,
    })?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::review::tests::card;
    use crate::stores::Stores;
    use lp_cli_driver::engine::FauxEngine;
    use lp_core::ports::{Alternative, CodingSession};
    use serde_json::json;

    fn decision(id: &str, session: &str) -> DecisionRecord {
        DecisionRecord {
            id: id.into(),
            session_id: session.into(),
            title: "Snapshot copy".into(),
            what: "w".into(),
            why: "y".into(),
            basis: DecisionBasis::Stated,
            alternatives: vec![Alternative {
                option: "open in place".into(),
                rejected_because: "WAL is live".into(),
            }],
            risk: String::new(),
            files: vec![],
            commit_sha: None,
        }
    }

    #[test]
    fn digest_shows_project_tool_due_cards_and_streak() {
        let d = tempfile::tempdir().unwrap();
        let s = Stores::open(d.path()).unwrap();
        let t = s.tracker.as_ref();
        t.upsert_session(&CodingSession {
            id: "claude:s1".into(),
            source: "claude".into(),
            cwd: "/home/me/code/tracker".into(),
            git_branch: None,
            started_at: 1,
            transcript: String::new(),
        })
        .unwrap();
        t.upsert_decision(&decision("d1", "claude:s1")).unwrap();
        t.upsert_decision(&decision("d2", "codex:gone")).unwrap();
        t.upsert_card(&card("c1", 0)).unwrap();
        t.upsert_card(&card("c2", i64::MAX)).unwrap();

        let dg = digest(s.graph.as_ref(), t, 1000).unwrap();
        let d1 = dg.decisions.iter().find(|d| d.id == "d1").unwrap();
        assert_eq!(
            (d1.project.as_str(), d1.tool.as_str()),
            ("tracker", "claude")
        );
        assert_eq!(d1.alternatives, ["open in place: WAL is live"]);
        assert!(d1.stated && !d1.folded);
        let d2 = dg.decisions.iter().find(|d| d.id == "d2").unwrap();
        assert_eq!((d2.project.as_str(), d2.tool.as_str()), ("-", "codex"));
        assert_eq!(dg.due.len(), 1);
        assert_eq!(dg.due[0].correct, 2);
        assert_eq!(dg.streak, 0);
    }

    #[test]
    fn decision_text_lists_alternatives() {
        let text = decision_text(&decision("d1", "claude:s1"));
        assert!(text.starts_with("Snapshot copy\n\nWhat: w"));
        assert!(text.contains("Alternatives: open in place (rejected: WAL is live)"));
    }

    #[tokio::test]
    async fn folding_extracts_then_marks_the_decision_folded() {
        let d = tempfile::tempdir().unwrap();
        let s = Stores::open(d.path()).unwrap();
        s.tracker
            .upsert_decision(&decision("d1", "claude:s1"))
            .unwrap();
        let faux = Arc::new(FauxEngine::new());
        faux.faux_structured(json!({"propositions": [{"id": "p1", "name": "Snapshot first", "statement": "Copy a live WAL db before reading it."}], "chains": []}));
        faux.faux_structured(json!({"links": []}));
        let cancel = CancellationToken::new();
        let unknown = fold_decision(
            faux.clone(),
            s.graph.clone(),
            s.tracker.as_ref(),
            "nope",
            0,
            cancel.clone(),
        );
        assert!(unknown.await.is_err());
        fold_decision(
            faux,
            s.graph.clone(),
            s.tracker.as_ref(),
            "d1",
            5000,
            cancel,
        )
        .await
        .unwrap();
        let dg = digest(s.graph.as_ref(), s.tracker.as_ref(), 0).unwrap();
        assert!(dg.decisions[0].folded);
        assert!(!s.graph.nodes().unwrap().is_empty());
    }
}
