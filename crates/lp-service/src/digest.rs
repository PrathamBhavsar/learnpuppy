//! Decision digest: distilled decisions with their project, the due quiz cards, the streak, and
//! folding a decision into the knowledge graph.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::anyhow;
use lp_agent::extract::{Pipeline, SourceMessage};
use lp_cli_driver::engine::LlmEngine;
use lp_core::ports::{
    CodingSession, DecisionBasis, DecisionRecord, GraphStore, SessionCheckpoint, StoreError,
    StoreResult, TrackerStore,
};
use serde::{Deserialize, Serialize};
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

/// Which cards were generated from which decision. The tracker keeps cards and decisions in
/// separate tables, so the link lives next to them in `tracker/decision-cards.json`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardLinks {
    pub by_decision: BTreeMap<String, Vec<String>>,
}

pub const LINKS_FILE: &str = "decision-cards.json";

impl CardLinks {
    fn file(tracker_dir: &Path) -> PathBuf {
        tracker_dir.join(LINKS_FILE)
    }

    /// The links under `tracker_dir`; none when the file is missing or unreadable.
    pub fn read(tracker_dir: &Path) -> Self {
        std::fs::read(Self::file(tracker_dir))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    /// Atomic write.
    pub fn save(&self, tracker_dir: &Path) -> anyhow::Result<()> {
        std::fs::create_dir_all(tracker_dir)?;
        let path = Self::file(tracker_dir);
        let tmp = path.with_extension(format!("tmp.{}", std::process::id()));
        std::fs::write(&tmp, serde_json::to_vec_pretty(self)?)?;
        std::fs::rename(&tmp, &path)?;
        Ok(())
    }

    /// Card ids generated from `decision_id`, in creation order.
    pub fn cards_of(&self, decision_id: &str) -> &[String] {
        self.by_decision.get(decision_id).map_or(&[], Vec::as_slice)
    }

    /// The decision a card was generated from.
    pub fn decision_of(&self, card_id: &str) -> Option<&str> {
        self.by_decision
            .iter()
            .find(|(_, cards)| cards.iter().any(|c| c == card_id))
            .map(|(d, _)| d.as_str())
    }

    /// Adds `cards` to `decision_id`, keeping order and skipping ones already linked.
    pub fn link(&mut self, decision_id: &str, cards: impl IntoIterator<Item = String>) {
        let list = self.by_decision.entry(decision_id.to_owned()).or_default();
        for c in cards {
            if !list.contains(&c) {
                list.push(c);
            }
        }
    }
}

/// Which decisions the digest lists.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Filter {
    /// Keep sessions started at or after this (epoch ms).
    pub from: Option<i64>,
    /// Keep sessions started before this (epoch ms).
    pub to: Option<i64>,
    /// Source CLIs (`claude`, `codex`, `agy`); empty keeps all.
    pub sources: Vec<String>,
}

/// One decision with whether it is already folded into the graph.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DecisionRow {
    pub record: DecisionRecord,
    pub folded: bool,
}

/// The decisions of one coding session.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionGroup {
    pub session_id: String,
    /// Source CLI, from the `<source>:<id>` session id.
    pub source: String,
    /// Last path component of the session's working directory, `-` when unknown.
    pub project: String,
    pub started_at: Option<i64>,
    pub decisions: Vec<DecisionRow>,
}

/// Sessions that started on one UTC day.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DayGroup {
    /// `YYYY-MM-DD` (UTC), or [`UNDATED`] for decisions whose session is unknown.
    pub day: String,
    pub sessions: Vec<SessionGroup>,
}

pub const UNDATED: &str = "undated";

/// The UTC day of an epoch-ms timestamp.
pub fn day_of(ms: i64) -> String {
    chrono::DateTime::from_timestamp_millis(ms)
        .unwrap_or(chrono::DateTime::UNIX_EPOCH)
        .format("%Y-%m-%d")
        .to_string()
}

fn project_of(session: Option<&CodingSession>) -> String {
    session
        .and_then(|s| {
            Path::new(&s.cwd)
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
        })
        .unwrap_or_else(|| "-".into())
}

fn source_of(session_id: &str) -> String {
    session_id.split(':').next().unwrap_or_default().to_owned()
}

