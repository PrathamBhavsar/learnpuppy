# LearnPuppy Project Log

Back to [Brain Index](/index.md). Related: [Overview](/overview.md), [Task DAG](/tasks/index.md).

## [2026-11-13] Site theme pass

- **Done**: Site restyled to product UI: light violet tokens from `web/src/styles/tokens.css`, Geist Mono headings, 10px radius, dark follows OS (softer violet). Pixel type only on logo + ASCII scene. Branch `feat/site-ui-theme`, merged to staging.

## 2026-11-11 — Open items closed (pratham404/deps-sqlite)
- Strand probe (F06) and course `why` (F02) are wired into course creation. `create(goal, why)` writes the probe with one cheap call (falls back to the plain quiz if that comes back unusable); `submit_placement` grades a round and returns `next`, binary-searching each strand with `lp_core::method::probe` (middle level first, then up or down, at most 6 questions). `guessed` makes a right answer `edge`, `blocked` labels the strand `blocked`. Labels and `why` are stored on the course and returned by `/plan`; the ready gate shows the `why`. Web: why field on create, follow-up rounds with a one-line verdict, guess and prerequisite checkboxes.
- Review (the `ocr` step ran as a self-review via `ocr delegate`, no LLM endpoint): a repeated or partial probe round is refused without changing anything, so a double submit cannot grade the next round blank.
- `rusqlite` 0.32 to 0.41 (`fallible_uint` keeps the u64/usize conversions), which fixes RUSTSEC-2026-0459; `Zlib` allowed in `deny.toml` for `foldhash`. `make deny` passes.
- `make okf` picks a python3 that can import PyYAML (Make puts Homebrew's, without it, first on PATH).
- Real smoke on real models: 6 probe questions, 10 lessons, why and three strand labels on the plan, zero model calls in the walk.

## 2026-11-10 — Finalize: plans closed, SSRF merged (pratham404/finalize)
- Learning method (ADR-0008) integrated: server gates `step` on the bout clock and honours workspace settings, `BoutShell` wired into the course session, settings route and screen. G3 benchmark run on the shipped Author prompt, see [G3 result](/learning-method/g3-result.md); it found plain-string claims not loading and angles that state the answer, both fixed in lp-core.
- SSRF: httpweb pins the normalised host, ignores proxies, blocks reserved and NAT64 ranges. `pratham404/H2-ssrf` merged (superseded by the broader fix).
- Real-engine smoke (`scripts/real_smoke.py`) caught that no authored course ever got `method_version = 1`: `/plan` answered 404 and the app fell back to the old lesson view, so the bout shell and gates were unreachable. `run_write` now stamps new courses at version 1 (regression test `a_newly_written_course_follows_the_learning_method`). Courses already on disk stay at 0.
- Visual e2e: the spec now pins the clock (Sunday, UTC) because the review forecast labels days from today; learn baselines refreshed for the Courses/Schedule/Profile nav. `make e2e` green (110 + 17).
- Known then, fixed on 2026-11-11: the `cargo deny` advisory (rusqlite bump) and `make okf` PyYAML lookup.
- Added LICENSE, SECURITY.md, CI, web check in the gate, `scripts/real_smoke.py`. Rescued untracked brain sections (alvar, huberman, gornik, learning-method).
- Open, owner-only: T9, G2, M14, G4 (real hour and retest).

## 2026-11-07 — S3 marketing + docs site (feat/site)
- Vite vanilla site in `site/`, pink skin on the shared kit, `graph` ASCII scene (pup + knowledge graph + synthetic forgetting curve). Review pass: pup rebuilt from ellipses, curve enlarged, pup mark in nav, repo-private note under the hero command, unsourced "3 GB" claim removed, platforms FAQ says the release workflow has not run on a tag. DESIGN.md added. Not done: six identical feature groups left as is; GitHub links stay while the repo is private.

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


## 2026-11-04 — Gornik Learning Brain Section
- Watched Aleks Gornik's *How I use AI to learn engineering (anti brain rot studying)* (youtu.be/GPM5MgFEkpk) via oembed metadata + full transcript fetched with `youtube-transcript-api` Python package (installed on host).
- Verified speaker background: EE graduate, internships in AI/hardware/software, now Member of Technical Staff at AI startup working on LLM training. Fetched website (aleksgornik.com) and kit.com blog posts.
- Identified 4 core methods: (1) Make concepts visual with AI-generated 3D models/interactive dashboards (Claude Code, GPT Codex), (2) Filter resources with AI prompts (syllabus + reading list → which chapters matter), (3) Find the essential 20% to get unstuck (graph neural network thesis story — broke deep learning down to "just get a training loop going in PyTorch"), (4) Find the best resources with AI sub-agents crawling the internet (found NASA Mars rover 3D models, satellite .stl files).
- Created [Gornik Learning](/gornik-learning/index.md) brain section with 3 documents: index (summary + how I watched + relevance-to-LearnPuppy + cross-reference table to Alvar Method and Huberman Learning), four-methods (detailed breakdown of all 4 methods with examples, prompts, and connections to the other two learning frameworks), and transcript (full auto-caption transcript with timestamps).
- Cross-referenced Gornik's methods to LearnPuppy (visualize ↔ lp-agent visual subagent, filter ↔ research subagent, 20% ↔ Probe phase, find resources ↔ lp-ingest transcript ingestor), Alvar Method (learn-visual, learn-verify, Probe phase, research subagent), and Huberman Learning (visual focus, self-testing, ~15% error rate, gap effects).

## 2026-11-04 — Huberman Learning Brain Section
- Watched Andrew Huberman's *Optimal Protocols for Studying & Learning* (youtu.be/ddq8JIMhz7c, Huberman Lab Podcast) via oembed metadata + full episode transcript from hubermanlab.com (105,393 bytes).
- Also fetched the *Essentials: Optimize Your Learning & Creativity* episode transcript (35,435 bytes) covering daily routine, circadian mechanisms, go/no-go circuits, and creativity.
- Fetched the *Neuroplasticity Super-Protocol* newsletter (7,514 bytes) with the condensed 9-step protocol.
- Cross-referenced with ai.hubermanlab.com summaries for structured study habit data.
- Created [Huberman Learning](/huberman-learning/index.md) brain section with 8 documents: index, neuroplasticity fundamentals (3 mechanisms, two-step process, sleep, NSDR), self-testing protocol (1917 study, SSSS/SSST/STTT, familiarity vs mastery, timing, 50% rule, test format), study habits (~700 med students: schedule/alone/phone-away/3-4hrs/teach-peers/aspirational), neuroplasticity super-protocol (9 steps), circadian optimization (autonomic arousal, go/no-go D1/D2, two daily bouts, creativity two-stage, morning/evening routines), advanced tools (gap effects, interleaving, emotion, ~15% error rate, cold exposure, skill levels), and transcript excerpts.
- Cross-referenced Huberman concepts to both LearnPuppy's crate architecture and the Alvar Method (e.g., self-testing ↔ quiz-after-every-step, gap effects ↔ potential lp-agent feature, interleaving ↔ mixed pass every third quiz, familiarity vs mastery ↔ mastery bar MC + free recall).

## 2026-11-07 — S3 Marketing + docs site (feat/site) and release workflow (feat/release)
- `site/`: Vite vanilla landing + docs on the shared kit (copied from bunfiiliate with `scripts/sync-kit.sh`), pink skin, `graph` ASCII scene (pixel pup, graph, forgetting curve labelled as a synthetic illustration), 11 docs pages, Pages workflow, smoke test.
- Install tabs lead with build-from-source. Binary installer needs a public repo and a first tag (`feat/release`: cargo-dist, asset names `lp-server-*`).
- Open: no LICENSE file (site says MIT from Cargo.toml); no SECURITY.md; several screen details were taken from `docs/web/06-screens.md`, not the React source.

