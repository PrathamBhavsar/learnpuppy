# Task DAG & Worker Dispatch

Back to [Brain Index](/index.md). Related: [Overview](/overview.md), [Architecture Index](/architecture/index.md), [ADR-0006 Web UI](/decisions/0006-web-ui-over-dioxus.md), [Pre-Merge Gate](/wisdom/pre-merge-gate.md).

## 0. Web UI Rollout, done (`V00`–`A70`, [ADR-0006](/decisions/0006-web-ui-over-dioxus.md))

The authoritative DAG, owned files and done-when criteria are in `docs/web/08-task-dag.md`; this table only tracks status. All rows were audited against `docs/web/10-acceptance.md` on 2026-11-01 (`A70`, punch list zero P0/P1). Branch flow: `staging <- pratham404/<task-id>` (`--no-ff`, `make gate` first).

| ID | Task | Depends On | Status |
|---|---|---|---|
| `V00` | Verify shadcn preset + disk plan (`docs/web/04-preset-facts.md`) | — | `done` |
| `V01` | Repo surgery: delete `lp-ui`, `lp-app`, `lp-tui`, Dioxus `tests/`, root e2e configs, `scripts/export-icons.ts` | `V00` | `done` |
| `V02` | `lp-service` (ported logic + tests, `paths`) and `lp-server` skeleton (`learnpuppy --version`, subcommands) | `V01` | `done` |
| `F00` | `web/` Vite + React + shadcn scaffold, tokens, shell, gallery | `V00` | `done` |
| `B10` | Workspaces core: registry, meta, trash, migration, export/import | `V02` | `done` |
| `B11` | Server core: auth, errors, SSE hub, OpenAPI, assets, lock, shutdown | `V02` | `done` |
| `B20` | Read APIs: workspaces, graph, chats, review, digest, accounts, settings, search, trash | `B10`, `B11` | `done` |
| `F10` | API client, SSE manager, MSW, routing, palette, hotkeys, toasts | `F00`, `B11` | `done` |
| `B21` | Chat engine: turns, view, answers, jobs, uploads, recovery | `B20` | `done` |
| `B22` | Graph mutate/search/layout cache, ingest run, export/import jobs | `B20` | `done` |
| `F20`–`F24` | Feature screens: workspaces, graph canvas, classroom, review + digest, ingest/accounts/settings | `F10` (+ `B21`/`B22`) | `done` |
| `P30` | Motion + polish pass | `F20`–`F24` | `done` |
| `Q40` | a11y + RTL + perf pass | `P30` | `done` |
| `E50` | Full Playwright E2E + visual baselines | `Q40` | `done` |
| `D60` | Docs/brain/CLAUDE.md sweep + legacy cleanup | `E50` | `done` |
| `A70` | Opus audit vs `docs/web/10-acceptance.md` | `D60` | `done` |

## 0a. Cheap tutor (`T0`–`T9`, [plan](/cheap-tutor/index.md), [ADR-0007](/decisions/0007-course-as-data.md))

| ID | Task | Status |
|---|---|---|
| `T0`–`T8` | Driver fixes, core types and gates, store, author/placer, service, routes, Course screen, FSRS cards, meter and budget | `done` |
| `G1` | Benchmark gate: Opus low effort is the default Author | `done` |
| `T9` | Real hour test by the owner | `owner`: runbook in [Real Hour](/learning-method/real-hour.md) |
| `G2` | Owner verdict: did you learn? | `owner` |

## 0b. Learning method (`M0`–`M14`, [plan](/learning-method/index.md), [ADR-0008](/decisions/0008-learning-method.md))

| ID | Task | Status |
|---|---|---|
| `M0` | ADR-0008 | `done` (2026-11-10) |
| `M1`–`M13` | Schema and mastery, runner and gates, bout and pacing, store, prompts and grading, course and learning services, routes, lesson steps UI, bout UI, close, schedule and profile, simulated bout | `done` |
| `M10b` | Bout UI integration: `BoutShell` wired into the course session, server refuses `step` without an open bout, settings route and screen | `done` (2026-11-10) |
| `G3` | Richer-schema benchmark, see [G3 result](/learning-method/g3-result.md) | `done` |
| `M14` | Real bout by the owner: pre-test, post-test, one-week retest | `owner`: runbook in [Real Hour](/learning-method/real-hour.md) |
| `G4` | Did it teach better? | `owner` |

