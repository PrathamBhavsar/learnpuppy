# LearnPuppy Project Log

Back to [Brain Index](/index.md). Related: [Overview](/overview.md), [Task DAG](/tasks/index.md).

## 2026-10-27 — Reverse Engineering & Master Plan Initialization
- Inspected `/Applications/LearnHarness.app` (`v0.3.8`) using `rea-agents` (`/tmp/rea-learnharness.json`) and cloned `https://github.com/amosblomqvist/learnharness.git` (`/tmp/learnharness-repo`).
- Audited all `learnharness` core contracts (`graph.ts`, `plan.ts`, `probe.ts`, `jobs.ts`, `chat.ts`, `chains.ts`, `question-core.ts`), agent tools (`probe`/`recall` vs `quiz`/`explain`, `probe_done`, `plan_init`, `problem`/`conclude`), and the 3-stage extraction pipeline (`Extractor -> Verdicts-Only Auditor -> Deterministic Merger` + `simplifyChains`).
- Audited `/Users/pratham/code/tracker/server/digest/` (`ingest.ts`, `llm.ts`, `distill.ts`, `cards.ts`, `fsrs.ts`) for local Claude/Codex/AGY transcript ingestion, git commit window matching, decision distillation, distractor quality gates, and deterministic FSRS v5 review replay.
- Verified multi-account Claude setup on host machine (`~/.claude` for `q97159745@gmail.com` and `~/.claude-accounts/account2` for `pratham.bhavsar501@gmail.com`, plus `~/.codex/auth.json` and `~/.gemini/oauth_creds.json`).
- Authored `docs/REVERSE_ENGINEERING.md`, `docs/PLAN.md`, and initialized the OKF v0.1 knowledge bundle in `brain/` with `prod` and `staging` git branches.
  - Files: [Overview](/overview.md), [System Architecture](/architecture/system.md), [Multi-Account CLI Pool](/architecture/cli-auth-pool.md), [Task DAG](/tasks/index.md).

## 2026-10-28 — Phase 1–5 Backend & TUI Completion + Pivot to Dioxus 0.7 Visual Desktop (`lp-ui` + `lp-app`)
- Completed full Rust implementation of `lp-core`, `lp-store`, `lp-cli-driver` (multi-account Claude pool + Codex + AGY + SSRF-hardened web adapter), `lp-agent` (Prober, Teacher, Practice, Workbench sub-agents, 3-stage Graph Extractor/Auditor/Merger), `lp-ingest` (transcript + git ingestor, decision distiller, FSRS card generator), and `lp-tui` (28,183 LOC across 6 crates).
- Isolated the complete Ratatui CLI/TUI implementation on git branch `legacy/tui-cli`.
- Cloned and audited `DioxusLabs/components` (`dioxus-primitives` 0.7) and `rust-ui/dioxus-ui` to establish strict Dioxus 0.7 desktop engineering rules (banning `rust-ui/dioxus-ui`'s inline `<script>` DOM polling and `wasm-bindgen` dependencies).
- Installed per-project skills from `humanlayer/skills` (`rpi`, `show-me`, `visual-pr`, `improve-claude-md`, `build-iterated-agentic-loop`, `design-control-loop`) and `tester-army/e2e` (`e2e`, `create-verification-skill`), plus custom `dioxus-desktop-ui` and `orca-model-matrix` skills in `.claude/skills/` and `.agents/skills/`.
- Authored `<important if="...">` structured `CLAUDE.md`, [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [ADR-0005](/decisions/0005-dioxus-desktop-over-tui.md), and the Phase D multi-worktree execution DAG (`D00`–`D60` for macOS first, `T70` Windows next, `T80` iOS/Android last).
