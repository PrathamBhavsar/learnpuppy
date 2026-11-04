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

## 2026-10-28 — Phase D Native macOS Visual Desktop Completion & D60 Final Audit
- Completed Phase D worktrees (`D00`, `D10`, `D11`, `D20`, `D21`, `D22`, `D23`, `D30`) delivering `crates/lp-ui` (OKLCH dark theme, 20+ Lucide SVG icons, `Button`, `Badge`, `Card`, `Progress`, `MasteryRing`, `CodeBlock`, `Dialog`, `Tabs`, `Tooltip`, `Slider`, `Switch`, `ScrollArea`, `SplitPane`, `CommandPalette`) and `crates/lp-app` (Desktop Shell, async `DesktopBackend` over `tokio::task::spawn_blocking`, Visual Classroom, interactive SVG Knowledge Graph Canvas & 3-level Chain Table, FSRS 4-choice Review Deck, Local CLI Transcript Decision Digest Browser, and Multi-Account CLI Pool modal).
- Completed `D60` final Opus audit across `crates/lp-ui` and `crates/lp-app`: verified zero occurrences of banned browser/WASM APIs (`wasm_bindgen`, `web_sys`, `js_sys`, `gloo_timers`, `dangerous_inner_html`, `setTimeout`), verified `evt.prevent_default()` on all 7 `<form onsubmit>` handlers, and verified all SQLite/filesystem/ForceAtlas2 workloads run off the Dioxus UI thread via `tokio::task::spawn_blocking`.
- Fixed boolean HTML `hidden="false"` serialization bug in `TabsContent` (`crates/lp-ui/src/tabs.rs`) and `Dialog` (`crates/lp-ui/src/dialog.rs`) using `(!is_active).then_some("true")` / `(!is_open).then_some("true")`, and fixed `CommandPalette` (`crates/lp-ui/src/command_palette.rs`) to clear its search query and reset selection on close across both internal and external close transitions, with `dioxus-ssr` regression tests.
- Verified full `make ci` gate, `crates/lp-app/tests/e2e_desktop.rs`, and `tester-army/e2e` browser suites (`npx e2e run`), marking `D00`–`D60` as `done` in [Task DAG](/tasks/index.md).

## 2026-10-31 — V01+V02 Repo Surgery: Web UI Over Dioxus (`lp-service` + `lp-server`)
- Ported settings/profiles, `CliPoolEngine` role routing, the classroom session helpers (`run_to_rest`, `pending_call`, `render_answer`, `option_lines`, `resolve_option`, `verdict`, `classroom_view`), FSRS review, digest data prep, graph delete/practice, accounts and ingest from `lp-tui`/`lp-app` into the new no-HTTP `crates/lp-service` with their unit tests, plus a `paths` data-root helper. `lp-store` constructors already take a root directory (no hardcoded `~/.learnpuppy`).
- Deleted `lp-ui`, `lp-app`, `lp-tui`, the Dioxus `tests/`, root `e2e.config.ts`/`e2e.web.config.ts`/`package.json`, and `scripts/export-icons.ts`; pruned UI-only workspace deps and `deny.toml` exceptions. Added `crates/lp-server` (binary `learnpuppy`: `serve`, `migrate`, `seed`, `doctor`, `openapi`). Workspace is now 7 crates.
- Recorded [ADR-0006](/decisions/0006-web-ui-over-dioxus.md) superseding [ADR-0005](/decisions/0005-dioxus-desktop-over-tui.md); archived [Dioxus UI](/architecture/dioxus-ui.md) and [TUI](/architecture/tui.md) specs; added the web rollout to the [Task DAG](/tasks/index.md).


## 2026-11-01 — D60 Docs Sweep
- Added [Web UI](/architecture/web-ui.md) and [Server API](/architecture/server-api.md) specs, root `README.md`, superseded note on `docs/PLAN.md`, archived pointer on `docs/DESIGN_SYSTEM_PLAN.md`; marked `D60` done in the [Task DAG](/tasks/index.md).


## 2026-11-04 — Alvar Method Brain Section
- Watched Eero Alvar's *How I Use AI to Learn Things* (youtu.be/kzcI5F4tGiU, 2026-08-14, 18:58) via oembed metadata + cleaned auto-caption transcript from the Alvarmethod repo.
- Identified companion video *How I Learn Difficult Things* (youtu.be/ciC6ffUqI8k) — "Start from facts the brain can accept without hedging, then build by motivated discovery at the edge of what you already hold."
- Surveyed 5 community implementations: Alvarmethod (vasanthsreeram), agentic-learning (pacchio1), EmilioFarias/teach, Mathesis (tdsiar), learning-with-copilot (MMostafa-Hub). Fetched all skill files (SKILL.md, protocols.md, textbook-mode.md, quiz-ui.md, learner-files.md, philosophy.md, process.md) and the LEARNER.md template.
- Created [Alvar Method](/alvar-method/index.md) brain section with 8 documents: index, philosophy (one-to-one teaching), process (Probe → Plan → Teach → Close), teaching protocols (mastery bar, graded quizzes, prediction-before-reveal, faded examples, interleaving, Feynman pass, spaced review, misconception register, confidence tracking, no-reveal rule, notes gate), visual & verify skills (self-reviewed SVG loop, fact-check subagent), skills reference (5 skills + quiz UI mapping per harness + file structures), community implementations (comparison table + LearnPuppy mapping), and full transcript.
- Cross-referenced the Alvar Method concepts to LearnPuppy's crate architecture (`lp-core` 3-phase fold, `lp-agent` ProberAgent/TeacherAgent/subagents, `lp-store` FSRS v5, `lp-service` profiles, `lp-server` SSE → React SPA).


## 2026-11-04 — Huberman Learning Brain Section
- Watched Andrew Huberman's *Optimal Protocols for Studying & Learning* (youtu.be/ddq8JIMhz7c, Huberman Lab Podcast) via oembed metadata + full episode transcript from hubermanlab.com (105,393 bytes).
- Also fetched the *Essentials: Optimize Your Learning & Creativity* episode transcript (35,435 bytes) covering daily routine, circadian mechanisms, go/no-go circuits, and creativity.
- Fetched the *Neuroplasticity Super-Protocol* newsletter (7,514 bytes) with the condensed 9-step protocol.
- Cross-referenced with ai.hubermanlab.com summaries for structured study habit data.
- Created [Huberman Learning](/huberman-learning/index.md) brain section with 8 documents: index, neuroplasticity fundamentals (3 mechanisms, two-step process, sleep, NSDR), self-testing protocol (1917 study, SSSS/SSST/STTT, familiarity vs mastery, timing, 50% rule, test format), study habits (~700 med students: schedule/alone/phone-away/3-4hrs/teach-peers/aspirational), neuroplasticity super-protocol (9 steps), circadian optimization (autonomic arousal, go/no-go D1/D2, two daily bouts, creativity two-stage, morning/evening routines), advanced tools (gap effects, interleaving, emotion, ~15% error rate, cold exposure, skill levels), and transcript excerpts.
- Cross-referenced Huberman concepts to both LearnPuppy's crate architecture and the Alvar Method (e.g., self-testing ↔ quiz-after-every-step, gap effects ↔ potential lp-agent feature, interleaving ↔ mixed pass every third quiz, familiarity vs mastery ↔ mastery bar MC + free recall).