---

## 1. Exact Model Capabilities & Effort Matrix (`claude`, `codex`, `agy`)

Every task in the DAG is assigned to the model and CLI tool whose strengths best match the engineering domain:

| Role | CLI Tool | Exact Model ID | Effort Level | Why This Model Is Best Here |
|---|---|---|---|---|
| **Chief Planner & Final Sign-Off Auditor** | `agy` / `claude` | `claude-opus-4-6-thinking` (`opus`) | `high` (`max` on `D60`) | Deepest cross-crate architectural reasoning, subtle state-machine/deadlock detection, and strict OKF + parity verification. |
| **Rust Systems, Math & Cargo Architect** | `codex` | `gpt-5.4` / `gpt-6-luna` (`-c model_reasoning_effort="high"`) | `high` | Best at pure algorithmic Rust (`lp-core`), SQLite WAL/FTS5 (`lp-store`), Cargo dependency resolution (`D00`), and deterministic math/SVG coordinate geometry (`D22`). |
| **Dioxus 0.7 UI & Stateful Feature Engineer** | `agy` / `claude` | `claude-sonnet-4-6` (`sonnet`) | `high` | Best at ergonomic UI component APIs (`lp-ui` `D10`, `D11`), rich visual layouts (`lp-app` `D20`, `D21`, `D23`), CSS/OKLCH tokens, and stateful agent flows. |
| **E2E Harness, Subprocess & Code Review Specialist** | `agy` | `gemini-3.1-pro-high` | `high` | Fast, thorough execution of `tester-army/e2e` suites, `alibaba/open-code-review` (`ocr`) scans, and multi-account CLI subprocess verification (`D30`, `D60`). |

---

## 2. Completed Foundation & Backend Crates (`T00`–`T54`, Preserved on `legacy/tui-cli` & `staging`)

