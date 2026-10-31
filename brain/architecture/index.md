# Architecture Index

Back to [Brain Index](/index.md). Related: [Overview](/overview.md), [Decisions Index](/decisions/index.md), [Task DAG](/tasks/index.md).

## Architectural Specs
- [System Architecture & Crate Topology](/architecture/system.md) — 7-crate hexagonal workspace (`lp-core`, `lp-store`, `lp-cli-driver`, `lp-agent`, `lp-ingest`, `lp-service`, `lp-server`), trait boundaries, and dependency stack.
- [Web UI Architecture (`lp-server` + `web/`)](/architecture/web-ui.md) — Loopback axum server embedding the Vite + React + shadcn SPA over `lp-service`, workspace-scoped URLs, OpenAPI contract.
- [Server API Summary](/architecture/server-api.md) — Endpoint groups, SSE, and conventions; authoritative list in `docs/web/03-server-api.md`.
- **Archived** (superseded by [ADR-0006](/decisions/0006-web-ui-over-dioxus.md)): [Dioxus 0.7 & `lp-ui` Visual Desktop Architecture](/architecture/dioxus-ui.md) — Native macOS desktop visual learning app (`lp-app`), custom `shadcn/ui` + `dioxus-primitives` Rust component crate (`lp-ui`), interactive SVG Knowledge Graph canvas, and Dioxus 0.7 rules/bans.
- [Core Domain Contracts & Algorithms (`lp-core`)](/architecture/core-domain.md) — Zero-IO domain types, 3-phase fold (`Probe -> Plan -> Teach`), Mermaid `plan_init` parser, `derive_plan_statuses`, `question-core`, and argument healing.
- [Hybrid Storage & Memory Model (`lp-store`)](/architecture/storage-memory.md) — Append-only JSONL ledgers + SQLite WAL + FTS5, atomic write-ahead durability, `ArcSwap` lock-free snapshots, `SmolStr` interning, and bounded `mpsc` channels.
- [Multi-Account CLI Auth Discovery & Quota Pool (`lp-cli-driver`)](/architecture/cli-auth-pool.md) — Auto-discovery of `claude` (multiple subscription accounts via `CLAUDE_CONFIG_DIR`), `codex`, and `agy`, rate-limit cooldown tracking, automatic failover, and `FauxEngine`.
- [Classroom & Async Workbench Engine (`lp-agent`)](/architecture/classroom-engine.md) — Stateless per-turn `ProberAgent` and `TeacherAgent`, `probe_done` handoff, background sub-agents (`research`, `pdf`, `visual`, `demo`), 3-permit semaphore, and deterministic `steer_<jobId>` wake pump.
- [Knowledge Graph & 3-Stage Extraction Pipeline (`lp-agent` + `lp-core`)](/architecture/knowledge-graph.md) — Chains-primary ledger model, message coverage fingerprints, digest-aware Extractor, Verdicts-Only Auditor, deterministic `merge_findings`, and transitive `simplify_chains`.
- [Adaptive Practice Sessions & FSRS v5 Engine](/architecture/practice-fsrs.md) — Interactive `PracticeAgent` (`problem`/`conclude`, `R0..R5` hint ladder, miss classification) and `tracker` deterministic FSRS v5 4-choice spaced-repetition engine.
- [Local CLI Transcript & Git Commit Ingestor (`lp-ingest`)](/architecture/transcript-ingest.md) — Incremental `mtime+size` parsers for Claude (all accounts), Codex, and Antigravity (`with_snapshot` SQLite copy), Git commit window matcher, decision distiller, and MC card quality gates.
- **Archived** (superseded by [ADR-0006](/decisions/0006-web-ui-over-dioxus.md)): [Headless CLI & Isolated Legacy TUI (`lp-tui`)](/architecture/tui.md) — Headless CLI subcommands (`auth`, `chat`, `graph`, `ingest`, `review`) and legacy Ratatui TUI isolated on branch `legacy/tui-cli`.