/// Decisions grouped by day, then session, newest first. A decision whose session was never
/// stored lands under [`UNDATED`] and is dropped by any date filter.
pub fn grouped(g: &dyn GraphStore, t: &dyn TrackerStore, f: &Filter) -> StoreResult<Vec<DayGroup>> {
    let folded: HashSet<_> = g.checkpoints()?.into_iter().map(|c| c.chat_id).collect();
    let dated = f.from.is_some() || f.to.is_some();
    let mut sessions: BTreeMap<String, SessionGroup> = BTreeMap::new();
    for d in t.decisions()? {
        let session_id = d.session_id.to_string();
        let source = source_of(&session_id);
        if !f.sources.is_empty() && !f.sources.contains(&source) {
            continue;
        }
        if !sessions.contains_key(&session_id) {
            let s = t.session(&session_id)?;
            let started_at = s.as_ref().map(|s| s.started_at);
            let keep = match started_at {
                Some(at) => f.from.is_none_or(|from| at >= from) && f.to.is_none_or(|to| at < to),
                None => !dated,
            };
            if !keep {
                continue;
            }
            sessions.insert(
                session_id.clone(),
                SessionGroup {
                    project: project_of(s.as_ref()),
                    session_id: session_id.clone(),
                    source,
                    started_at,
                    decisions: vec![],
                },
            );
        }
        if let Some(group) = sessions.get_mut(&session_id) {
            group.decisions.push(DecisionRow {
                folded: folded.contains(&d.id),
                record: d,
            });
        }
    }
    let mut days: BTreeMap<String, Vec<SessionGroup>> = BTreeMap::new();
    for s in sessions.into_values() {
        let day = s.started_at.map_or_else(|| UNDATED.to_owned(), day_of);
        days.entry(day).or_default().push(s);
    }
    let mut out: Vec<DayGroup> = days
        .into_iter()
        .map(|(day, mut sessions)| {
            sessions.sort_by(|a, b| {
                b.started_at
                    .cmp(&a.started_at)
                    .then_with(|| a.session_id.cmp(&b.session_id))
            });
            DayGroup { day, sessions }
        })
        .collect();
    // Newest day first; undated last.
    out.sort_by(|a, b| match (a.day == UNDATED, b.day == UNDATED) {
        (true, true) => std::cmp::Ordering::Equal,
        (true, false) => std::cmp::Ordering::Greater,
        (false, true) => std::cmp::Ordering::Less,
        (false, false) => b.day.cmp(&a.day),
    });
    Ok(out)
}

/// One decision with its session (when stored) and fold state.
pub fn decision(
    g: &dyn GraphStore,
    t: &dyn TrackerStore,
    id: &str,
) -> StoreResult<(DecisionRow, Option<CodingSession>)> {
    let record = t
        .decisions()?
        .into_iter()
        .find(|d| d.id == id)
        .ok_or_else(|| StoreError::NotFound(id.to_owned()))?;
    let folded = g.checkpoints()?.iter().any(|c| c.chat_id == record.id);
    let session = t.session(&record.session_id)?;
    Ok((DecisionRow { record, folded }, session))
}

/// What regenerating a decision's cards produced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Regenerated {
    /// Cards saved by this run.
    pub saved: usize,
    /// Every card now linked to the decision.
    pub card_ids: Vec<String>,
    /// Drafts that failed `validate_card_shape`: `(front, reason)`.
    pub rejected: Vec<(String, String)>,
}

#[derive(Debug, thiserror::Error)]
pub enum RegenError {
    #[error("unknown decision {0}")]
    NotFound(String),
    #[error("{0}")]
    Engine(String),
    #[error("{0}")]
    Store(String),
}

