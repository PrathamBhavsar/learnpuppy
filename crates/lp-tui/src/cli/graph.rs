//! `learnpuppy graph stats | search | import`.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{bail, Context};
use clap::Subcommand;
use lp_agent::extract::Pipeline;
use lp_cli_driver::engine::LlmEngine;
use lp_core::graph::GraphSnapshot;
use lp_core::ports::GraphStore;
use lp_store::FsGraphStore;
use tokio_util::sync::CancellationToken;

#[derive(Subcommand)]
pub enum Cmd {
    Stats,
    Search {
        query: String,
    },
    /// Fold a Markdown/text file (or a PDF, via `pdftotext`) into the graph.
    Import {
        file: PathBuf,
    },
}

fn read_notes(file: &Path) -> anyhow::Result<String> {
    if file
        .extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
    {
        let out = std::process::Command::new("pdftotext")
            .arg(file)
            .arg("-")
            .output()
            .context("pdftotext is required to import PDFs")?;
        if !out.status.success() {
            bail!("pdftotext failed on {}", file.display());
        }
        return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
    }
    std::fs::read_to_string(file).with_context(|| file.display().to_string())
}

pub async fn run<E: LlmEngine>(
    store: Arc<FsGraphStore>,
    engine: Arc<E>,
    cmd: Cmd,
    out: &mut dyn Write,
) -> anyhow::Result<()> {
    match cmd {
        Cmd::Stats => {
            let snap = GraphSnapshot::from_ledgers(&store.nodes()?, &store.chains()?);
            writeln!(
                out,
                "nodes {}\nchains {}\nclusters {}\ncovered messages {}\nimported sources {}",
                snap.nodes.len(),
                snap.chains.len(),
                snap.birth_session_partition().len(),
                store.coverage()?.len(),
                store.checkpoints()?.len(),
            )?;
        }
        Cmd::Search { query } => {
            for n in store.search_propositions(&query, 20)? {
                writeln!(out, "{}  {} — {}", n.id, n.name, n.statement)?;
            }
        }
        Cmd::Import { file } => {
            let text = read_notes(&file)?;
            let before = store.nodes()?.len();
            let source = Pipeline::new(engine, store.clone())
                .import_notes(&text, CancellationToken::new())
                .await?;
            writeln!(
                out,
                "imported {source}: {} new node lines",
                store.nodes()?.len() - before
            )?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use lp_cli_driver::engine::FauxEngine;
    use serde_json::json;

    #[tokio::test]
    async fn import_then_stats_and_search() {
        let d = tempfile::tempdir().unwrap();
        let store = Arc::new(FsGraphStore::open(d.path().join("g")).unwrap());
        let faux = Arc::new(FauxEngine::new());
        faux.faux_structured(json!({
            "propositions": [{"id": "p1", "name": "Ownership", "statement": "Each value has one owner"}],
            "chains": []
        }));
        faux.faux_structured(json!({"links": []}));
        let notes = d.path().join("n.md");
        std::fs::write(&notes, "Each value has one owner.").unwrap();
        let mut out = Vec::new();
        run(
            store.clone(),
            faux.clone(),
            Cmd::Import { file: notes },
            &mut out,
        )
        .await
        .unwrap();
        assert!(String::from_utf8(out).unwrap().contains("1 new node lines"));

        let mut out = Vec::new();
        run(store.clone(), faux.clone(), Cmd::Stats, &mut out)
            .await
            .unwrap();
        assert!(String::from_utf8(out).unwrap().contains("nodes 1"));
        let mut out = Vec::new();
        let q = Cmd::Search {
            query: "owner".into(),
        };
        run(store, faux, q, &mut out).await.unwrap();
        assert!(String::from_utf8(out).unwrap().contains("Ownership"));
    }
}
