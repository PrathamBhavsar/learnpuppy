# Task DAG & Worker Dispatch (`T00`–`T60`)

Back to [Brain Index](/index.md). Related: [Overview](/overview.md), [Architecture Index](/architecture/index.md), [Pre-Merge Gate](/wisdom/pre-merge-gate.md).

## Orchestration Model (`/orchestration` + `/ponytail`)

- **Chief Planner & Auditor**: **1x Claude Opus High** (`claude -p --model opus --effort high` / `claude-opus-4-6-thinking`) — audits the plan in `T00-audit` and signs off in `T60`.
- **Implementation Workers**: Spawned via `orca spawn` on isolated git worktrees (`/Users/pratham/code/learnpuppy-wt-<ID>`) branched off `staging`:
  - **Codex (`gpt-5.4`)**: Pure algorithmic core (`T10`, `T11`, `T12`) and hybrid storage engine (`T20`).
  - **Claude Sonnet 4.6**: Stateful agent loops (`T22`, `T30`, `T31`, `T32`), decision/card distillation (`T41`), and 60fps Ratatui TUI (`T50`, `T51`, `T52`, `T53`).
  - **Antigravity (`agy`)**: Workspace & CI scaffold (`T00`, `T01`), multi-account CLI discovery (`T21`), transcript ingestor (`T40`), and E2E/OCR gates (`T60`).

---

## Phase 0 — Plan Audit & Workspace Scaffold

| ID | Task | Worker | Depends On | Spec Links | Status |
|---|---|---|---|---|---|
| `T00-audit` | Audit `docs/PLAN.md`, `docs/REVERSE_ENGINEERING.md`, and `brain/` OKF bundle for edge cases and parity | `Claude Opus High` | — | [Overview](/overview.md), [System Architecture](/architecture/system.md) | `pending` |
| `T00` | Initialize 6-crate Cargo workspace (`lp-core`, `lp-store`, `lp-cli-driver`, `lp-agent`, `lp-ingest`, `lp-tui`) | `Codex` / `AGY` | `T00-audit` | [System Architecture](/architecture/system.md) | `pending` |
| `T01` | Add `Makefile`, `deny.toml` (`cargo-deny`), `.gitignore`, and OKF validation gate | `AGY` | `T00` | [Pre-Merge Gate](/wisdom/pre-merge-gate.md) | `pending` |

## Phase 1 — Pure Domain Core (`lp-core`, Zero IO)

| ID | Task | Worker | Depends On | Spec Links | Status |
|---|---|---|---|---|---|
| `T10` | `lp-core` Knowledge Graph contracts, `merge_findings`, `simplify_chains`, `birth_session_partition`, ForceAtlas2 layout (`proptest`) | `Codex` | `T01` | [Knowledge Graph](/architecture/knowledge-graph.md) | `pending` |
| `T11` | `lp-core` Mermaid `plan_init` DAG parser, `derive_plan_statuses`, `phase_at_3`, `question_core`, Fisher-Yates shuffle, `argument_heal`, `PracticeGate` | `Codex` | `T01` | [Core Domain](/architecture/core-domain.md) | `pending` |
| `T12` | `lp-core` FSRS v5 pure scheduler & deterministic replay, `validate_card_shape`, 20k-token compaction cutfinder, cost/usage folds | `Codex` | `T01` | [Practice & FSRS](/architecture/practice-fsrs.md) | `pending` |

## Phase 2 — Hybrid Storage & Multi-Account CLI Driver (`lp-store`, `lp-cli-driver`)

| ID | Task | Worker | Depends On | Spec Links | Status |
|---|---|---|---|---|---|
| `T20` | `lp-store` Append-only JSONL ledgers + SQLite WAL (`index.db`) + FTS5 (`propositions_fts`, `transcripts_fts`) + `claim_steers` + `sync-<device>.jsonl` | `Codex` | `T10`, `T11`, `T12` | [Storage & Memory Model](/architecture/storage-memory.md) | `pending` |
| `T21` | `lp-cli-driver` Multi-account CLI auth discovery (`claude` 1..N via `CLAUDE_CONFIG_DIR`, `codex`, `agy`) + `AccountPool` quota/cooldown failover | `AGY` / `Sonnet` | `T12` | [Multi-Account CLI Pool](/architecture/cli-auth-pool.md) | `pending` |
| `T22` | `lp-cli-driver` Sandboxed subprocess streaming runner (`LlmEngine`), `--json-schema` structured runner, BYOK fallback, and `FauxEngine` | `Sonnet` | `T21` | [Multi-Account CLI Pool](/architecture/cli-auth-pool.md), [Time Traps](/wisdom/time-traps.md) | `pending` |

