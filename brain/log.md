# LearnPuppy Project Log

Back to [Brain Index](/index.md). Related: [Overview](/overview.md), [Task DAG](/tasks/index.md).

## 2026-10-27 — Reverse Engineering & Master Plan Initialization
- Inspected `/Applications/LearnHarness.app` (`v0.3.8`) using `rea-agents` (`/tmp/rea-learnharness.json`) and cloned `https://github.com/amosblomqvist/learnharness.git` (`/tmp/learnharness-repo`).
- Audited all `learnharness` core contracts (`graph.ts`, `plan.ts`, `probe.ts`, `jobs.ts`, `chat.ts`, `chains.ts`, `question-core.ts`), agent tools (`probe`/`recall` vs `quiz`/`explain`, `probe_done`, `plan_init`, `problem`/`conclude`), and the 3-stage extraction pipeline (`Extractor -> Verdicts-Only Auditor -> Deterministic Merger` + `simplifyChains`).
- Audited `/Users/pratham/code/tracker/server/digest/` (`ingest.ts`, `llm.ts`, `distill.ts`, `cards.ts`, `fsrs.ts`) for local Claude/Codex/AGY transcript ingestion, git commit window matching, decision distillation, distractor quality gates, and deterministic FSRS v5 review replay.
- Verified multi-account Claude setup on host machine (`~/.claude` for `q97159745@gmail.com` and `~/.claude-accounts/account2` for `pratham.bhavsar501@gmail.com`, plus `~/.codex/auth.json` and `~/.gemini/oauth_creds.json`).
- Authored `docs/REVERSE_ENGINEERING.md`, `docs/PLAN.md`, and initialized the OKF v0.1 knowledge bundle in `brain/` with `prod` and `staging` git branches.
  - Files: [Overview](/overview.md), [System Architecture](/architecture/system.md), [Multi-Account CLI Pool](/architecture/cli-auth-pool.md), [Task DAG](/tasks/index.md).
