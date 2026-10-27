//! `learnpuppy ingest [--since 7d] [--repo <path>]`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context};
use clap::Args as ClapArgs;
use lp_agent::extract::Pipeline;
use lp_cli_driver::engine::LlmEngine;
use lp_core::ports::{DecisionRecord, GraphStore, TrackerStore};
use lp_ingest::distill::{distill_session, emit_to_graph, generate_decision_cards};
use lp_ingest::{commits_for_session, ingest, IngestReport, Roots};
use tokio_util::sync::CancellationToken;

use crate::state::{now_ms, Settings, Stores};

#[derive(ClapArgs)]
pub struct Args {
    /// Only sessions started within this window: `<n>h`, `<n>d` or `<n>w`.
    #[arg(long)]
    since: Option<String>,
    /// Repo to allowlist (saved to settings); repeatable.
    #[arg(long)]
    repo: Vec<PathBuf>,
    /// Ingest the author's own Rust / system-design repos (no allowlist needed).
    #[arg(long)]
    seed_rust_sysdesign: bool,
}

fn since_ms(spec: &str, now: i64) -> anyhow::Result<i64> {
    lp_ingest::since_ms(spec, now).map_err(|e| anyhow::anyhow!(e))
}

/// Scans transcripts, then distills each new session into decisions, cards and graph passages.
pub async fn run<E: LlmEngine + 'static>(
    stores: &Stores,

    engine: Arc<E>,
    root: &Path,
    home: &Path,
    settings: &mut Settings,
    args: Args,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let tracker = stores.tracker.as_ref();
    let mut changed = false;
    for r in args.repo {
        let r = r.canonicalize().with_context(|| r.display().to_string())?;
        if !settings.repos.contains(&r) {
            settings.repos.push(r);
            changed = true;
        }
    }
    if changed {
        settings.save(root)?;
    }
    let mut allow = settings.repos.clone();
    let rep = if args.seed_rust_sysdesign {
        allow.extend(["code/tracker", "code/bunfiiliate"].map(|r| home.join(r)));
        lp_ingest::distill::seed_rust_sysdesign(tracker, home)
    } else {
        if allow.is_empty() {
            bail!("no repos allowlisted; pass --repo <path>");
        }
        let since = args.since.map_or(Ok(0), |s| since_ms(&s, now_ms()))?;
        let mut roots = Roots::from_home(home);
        roots
            .claude
            .extend(settings.claude_dirs.iter().map(|d| d.join("projects")));
        if let Some(d) = std::env::var_os("CLAUDE_CONFIG_DIR") {
            roots.claude.push(PathBuf::from(d).join("projects"));
        }
        ingest(tracker, &roots, &allow, since)
    };
    writeln!(out, "files {}  sessions {}", rep.files, rep.sessions)?;
    for e in &rep.errors {
        writeln!(out, "error: {e}")?;
    }
    distill_all(tracker, stores.graph.clone(), engine, &allow, &rep, out).await
}

async fn distill_all<E: LlmEngine + 'static, G: GraphStore + 'static>(
    tracker: &dyn TrackerStore,
    graph: Arc<G>,
    engine: Arc<E>,
    allow: &[PathBuf],
    rep: &IngestReport,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    let pipe = Pipeline::new(engine.clone(), graph);
    let cancel = CancellationToken::new();
    let (mut decisions, mut cards): (Vec<DecisionRecord>, usize) = (vec![], 0);
    for s in &rep.stored {
        let commits = commits_for_session(s, s.started_at, allow);
        let step = async {
            let recs =
                distill_session(engine.as_ref(), tracker, s, &commits, cancel.clone()).await?;
            let made =
                generate_decision_cards(engine.as_ref(), tracker, &recs, now_ms(), cancel.clone())
                    .await?;
            emit_to_graph(&pipe, &s.id, &recs, cancel.clone()).await?;
            anyhow::Ok((recs, made.saved))
        };
        match step.await {
            Ok((recs, saved)) => {
                decisions.extend(recs);
                cards += saved;
            }
            Err(e) => writeln!(out, "error: {}: {e}", s.id)?,
        }
    }
    writeln!(out, "decisions {}  cards {cards}", decisions.len())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_since_windows() {
        assert_eq!(since_ms("7d", 10 * 86_400_000).unwrap(), 3 * 86_400_000);
        assert_eq!(since_ms("2h", 3_600_000 * 3).unwrap(), 3_600_000);
        assert!(since_ms("7x", 0).is_err());
        assert!(since_ms("", 0).is_err());
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
        use lp_cli_driver::engine::FauxEngine;
        use serde_json::json;
        let d = tempfile::tempdir().unwrap();
        let stores = Stores::open(d.path()).unwrap();
        let t = stores.tracker.clone();
        let (root, home) = (d.path().join("lp"), d.path().join("home"));
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
        let engine = Arc::new(FauxEngine::new());
        let mut s = Settings::default();
        let none = Args {
            since: None,
            repo: vec![],
            seed_rust_sysdesign: false,
        };
        let mut sink = Vec::new();
        let r = run(
            &stores,
            engine.clone(),
            &root,
            &home,
            &mut s,
            none,
            &mut sink,
        );
        assert!(r.await.is_err());

        engine.faux_structured(json!({"decisions": [{
            "title": "Snapshot copy", "what": "copy db", "why": "WAL is live", "basis": "stated",
            "alternatives": [], "risk": "torn copy", "files": [], "commit_sha": null}]}));
        engine.faux_structured(json!({"cards": [{
            "kind": "why", "front": "Why copy the db first?", "right": "The WAL file is live",
            "wrong": ["It is faster", "It is smaller", "It is encrypted"], "rationale": "because"}]}));
        engine.faux_structured(json!({"propositions": [{"id": "p1", "name": "Snapshot before reading WAL", "statement": "Copy a live WAL db before reading it."}], "chains": []}));
        engine.faux_structured(json!({"links": []}));
        let args = Args {
            since: Some("100000d".into()),
            repo: vec![repo],
            seed_rust_sysdesign: false,
        };
        let mut out = Vec::new();
        run(&stores, engine, &root, &home, &mut s, args, &mut out)
            .await
            .unwrap();
        let text = String::from_utf8(out).unwrap();
        assert_eq!(t.decisions().unwrap().len(), 1, "{text}");
        assert_eq!(t.cards().unwrap().len(), 1, "{text}");
        assert!(text.contains("decisions 1  cards 1"), "{text}");
    }
}
