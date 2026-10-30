# CLAUDE.md

LearnPuppy (`learnpuppy`) — local learning web app: visual learning harness, cross-session Knowledge Graph, and FSRS spaced repetition, with multiple isolated workspaces. `learnpuppy serve` (Rust, axum) runs on `127.0.0.1` and serves a Vite + React + shadcn SPA from `web/` (ADR-0006, plan in `docs/web/`). The old Dioxus desktop app and Ratatui TUI are gone; TUI history is on branch `legacy/tui-cli`.

## Project map

- `brain/` — OKF v0.1 knowledge bundle (cold-start orientation, architecture specs, ADRs, guardrails, and task DAG). Read `brain/index.md` first.
- `docs/web/` — The web UI plan (decisions, architecture, workspaces, API, screens, testing, task DAG). Read `docs/web/README.md` first; it is the spec.
- `docs/` — Original execution plan (`docs/PLAN.md`) and clean-room reverse-engineering spec (`docs/REVERSE_ENGINEERING.md`).
- `crates/lp-core/` — Pure synchronous domain core (Graph merger, `simplify_chains`, ForceAtlas2 layout, Mermaid `plan_init` DAG parser, 3-phase state machine, FSRS v5 scheduler, port traits). Zero IO, zero async.
- `crates/lp-store/` — Hybrid append-only JSONL ledgers (`nodes.jsonl`, `chains.jsonl`, `coverage.jsonl`, `sync-<device>.jsonl`) + SQLite WAL (`index.db`) with FTS5 full-text search.
- `crates/lp-cli-driver/` — Zero-config local CLI auth auto-discovery (`claude` multi-account pool via `CLAUDE_CONFIG_DIR`, `codex`, `agy`), rate-limit failover scheduler, sandboxed subprocess runner, and `FauxEngine`.
- `crates/lp-agent/` — Stateless per-turn `ProberAgent`, `TeacherAgent`, `PracticeAgent`, async workbench sub-agents (`research`, `pdf`, `visual`, `demo`), Wake pump, and 3-stage `Extractor -> Auditor -> Merger`.
- `crates/lp-ingest/` — Incremental `mtime+size` ingestor for local Claude (all accounts), Codex, and Antigravity (`with_snapshot` SQLite copy) transcripts + Git commits/diffs, decision distiller, and 4-choice FSRS card generator.
- `crates/lp-service/` — App logic with no HTTP and no UI types: `settings` (profiles, pins), `engine` (`CliPoolEngine` role routing), `chat` (`run_to_rest`, `classroom_view`, answers, verdicts), `review` (FSRS due/grade/sync/streak), `digest`, `graph`, `accounts`, `ingest`, `paths` (data root, `workspaces/`, `state.json`).
- `crates/lp-server/` — Binary `learnpuppy` (`serve`, `migrate`, `seed`, `doctor`, `openapi`); axum + SSE + OpenAPI + embedded SPA, built out per `docs/web/08-task-dag.md`.
- `web/` — Vite + React + TypeScript + shadcn SPA (bun only).
- `.claude/skills/` & `.agents/skills/` — Installed project skills (`rpi`, `show-me`, `visual-pr`, `improve-claude-md`, `build-iterated-agentic-loop`, `design-control-loop`, `e2e`, `create-verification-skill`, `orca-model-matrix`; `dioxus-desktop-ui` is archived with the Dioxus crates).

<important if="you need to run commands to build, test, lint, validate OKF, or run E2E and code review">

Always prepend `/opt/homebrew/opt/rustup/bin` to `PATH` or run via `make`:

| Command | What it does |
|---|---|
| `make gate` | Run `fmt`, `clippy -D warnings`, `test --workspace`, and strict OKF validation (`brain/`) |
| `make ci` | Run `make gate` + `cargo deny check` |
| `make okf` | Validate `brain/` OKF knowledge bundle with `--strict` (0 errors, 0 warnings) |
| `cargo check --workspace` | Fast typecheck of all 7 crates |
| `cargo run -p lp-server -- --version` | Build and run the `learnpuppy` binary |
| `cargo test --workspace --all-targets` | Run all unit, `proptest`, and integration tests across the workspace |
| `ocr review` / `ocr scan` | Run Alibaba `open-code-review` (`ocr`) static + LLM diff audit |
</important>

<important if="you are working on crates/lp-service or crates/lp-server">

- Follow `docs/web/` exactly (00 decisions, 01 architecture, 02 workspaces, 03 server API); they are decisions, not suggestions.
- `lp-service` has no HTTP and no UI types; `lp-server` maps its plain structs to DTOs. Never duplicate domain logic (FSRS, merge, layout, plan statuses, option shuffle/verdict) in TypeScript.
- Every workspace root is passed to the `lp-store` constructors (`Stores::open(root)`); never hardcode `~/.learnpuppy` (use `lp_service::paths`).
- Run blocking store/filesystem work in `tokio::task::spawn_blocking`; no `unwrap()` in server routes.
- Bind `127.0.0.1` only; never expose OAuth tokens or keychain data in responses.
</important>

<important if="you are working on crates/lp-core">

- `lp-core` must remain 100% pure synchronous Rust (`unsafe_code = "forbid"`, zero `tokio`, zero `rusqlite`, zero `std::fs`).
- `PlanTree` stores structure only; node statuses (`Current`, `Visited`, `Upcoming`) are derived at render time from `quiz`/`explain` tool calls via `derive_plan_statuses`.
- `nodes.jsonl` and `chains.jsonl` are the sole stored graph truth; edges and implication junctions (`arg_...`) are derived via `derive_graph_snapshot` after `simplify_chains`.
</important>

<important if="you are working on CLI authentication or subprocess execution in crates/lp-cli-driver">

- Never invoke `security find-generic-password -w` (triggers a blocking macOS GUI keychain dialog).
- Discover every Claude subscription profile (`~/.claude`, `~/.claude-accounts/*`, `~/.claude-work`, custom `CLAUDE_CONFIG_DIR`) by reading `<dir>/.claude.json` (`oauthAccount.accountUuid`, `emailAddress`, `seatTier`) and treat each as an independent rate-limit bucket in `AccountPool`.
- Spawn `claude`, `codex`, and `agy` subprocesses with `kill_on_drop(true)` in an isolated empty `/tmp/learnpuppy-sandbox` directory with `CLAUDECODE`, `CLAUDE_CODE_ENTRYPOINT`, and `ORCA_*` env vars stripped.
</important>

<important if="you are working on transcript ingestion in crates/lp-ingest">

- Never open `~/.gemini/antigravity-cli/conversation_summaries.db` in place; always copy `.db`, `-wal`, and `-shm` to a `TempDir` snapshot first (`with_snapshot`).
- Reuse a single line buffer (`BufReader::read_line`) when scanning JSONL transcripts and redact secrets before storing or sending to LLMs.
</important>

<important if="you are orchestrating workers or committing on an Orca worktree">

Read `.claude/skills/orca-model-matrix/SKILL.md` and `brain/tasks/index.md`:
- Branch flow is `prod <- staging <- pratham404/<task-id>`. Keep `legacy/tui-cli` untouched as the isolated CLI/TUI branch.
- Run `make gate` before every commit and merge into `staging` with `git merge --no-ff`.
- Keep commit messages short and lowercase: `feat: <what you did>`, `fix: <what you did>`, `test: <what you did>`, `docs: <what you did>`.
</important>
