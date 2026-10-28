# Task DAG & Worker Dispatch (`T00`–`T80`)

Back to [Brain Index](/index.md). Related: [Overview](/overview.md), [Architecture Index](/architecture/index.md), [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [Pre-Merge Gate](/wisdom/pre-merge-gate.md).

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
| `T50`–`T54` | Legacy Ratatui TUI & Headless CLI (`lp-tui`) — isolated on git branch `legacy/tui-cli` | `agy claude-sonnet-4-6 (high)` | `T31`, `T41` | [Headless CLI & Legacy TUI](/architecture/tui.md) | `done` |

---

## 3. Phase D — Native macOS Dioxus 0.7 + `lp-ui` Visual Desktop App (Active Execution Wave)

Designed for parallel multi-worktree execution via Orca (`orca worktree create --base-branch staging`):

| ID | Task | Disjoint Files Owned | Worker & Effort | Depends On | Spec Links | Status |
|---|---|---|---|---|---|---|
| `D00` | Scaffold `crates/lp-ui` and `crates/lp-app` in workspace `Cargo.toml` with `dioxus = "0.7"` (`desktop`, `ssr`) and module stubs so parallel workers have zero merge conflicts | `Cargo.toml`, `crates/lp-ui/**`, `crates/lp-app/**` | `codex gpt-5.4 (high)` | `T41` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md) | `pending` |
| `D10` | `lp-ui` Foundation: OKLCH Dark Theme CSS (`theme.rs`), 20+ Lucide SVG Icons (`icons.rs`), `Button`, `Badge`, `Card`, `Separator`, `Progress`, `MasteryRing`, `CodeBlock` + `dioxus-ssr` unit tests | `crates/lp-ui/src/{theme,icons,primitives,code_block}.rs` | `agy claude-sonnet-4-6 (high)` | `D00` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md) | `pending` |
| `D11` | `lp-ui` Interactive Primitives (Zero Inline-JS): `Dialog`, `Tabs`, `Tooltip`, `Slider`, `Switch`, `ScrollArea`, `SplitPane`, `CommandPalette` (`Cmd+K`) + `dioxus-ssr` unit tests | `crates/lp-ui/src/{dialog,tabs,overlays,command_palette}.rs` | `agy claude-sonnet-4-6 (high)` | `D00` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [Anti-Patterns](/wisdom/anti-patterns.md) | `pending` |
| `D20` | `lp-app` Desktop Shell, Async `DesktopBackend` Bridge (`lp-store` + `lp-cli-driver` + `lp-agent` + `lp-ingest`), Left Icon Rail, Quick-Action Toolbar, and Multi-Account CLI Modal (`Cmd+A`) | `crates/lp-app/src/{main,lib,state,backend,shell,views/accounts}.rs` | `codex gpt-5.4 (high)` | `D10`, `D11` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [Multi-Account CLI Pool](/architecture/cli-auth-pool.md) | `pending` |
| `D21` | `lp-app` Visual Classroom View: Session Drawer, Markdown/Code/Diagram Lesson Stream, Interactive Question Dock (`[A]..[D]`, `[I]`, `1..5` Confidence Slider, dual thinking tabs, graded verdict), Plan DAG SVG Pane, and Sub-Agent Workbench Cards | `crates/lp-app/src/views/classroom.rs` | `agy claude-sonnet-4-6 (high)` | `D20` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [Classroom Engine](/architecture/classroom-engine.md) | `pending` |
| `D22` | `lp-app` Interactive SVG Knowledge Graph Canvas & 3-Level Collapsible Chain Table: Zoom/Pan `<svg>` camera, cubic-bezier edges, cluster hulls, mastery rings, node provenance drawer, and multi-select Adaptive Practice launcher | `crates/lp-app/src/views/graph_canvas.rs` | `codex gpt-5.4 (high)` | `D20` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [Knowledge Graph](/architecture/knowledge-graph.md) | `pending` |
| `D23` | `lp-app` FSRS 4-Choice Visual Review Deck & Local CLI Transcript Decision Digest Browser (one-click "Fold into Graph", "Ingest Now", and `Again/Hard/Good/Easy` grading) | `crates/lp-app/src/views/digest_fsrs.rs` | `agy claude-sonnet-4-6 (high)` | `D20` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md), [Practice & FSRS](/architecture/practice-fsrs.md) | `pending` |
| `D30` | `tester-army/e2e` Setup (`e2e.config.ts`, `tests/*.e2e.ts`) + `LP_E2E_PORT` Dual-Mode Web Harness + Rust E2E Integration Suite (`crates/lp-app/tests/e2e_desktop.rs`) | `e2e.config.ts`, `tests/**`, `crates/lp-app/tests/**` | `agy gemini-3.1-pro-high (high)` | `D21`, `D22`, `D23` | [Pre-Merge Gate](/wisdom/pre-merge-gate.md) | `pending` |
| `D60` | Full `make ci`, `tester-army/e2e` execution, `alibaba/open-code-review` (`ocr`) diff/scan audit, `Claude Opus 4.6 Thinking High` final sign-off, and fast-forward `prod` to `staging` | All crates + `brain/log.md` | `agy claude-opus-4-6-thinking (high)` | `D30` | [Pre-Merge Gate](/wisdom/pre-merge-gate.md) | `pending` |

---

## 4. Future Cross-Platform Phases (`TODO` — Scheduled After macOS Release)

| ID | Task | Target Platform | Worker & Effort | Depends On | Spec Links | Status |
|---|---|---|---|---|---|---|
| `T70` | **Phase W (Next)**: Windows Desktop Packaging (`x86_64-pc-windows-msvc`), WebView2 installer bundle, and Windows `%APPDATA%` / `%USERPROFILE%\.claude` path discovery | Windows 11 Desktop | `codex gpt-5.4 (high)` | `D60` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md) | `pending` |
| `T80` | **Phase M (Last)**: iOS & Android Mobile Companion (`dioxus` `mobile` feature), touch gesture Graph Canvas, and LAN/folder `sync-<device>.jsonl` + `nodes.jsonl` replication for mobile FSRS review | iOS & Android | `agy claude-sonnet-4-6 (high)` | `T70` | [Dioxus 0.7 & lp-ui Architecture](/architecture/dioxus-ui.md) | `pending` |