/// Asks the engine for 4-choice cards on `decision_id` (existing cards keep their FSRS state)
/// and links the new ones to it. `tracker_dir` holds the link file.
pub async fn regenerate_cards<E: LlmEngine>(
    engine: &E,
    t: &dyn TrackerStore,
    tracker_dir: &Path,
    decision_id: &str,
    now: i64,
    cancel: CancellationToken,
) -> Result<Regenerated, RegenError> {
    let store = |e: StoreError| RegenError::Store(e.to_string());
    let rec = t
        .decisions()
        .map_err(store)?
        .into_iter()
        .find(|d| d.id == decision_id)
        .ok_or_else(|| RegenError::NotFound(decision_id.to_owned()))?;
    let before: HashSet<String> = t
        .cards()
        .map_err(store)?
        .into_iter()
        .map(|c| c.id.to_string())
        .collect();
    let report = lp_ingest::distill::generate_decision_cards(
        engine,
        t,
        std::slice::from_ref(&rec),
        now,
        cancel,
    )
    .await
    .map_err(|e| RegenError::Engine(e.to_string()))?;
    let fresh: Vec<String> = t
        .cards()
        .map_err(store)?
        .into_iter()
        .map(|c| c.id.to_string())
        .filter(|id| !before.contains(id))
        .collect();
    let mut links = CardLinks::read(tracker_dir);
    links.link(decision_id, fresh);
    links
        .save(tracker_dir)
        .map_err(|e| RegenError::Store(e.to_string()))?;
    Ok(Regenerated {
        saved: report.saved,
        card_ids: links.cards_of(decision_id).to_vec(),
        rejected: report
            .rejected
            .into_iter()
            .map(|(front, why)| (front, why.to_string()))
            .collect(),
    })
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

    #[test]
    fn grouped_digest_filters_by_day_and_source_and_sorts_newest_first() {
        let d = tempfile::tempdir().unwrap();
        let s = Stores::open(d.path()).unwrap();
        let t = s.tracker.as_ref();
        let day = 86_400_000;
        for (id, source, at) in [
            ("claude:a", "claude", 10 * day + 5),
            ("claude:b", "claude", 12 * day + 7),
            ("codex:c", "codex", 12 * day + 9),
        ] {
            t.upsert_session(&CodingSession {
                id: id.into(),
                source: source.into(),
                cwd: format!("/code/{source}-proj"),
                git_branch: None,
                started_at: at,
                transcript: String::new(),
            })
            .unwrap();
        }
        for (id, session) in [
            ("d1", "claude:a"),
            ("d2", "claude:b"),
            ("d3", "claude:b"),
            ("d4", "codex:c"),
            ("d5", "agy:lost"),
        ] {
            t.upsert_decision(&decision(id, session)).unwrap();
        }
        let shape = |days: &[DayGroup]| {
            days.iter()
                .map(|g| {
                    let sessions: Vec<String> = g
                        .sessions
                        .iter()
                        .map(|s| {
                            let ids: Vec<_> =
                                s.decisions.iter().map(|d| d.record.id.as_str()).collect();
                            format!("{}={}", s.session_id, ids.join("+"))
                        })
                        .collect();
                    format!("{}:{}", g.day, sessions.join(","))
                })
                .collect::<Vec<_>>()
        };
        let all = grouped(s.graph.as_ref(), t, &Filter::default()).unwrap();
        assert_eq!(
            shape(&all),
            [
                "1970-01-13:codex:c=d4,claude:b=d2+d3",
                "1970-01-11:claude:a=d1",
                "undated:agy:lost=d5"
            ]
        );
        assert_eq!(all[0].sessions[0].project, "codex-proj");
        assert_eq!(all[2].sessions[0].project, "-");

        let dated = Filter {
            from: Some(11 * day),
            ..Filter::default()
        };
        assert_eq!(
            shape(&grouped(s.graph.as_ref(), t, &dated).unwrap()),
            ["1970-01-13:codex:c=d4,claude:b=d2+d3"]
        );
        let claude = Filter {
            sources: vec!["claude".into()],
            to: Some(12 * day),
            ..Filter::default()
        };
        assert_eq!(
            shape(&grouped(s.graph.as_ref(), t, &claude).unwrap()),
            ["1970-01-11:claude:a=d1"]
        );

        let (row, session) = super::decision(s.graph.as_ref(), t, "d4").unwrap();
        assert_eq!(row.record.id, "d4");
        assert_eq!(session.unwrap().id, "codex:c");
        assert!(super::decision(s.graph.as_ref(), t, "zz").is_err());
        assert_eq!(day_of(0), "1970-01-01");
    }

    #[test]
    fn card_links_round_trip_and_ignore_duplicates() {
        let d = tempfile::tempdir().unwrap();
        assert_eq!(CardLinks::read(d.path()), CardLinks::default());
        let mut l = CardLinks::default();
        l.link("d1", ["c1".to_owned(), "c2".to_owned()]);
        l.link("d1", ["c2".to_owned(), "c3".to_owned()]);
        l.link("d2", ["c9".to_owned()]);
        l.save(d.path()).unwrap();
        let back = CardLinks::read(d.path());
        assert_eq!(back, l);
        assert_eq!(back.cards_of("d1"), ["c1", "c2", "c3"]);
        assert_eq!(back.cards_of("none"), &[] as &[String]);
        assert_eq!(back.decision_of("c9"), Some("d2"));
        assert_eq!(back.decision_of("zz"), None);
        std::fs::write(d.path().join(LINKS_FILE), "{").unwrap();
        assert_eq!(CardLinks::read(d.path()), CardLinks::default());
    }

    #[tokio::test]
    async fn regenerating_cards_links_new_ones_and_reports_rejects() {
        let d = tempfile::tempdir().unwrap();
        let s = Stores::open(d.path()).unwrap();
        let t = s.tracker.as_ref();
        t.upsert_decision(&decision("d1", "claude:s1")).unwrap();
        let faux = FauxEngine::new();
        faux.faux_structured(json!({"cards": [
            {"front": "Why copy a live WAL db first?", "right": "The WAL file is live", "wrong": ["It is faster", "It is smaller", "It is encrypted"], "rationale": "r"},
            {"front": "Same option twice?", "right": "x", "wrong": ["x", "y", "z"], "rationale": "r"}
        ]}));
        let tdir = d.path().join("tracker");
        let out = regenerate_cards(&faux, t, &tdir, "d1", 5, CancellationToken::new())
            .await
            .unwrap();
        assert_eq!(out.saved, 1);
        assert_eq!(out.card_ids.len(), 1);
        assert_eq!(out.rejected.len(), 1);
        assert_eq!(out.rejected[0].0, "Same option twice?");
        assert!(out.rejected[0].1.contains("distinct"), "{:?}", out.rejected);
        assert_eq!(CardLinks::read(&tdir).cards_of("d1"), out.card_ids);

        let gone = regenerate_cards(&faux, t, &tdir, "nope", 5, CancellationToken::new()).await;
        assert!(matches!(gone, Err(RegenError::NotFound(_))));
        // No scripted response left: the engine fails and nothing is linked.
        let dead = regenerate_cards(&faux, t, &tdir, "d1", 5, CancellationToken::new()).await;
        assert!(matches!(dead, Err(RegenError::Engine(_))), "{dead:?}");
    }
}