## Phase 3 — Agent Pipelines (`lp-agent`)

| ID | Task | Worker | Depends On | Spec Links | Status |
|---|---|---|---|---|---|
| `T30` | `lp-agent` Stateless per-turn `ProberAgent`, `TeacherAgent`, and `PracticeAgent` loops, embedded prompt fragments, `probe_done` handoff, compaction | `Sonnet` | `T20`, `T22` | [Classroom Engine](/architecture/classroom-engine.md), [Practice & FSRS](/architecture/practice-fsrs.md) | `pending` |
| `T31` | `lp-agent` Async Workbench Sub-Agents (`researcher`, `pdf_reader`, `visual_maker`, `demo_builder`), 3-permit semaphore, and Wake Pump | `Sonnet` | `T30` | [Classroom Engine](/architecture/classroom-engine.md) | `pending` |
| `T32` | `lp-agent` 3-Stage Knowledge Graph Extraction Pipeline (`Extractor -> Verdicts-Only Auditor -> Merger`) + Markdown/PDF file import | `Sonnet` | `T30` | [Knowledge Graph](/architecture/knowledge-graph.md) | `pending` |

## Phase 4 — Local CLI Transcript & Codebase Learning (`lp-ingest`)

| ID | Task | Worker | Depends On | Spec Links | Status |
|---|---|---|---|---|---|
| `T40` | `lp-ingest` Incremental `mtime+size` parsers for Claude (all accounts), Codex, AGY (`with_snapshot` SQLite copy), and Git commit/diff window matcher | `AGY` / `Codex` | `T20`, `T22` | [Transcript Ingestor](/architecture/transcript-ingest.md) | `pending` |
| `T41` | `lp-ingest` Decision distiller (`DecisionRecord` -> Graph bridge), 4-choice MC card generator, book chunker, and `/Users/pratham/code/tracker` seed | `Sonnet` | `T32`, `T40` | [Transcript Ingestor](/architecture/transcript-ingest.md), [Practice & FSRS](/architecture/practice-fsrs.md) | `pending` |

## Phase 5 — 60fps Ratatui TUI & Headless CLI (`lp-tui`)

| ID | Task | Worker | Depends On | Spec Links | Status |
|---|---|---|---|---|---|
| `T50` | `lp-tui` 60fps event loop, `ArcSwap` view state, `Ctrl+K` palette, `Ctrl+A` Multi-Account & Slots modal, and `clap` headless CLI subcommands | `Sonnet` | `T31`, `T41` | [TUI Architecture](/architecture/tui.md) | `pending` |
| `T51` | `lp-tui` Classroom View (streaming Markdown/`syntect`/Unicode math, Question Dock with confidence & dual thinking lanes, Plan DAG split pane, TUI Demo modal) | `Sonnet` | `T50` | [TUI Architecture](/architecture/tui.md), [Classroom Engine](/architecture/classroom-engine.md) | `pending` |
| `T52` | `lp-tui` Interactive Braille Force-Directed Graph Canvas (`Marker::Braille`) and 3-Level Collapsible Chain Table (`Cluster -> Chain -> Statements`) | `Sonnet` | `T50` | [TUI Architecture](/architecture/tui.md), [Knowledge Graph](/architecture/knowledge-graph.md) | `pending` |
| `T53` | `lp-tui` Tracker CLI Decision Digest browser & FSRS 4-Choice Spaced-Repetition Quiz UI (`insta` snapshot suite) | `Sonnet` | `T50` | [TUI Architecture](/architecture/tui.md), [Practice & FSRS](/architecture/practice-fsrs.md) | `pending` |

## Phase 6 — End-to-End Verification, Code Review & Sign-Off

| ID | Task | Worker | Depends On | Spec Links | Status |
|---|---|---|---|---|---|
| `T60` | Full `make ci`, PTY E2E suite (`tester-army/e2e`), `alibaba/open-code-review` (`ocr`) audit, Claude Opus High sign-off, and `prod` release | `AGY` + `Claude Opus High` | `T51`, `T52`, `T53` | [Pre-Merge Gate](/wisdom/pre-merge-gate.md) | `pending` |