| ID | Task | Worker | Depends On | Spec Links | Status |
|---|---|---|---|---|---|
| `T00-audit` | Audit `docs/PLAN.md`, `docs/REVERSE_ENGINEERING.md`, and `brain/` OKF bundle | `claude-opus-4-6-thinking (high)` | — | [Overview](/overview.md), [System Architecture](/architecture/system.md) | `done` |
| `T00` | Initialize Cargo workspace (`lp-core`, `lp-store`, `lp-cli-driver`, `lp-agent`, `lp-ingest`, `lp-tui`) | `codex gpt-5.4 (high)` | `T00-audit` | [System Architecture](/architecture/system.md) | `done` |
| `T01` | Add `Makefile`, `deny.toml` (`cargo-deny`), `.gitignore`, and OKF validation gate | `agy gemini-3.1-pro-high` | `T00` | [Pre-Merge Gate](/wisdom/pre-merge-gate.md) | `done` |
| `T10` | `lp-core` Knowledge Graph contracts, `merge_findings`, `simplify_chains`, `birth_session_partition`, ForceAtlas2 layout (`proptest`) | `codex gpt-5.4 (high)` | `T01` | [Knowledge Graph](/architecture/knowledge-graph.md) | `done` |
| `T11` | `lp-core` Mermaid `plan_init` DAG parser, `derive_plan_statuses`, `phase_at_3`, `question_core`, Fisher-Yates shuffle, `argument_heal`, `PracticeGate` | `codex gpt-5.4 (high)` | `T01` | [Core Domain](/architecture/core-domain.md) | `done` |
| `T12` | `lp-core` FSRS v5 pure scheduler & deterministic replay, `validate_card_shape`, 20k-token compaction cutfinder, cost/usage folds | `codex gpt-5.4 (high)` | `T01` | [Practice & FSRS](/architecture/practice-fsrs.md) | `done` |
| `T20` | `lp-store` Append-only JSONL ledgers + SQLite WAL (`index.db`) + FTS5 (`propositions_fts`, `transcripts_fts`) + `claim_steers` + `sync-<device>.jsonl` | `codex gpt-5.4 (high)` | `T10`, `T11`, `T12` | [Storage & Memory Model](/architecture/storage-memory.md) | `done` |
| `T21` | `lp-cli-driver` Multi-account CLI auth discovery (`claude` 1..N via `CLAUDE_CONFIG_DIR`, `codex`, `agy`) + `AccountPool` quota/cooldown failover | `agy claude-sonnet-4-6 (high)` | `T12` | [Multi-Account CLI Pool](/architecture/cli-auth-pool.md) | `done` |
| `T22` | `lp-cli-driver` Sandboxed subprocess streaming runner (`LlmEngine`), `--json-schema` structured runner, BYOK fallback, and `FauxEngine` | `agy claude-sonnet-4-6 (high)` | `T21` | [Multi-Account CLI Pool](/architecture/cli-auth-pool.md), [Time Traps](/wisdom/time-traps.md) | `done` |
| `T30` | `lp-agent` Stateless per-turn `ProberAgent`, `TeacherAgent`, and `PracticeAgent` loops, embedded prompt fragments, `probe_done` handoff, compaction | `agy claude-sonnet-4-6 (high)` | `T20`, `T22` | [Classroom Engine](/architecture/classroom-engine.md), [Practice & FSRS](/architecture/practice-fsrs.md) | `done` |
| `T31` | `lp-agent` Async Workbench Sub-Agents (`researcher`, `pdf_reader`, `visual_maker`, `demo_builder`), 3-permit semaphore, and Wake Pump | `agy claude-sonnet-4-6 (high)` | `T30` | [Classroom Engine](/architecture/classroom-engine.md) | `done` |
| `T32` | `lp-agent` 3-Stage Knowledge Graph Extraction Pipeline (`Extractor -> Verdicts-Only Auditor -> Merger`) + Markdown/PDF file import | `agy claude-sonnet-4-6 (high)` | `T30` | [Knowledge Graph](/architecture/knowledge-graph.md) | `done` |
| `T40` | `lp-ingest` Incremental `mtime+size` parsers for Claude (all accounts), Codex, AGY (`with_snapshot` SQLite copy), and Git commit/diff window matcher | `codex gpt-5.4 (high)` | `T20`, `T22` | [Transcript Ingestor](/architecture/transcript-ingest.md) | `done` |
| `T41` | `lp-ingest` Decision distiller (`DecisionRecord` -> Graph bridge), 4-choice MC card generator, book chunker, and `/Users/pratham/code/tracker` seed | `agy claude-sonnet-4-6 (high)` | `T32`, `T40` | [Transcript Ingestor](/architecture/transcript-ingest.md), [Practice & FSRS](/architecture/practice-fsrs.md) | `done` |
| `T50`–`T54` | Legacy Ratatui TUI & Headless CLI (`lp-tui`) — isolated on git branch `legacy/tui-cli`; crate deleted from `staging` by `V01` | `agy claude-sonnet-4-6 (high)` | `T31`, `T41` | [Headless CLI & Legacy TUI](/architecture/tui.md) | `done` |

---

## 3. Phase D — Native macOS Dioxus 0.7 + `lp-ui` Visual Desktop App (Archived)

> Superseded by [ADR-0006](/decisions/0006-web-ui-over-dioxus.md): `lp-ui` and `lp-app` were deleted by `V01`. Kept as history.

Designed for parallel multi-worktree execution via Orca (`orca worktree create --base-branch staging`):

