//! Transcript ingest for one workspace: scan allowlisted sessions, then distill each new session
//! into decisions, FSRS cards and graph passages.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use lp_agent::extract::Pipeline;
use lp_cli_driver::engine::LlmEngine;
use lp_core::ports::{GraphStore, TrackerStore};
use lp_ingest::distill::{distill_session, emit_to_graph, generate_decision_cards};
use lp_ingest::{commits_for_session, IngestReport, Roots};
use tokio_util::sync::CancellationToken;

/// Transcript roots: the defaults under `home`, every saved Claude profile, and `$CLAUDE_CONFIG_DIR`.
pub fn roots(home: &Path, claude_dirs: &[PathBuf], claude_config_dir: Option<PathBuf>) -> Roots {
    let mut roots = Roots::from_home(home);
    roots
        .claude
        .extend(claude_dirs.iter().map(|d| d.join("projects")));
    if let Some(d) = claude_config_dir {
        roots.claude.push(d.join("projects"));
    }
    roots
}

/// The start of an ingest window: `<n>h`, `<n>d` or `<n>w` before `now`.
pub fn since_ms(spec: &str, now: i64) -> anyhow::Result<i64> {
    lp_ingest::since_ms(spec, now).map_err(|e| anyhow::anyhow!(e))
}

/// Scans transcripts of sessions inside `allow` started at/after `since`. An empty allow-list
/// is refused: ingest never reads everything by accident. Blocking.
pub fn scan(
    t: &dyn TrackerStore,
    roots: &Roots,
    allow: &[PathBuf],
    since: i64,
) -> anyhow::Result<IngestReport> {
    anyhow::ensure!(!allow.is_empty(), "no repos allowlisted");
    Ok(lp_ingest::ingest(t, roots, allow, since))
}

/// What distilling one scan produced.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Distilled {
    pub decisions: usize,
    pub cards: usize,
    /// `<session id>: <error>`; a failing session never stops the others.
    pub errors: Vec<String>,
    /// Sessions whose git commits could not be read (distilled without them).
    pub warnings: Vec<String>,
}

/// Distills every session stored by `rep` into decisions, cards and graph passages.
pub async fn distill<E, G>(
    t: &dyn TrackerStore,
    graph: Arc<G>,
    engine: Arc<E>,
    allow: &[PathBuf],
    rep: &IngestReport,
    now: i64,
    cancel: CancellationToken,
) -> Distilled
where
    E: LlmEngine + 'static,
    G: GraphStore + 'static,
{
    let pipe = Pipeline::new(engine.clone(), graph);
    let mut out = Distilled::default();
    for s in &rep.stored {
        let commits = match commits_for_session(s, s.started_at, allow).await {
            Ok(c) => c,
            Err(e) => {
                out.warnings.push(format!("{}: {e}", s.id));
                vec![]
            }
        };
        let step = async {
            let recs = distill_session(engine.as_ref(), t, s, &commits, cancel.clone()).await?;
            let made =
                generate_decision_cards(engine.as_ref(), t, &recs, now, cancel.clone()).await?;
            emit_to_graph(&pipe, &s.id, &recs, cancel.clone()).await?;
            anyhow::Ok((recs.len(), made.saved))
        };
        match step.await {
            Ok((decisions, cards)) => {
                out.decisions += decisions;
                out.cards += cards;
            }
            Err(e) => out.errors.push(format!("{}: {e}", s.id)),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stores::Stores;
    use lp_cli_driver::engine::FauxEngine;
    use serde_json::json;

    #[test]
    fn parses_since_windows() {
        assert_eq!(since_ms("7d", 10 * 86_400_000).unwrap(), 3 * 86_400_000);
        assert_eq!(since_ms("2h", 3_600_000 * 3).unwrap(), 3_600_000);
        assert!(since_ms("7x", 0).is_err());
        assert!(since_ms("", 0).is_err());
    }

    #[test]
    fn roots_include_saved_profiles_and_config_dir() {
        let r = roots(
            Path::new("/h"),
            &[PathBuf::from("/p1")],
            Some(PathBuf::from("/cfg")),
        );
        assert!(r.claude.contains(&PathBuf::from("/p1/projects")));
        assert!(r.claude.contains(&PathBuf::from("/cfg/projects")));
    }

    fn claude_session(cwd: &str) -> String {
        format!(
            r#"{{"type":"user","cwd":"{cwd}","gitBranch":"main","timestamp":"2026-01-01T00:00:00Z","message":{{"content":"why a snapshot copy?"}}}}
{{"type":"assistant","timestamp":"2026-01-01T00:00:01Z","message":{{"content":[{{"type":"text","text":"WAL files are live."}}]}}}}
"#
        )
    }

    #[tokio::test]
    async fn requires_an_allowlisted_repo_then_distills_sessions_into_cards() {
        let d = tempfile::tempdir().unwrap();
        let stores = Stores::open(&d.path().join("ws")).unwrap();
        let t = stores.tracker.as_ref();
        let home = d.path().join("home");
        let repo = d.path().join("repo");
        let proj = home.join(".claude/projects/p");
        std::fs::create_dir_all(&proj).unwrap();
        std::fs::create_dir_all(&repo).unwrap();
        let repo = repo.canonicalize().unwrap();
        std::fs::write(
            proj.join("s.jsonl"),
            claude_session(&repo.display().to_string()),
        )
        .unwrap();
        let r = roots(&home, &[], None);
        assert!(scan(t, &r, &[], 0).is_err());

        let engine = Arc::new(FauxEngine::new());
        engine.faux_structured(json!({"decisions": [{
            "title": "Snapshot copy", "what": "copy db", "why": "WAL is live", "basis": "stated",
            "alternatives": [], "risk": "torn copy", "files": [], "commit_sha": null}]}));
        engine.faux_structured(json!({"cards": [{
            "kind": "why", "front": "Why copy the db first?", "right": "The WAL file is live",
            "wrong": ["It is faster", "It is smaller", "It is encrypted"], "rationale": "because"}]}));
        engine.faux_structured(json!({"propositions": [{"id": "p1", "name": "Snapshot before reading WAL", "statement": "Copy a live WAL db before reading it."}], "chains": []}));
        engine.faux_structured(json!({"links": []}));
        let allow = [repo];
        let rep = scan(t, &r, &allow, 0).unwrap();
        assert_eq!(rep.sessions, 1);
        let out = distill(
            t,
            stores.graph.clone(),
            engine,
            &allow,
            &rep,
            crate::now_ms(),
            CancellationToken::new(),
        )
        .await;
        assert_eq!((out.decisions, out.cards), (1, 1), "{out:?}");
        assert!(out.errors.is_empty(), "{out:?}");
        assert_eq!(t.decisions().unwrap().len(), 1);
        assert_eq!(t.cards().unwrap().len(), 1);
        assert!(!stores.graph.nodes().unwrap().is_empty());
    }
}
