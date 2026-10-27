# Architecture Index

Back to [Brain Index](/index.md). Related: [Overview](/overview.md), [Decisions Index](/decisions/index.md), [Task DAG](/tasks/index.md).

## Architectural Specs
- [System Architecture & Crate Topology](/architecture/system.md) — 6-crate hexagonal workspace, trait boundaries (`LlmEngine`, `ChatStore`, `GraphStore`, `JobStore`, `TrackerStore`), and curated dependency stack.
- [Core Domain Contracts & Algorithms (`lp-core`)](/architecture/core-domain.md) — Zero-IO domain types, 3-phase fold (`Probe -> Plan -> Teach`), Mermaid `plan_init` parser, `derive_plan_statuses`, `question-core`, and argument healing.
- [Hybrid Storage & Memory Model (`lp-store`)](/architecture/storage-memory.md) — Append-only JSONL ledgers + SQLite WAL + FTS5, atomic write-ahead durability, `ArcSwap` lock-free TUI snapshots, `SmolStr` interning, and bounded `mpsc` channels.
- [Multi-Account CLI Auth Discovery & Quota Pool (`lp-cli-driver`)](/architecture/cli-auth-pool.md) — Auto-discovery of `claude` (multiple subscription accounts via `CLAUDE_CONFIG_DIR`), `codex`, and `agy`, rate-limit cooldown tracking, automatic failover, and `FauxEngine`.
- [Classroom & Async Workbench Engine (`lp-agent`)](/architecture/classroom-engine.md) — Stateless per-turn `ProberAgent` and `TeacherAgent`, `probe_done` handoff, background sub-agents (`research`, `pdf`, `visual`, `demo`), 3-permit semaphore, and deterministic `steer_<jobId>` wake pump.
- [Knowledge Graph & 3-Stage Extraction Pipeline (`lp-agent` + `lp-core`)](/architecture/knowledge-graph.md) — Chains-primary ledger model, message coverage fingerprints, digest-aware Extractor, Verdicts-Only Auditor, deterministic `merge_findings`, and transitive `simplify_chains`.
- [Adaptive Practice Sessions & FSRS v5 Engine](/architecture/practice-fsrs.md) — Interactive `PracticeAgent` (`problem`/`conclude`, `R0..R5` hint ladder, miss classification) and `tracker` deterministic FSRS v5 4-choice spaced-repetition engine.
- [Local CLI Transcript & Git Commit Ingestor (`lp-ingest`)](/architecture/transcript-ingest.md) — Incremental `mtime+size` parsers for Claude (all accounts), Codex, and Antigravity (`with_snapshot` SQLite copy), Git commit window matcher, decision distiller, and MC card quality gates.
- [60fps Ratatui TUI & Headless CLI (`lp-tui`)](/architecture/tui.md) — Classroom split view, Question Dock, Braille force-directed Graph Canvas, 3-level Chain Table, FSRS Digest view, and CLI subcommands.