| ID | Task | Disjoint Files Owned | Worker & Effort | Depends On | Spec Links | Status |
|---|---|---|---|---|---|---|
| `D00` | Scaffold `crates/lp-ui` and `crates/lp-app` in workspace `Cargo.toml` with `dioxus = "0.7"` (`desktop`, `ssr`) and module stubs so parallel workers have zero merge conflicts | `Cargo.toml`, `crates/lp-ui/**`, `crates/lp-app/**` | `codex gpt-5.4 (high)` | `T41` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md) | `done` |
| `D10` | `lp-ui` Foundation: OKLCH Dark Theme CSS (`theme.rs`), 20+ Lucide SVG Icons (`icons.rs`), `Button`, `Badge`, `Card`, `Separator`, `Progress`, `MasteryRing`, `CodeBlock` + `dioxus-ssr` unit tests | `crates/lp-ui/src/{theme,icons,primitives,code_block}.rs` | `agy claude-sonnet-4-6 (high)` | `D00` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md) | `done` |
| `D11` | `lp-ui` Interactive Primitives (Zero Inline-JS): `Dialog`, `Tabs`, `Tooltip`, `Slider`, `Switch`, `ScrollArea`, `SplitPane`, `CommandPalette` (`Cmd+K`) + `dioxus-ssr` unit tests | `crates/lp-ui/src/{dialog,tabs,overlays,command_palette}.rs` | `agy claude-sonnet-4-6 (high)` | `D00` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [Anti-Patterns](/wisdom/anti-patterns.md) | `done` |
| `D20` | `lp-app` Desktop Shell, Async `DesktopBackend` Bridge (`lp-store` + `lp-cli-driver` + `lp-agent` + `lp-ingest`), Left Icon Rail, Quick-Action Toolbar, and Multi-Account CLI Modal (`Cmd+A`) | `crates/lp-app/src/{main,lib,state,backend,shell,views/accounts}.rs` | `codex gpt-5.4 (high)` | `D10`, `D11` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [Multi-Account CLI Pool](/architecture/cli-auth-pool.md) | `done` |
| `D21` | `lp-app` Visual Classroom View: Session Drawer, Markdown/Code/Diagram Lesson Stream, Interactive Question Dock (`[A]..[D]`, `[I]`, `1..5` Confidence Slider, dual thinking tabs, graded verdict), Plan DAG SVG Pane, and Sub-Agent Workbench Cards | `crates/lp-app/src/views/classroom.rs` | `agy claude-sonnet-4-6 (high)` | `D20` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [Classroom Engine](/architecture/classroom-engine.md) | `done` |
| `D22` | `lp-app` Interactive SVG Knowledge Graph Canvas & 3-Level Collapsible Chain Table: Zoom/Pan `<svg>` camera, cubic-bezier edges, cluster hulls, mastery rings, node provenance drawer, and multi-select Adaptive Practice launcher | `crates/lp-app/src/views/graph_canvas.rs` | `codex gpt-5.4 (high)` | `D20` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [Knowledge Graph](/architecture/knowledge-graph.md) | `done` |
| `D23` | `lp-app` FSRS 4-Choice Visual Review Deck & Local CLI Transcript Decision Digest Browser (one-click "Fold into Graph", "Ingest Now", and `Again/Hard/Good/Easy` grading) | `crates/lp-app/src/views/digest_fsrs.rs` | `agy claude-sonnet-4-6 (high)` | `D20` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [Practice & FSRS](/architecture/practice-fsrs.md) | `done` |
| `D30` | `tester-army/e2e` Setup (`e2e.config.ts`, `tests/*.e2e.ts`) + `LP_E2E_PORT` Dual-Mode Web Harness + Rust E2E Integration Suite (`crates/lp-app/tests/e2e_desktop.rs`) | `e2e.config.ts`, `tests/**`, `crates/lp-app/tests/**` | `agy gemini-3.1-pro-high (high)` | `D21`, `D22`, `D23` | [Pre-Merge Gate](/wisdom/pre-merge-gate.md) | `done` |
| `D60` | Full `make ci`, `tester-army/e2e` execution, `alibaba/open-code-review` (`ocr`) diff/scan audit, `Claude Opus 4.6 Thinking High` final sign-off, and fast-forward `prod` to `staging` | All crates + `brain/log.md` | `agy claude-opus-4-6-thinking (high)` | `D30` | [Pre-Merge Gate](/wisdom/pre-merge-gate.md) | `done` |

---

## 4. Cross-Platform Dioxus Phases (Cancelled)

> Cancelled by [ADR-0006](/decisions/0006-web-ui-over-dioxus.md); a native shell can wrap the web app later.

| ID | Task | Target Platform | Worker & Effort | Depends On | Spec Links | Status |
|---|---|---|---|---|---|---|
| `T70` | **Phase W (Next)**: Windows Desktop Packaging (`x86_64-pc-windows-msvc`), WebView2 installer bundle, and Windows `%APPDATA%` / `%USERPROFILE%\.claude` path discovery | Windows 11 Desktop | `codex gpt-5.4 (high)` | `D60` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md) | `cancelled` |
| `T80` | **Phase M (Last)**: iOS & Android Mobile Companion (`dioxus` `mobile` feature), touch gesture Graph Canvas, and LAN/folder `sync-<device>.jsonl` + `nodes.jsonl` replication for mobile FSRS review | iOS & Android | `agy claude-sonnet-4-6 (high)` | `T70` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md) | `cancelled` |
| `S3` | **Site**: Vite marketing + docs site in `site/` (branch `feat/site`, done, merged). Release workflow on `feat/release`. |

