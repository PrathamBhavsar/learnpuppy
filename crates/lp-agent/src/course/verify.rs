//! F08: fact-check ledger. The Author lists checkable claims; this background job asks a cheap
//! model for a source and an exact quote, then code fetches that page and checks the quote is
//! really there — a cheap worker has already invented citations in this project, so the model's
//! verdict is never trusted without a quote that survives the fetch.

use lp_cli_driver::engine::LlmEngine;
use lp_core::course::{Claim, ClaimVerdict, PACKET_NOTE};
use serde::Deserialize;

use super::{ask_parsed, parse, render, Fence};
use crate::workbench::Web;

const VERIFY: &str = include_str!("prompts/verify.md");

#[derive(Deserialize, Default)]
#[serde(default)]
struct Raw {
    source: String,
    quote: String,
    verdict: String,
}

/// Collapses whitespace and case so a quote matches across line wraps and re-flowing.
fn norm(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

fn unknown(text: &str, source: Option<String>) -> Claim {
    Claim {
        text: text.to_string(),
        verdict: ClaimVerdict::Unknown,
        source,
    }
}

/// Asks a cheap model for a source and an exact quote for one claim, then has code fetch that
/// page and check the quote is really on it. A missing source or quote, a failed fetch, or a
/// quote code cannot find on the fetched page all force `Unknown` — never `Confirmed` or
/// `Contradicted` on the model's say alone. Never fails: a check that cannot be done must never
/// block study (F08).
pub async fn verify_claim<E: LlmEngine, W: Web>(engine: &E, web: &W, claim_text: &str) -> Claim {
    let fence = Fence::new();
    let prompt = render(
        VERIFY,
        &[("claim", &format!("{PACKET_NOTE}{}", fence.wrap(claim_text)))],
    );
    let Ok(raw) = ask_parsed(engine, &prompt, parse::<Raw>).await else {
        return unknown(claim_text, None);
    };
    if raw.source.trim().is_empty() || raw.quote.trim().is_empty() {
        return unknown(claim_text, None);
    }
    let Ok(page) = web.fetch(&raw.source).await else {
        return unknown(claim_text, Some(raw.source));
    };
    if !norm(&page).contains(&norm(&raw.quote)) {
        return unknown(claim_text, Some(raw.source));
    }
    let verdict = match raw.verdict.as_str() {
        "confirmed" => ClaimVerdict::Confirmed,
        "qualified" => ClaimVerdict::Qualified,
        "contradicted" => ClaimVerdict::Contradicted,
        _ => ClaimVerdict::Unknown,
    };
    Claim {
        text: claim_text.to_string(),
        verdict,
        source: Some(raw.source),
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use lp_cli_driver::engine::FauxEngine;

    use super::*;
    use crate::workbench::Hit;

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

    #[tokio::test]
    async fn quote_found_on_page_is_trusted() {
        let engine = FauxEngine::new();
        engine.faux_text(
            r#"{"source":"https://example.test/raft","quote":"the leader sends heartbeats","verdict":"confirmed"}"#,
        );
        let web = FauxWeb(Ok(
            "In Raft, the Leader Sends Heartbeats to followers.".into()
        ));
        let claim = verify_claim(&engine, &web, "The leader sends heartbeats.").await;
        assert_eq!(claim.verdict, ClaimVerdict::Confirmed);
        assert_eq!(claim.source, Some("https://example.test/raft".into()));
    }

    #[tokio::test]
    async fn quote_missing_from_page_is_forced_unknown() {
        let engine = FauxEngine::new();
        engine.faux_text(
            r#"{"source":"https://example.test/raft","quote":"this exact wording is not on the page","verdict":"confirmed"}"#,
        );
        let web = FauxWeb(Ok("Totally unrelated page content.".into()));
        let claim = verify_claim(&engine, &web, "Some claim.").await;
        assert_eq!(claim.verdict, ClaimVerdict::Unknown);
    }

    #[tokio::test]
    async fn fetch_failure_is_forced_unknown() {
        let engine = FauxEngine::new();
        engine.faux_text(
            r#"{"source":"https://example.test/gone","quote":"anything","verdict":"confirmed"}"#,
        );
        let web = FauxWeb(Err("404".into()));
        let claim = verify_claim(&engine, &web, "Some claim.").await;
        assert_eq!(claim.verdict, ClaimVerdict::Unknown);
        assert_eq!(claim.source, Some("https://example.test/gone".into()));
    }

    #[tokio::test]
    async fn no_source_found_is_unknown_with_no_source() {
        let engine = FauxEngine::new();
        engine.faux_text(r#"{"source":"","quote":"","verdict":"confirmed"}"#);
        let web = FauxWeb(Ok(String::new()));
        let claim = verify_claim(&engine, &web, "Some claim.").await;
        assert_eq!(claim.verdict, ClaimVerdict::Unknown);
        assert_eq!(claim.source, None);
    }

    #[tokio::test]
    async fn engine_failure_is_unknown() {
        let engine = FauxEngine::new();
        let web = FauxWeb(Ok(String::new()));
        let claim = verify_claim(&engine, &web, "Some claim.").await;
        assert_eq!(claim.verdict, ClaimVerdict::Unknown);
        assert_eq!(claim.source, None);
    }

    #[tokio::test]
    async fn contradicted_verdict_survives_a_confirmed_quote() {
        let engine = FauxEngine::new();
        engine.faux_text(
            r#"{"source":"https://example.test/raft","quote":"the leader never sends heartbeats","verdict":"contradicted"}"#,
        );
        let web = FauxWeb(Ok(
            "The leader never sends heartbeats in this broken system.".into(),
        ));
        let claim = verify_claim(&engine, &web, "The leader sends heartbeats.").await;
        assert_eq!(claim.verdict, ClaimVerdict::Contradicted);
    }
}
