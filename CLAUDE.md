# CLAUDE.md

LearnPuppy (`learnpuppy`) — pure-Rust visual learning harness, cross-session Knowledge Graph, and FSRS spaced-repetition desktop app built with **Dioxus 0.7** + custom shadcn-style **`lp-ui`** (macOS first; Windows next; iOS/Android last). Legacy Ratatui CLI/TUI is preserved in isolation on branch `legacy/tui-cli`.

## Project map

- `brain/` — OKF v0.1 knowledge bundle (cold-start orientation, architecture specs, ADRs, guardrails, and task DAG). Read `brain/index.md` first.
- `docs/` — Master execution plan (`docs/PLAN.md`) and clean-room reverse-engineering spec (`docs/REVERSE_ENGINEERING.md`).
- `crates/lp-core/` — Pure synchronous domain core (Graph merger, `simplify_chains`, ForceAtlas2 layout, Mermaid `plan_init` DAG parser, 3-phase state machine, FSRS v5 scheduler, port traits). Zero IO, zero async.
- `crates/lp-store/` — Hybrid append-only JSONL ledgers (`nodes.jsonl`, `chains.jsonl`, `coverage.jsonl`, `sync-<device>.jsonl`) + SQLite WAL (`index.db`) with FTS5 full-text search.
- `crates/lp-cli-driver/` — Zero-config local CLI auth auto-discovery (`claude` multi-account pool via `CLAUDE_CONFIG_DIR`, `codex`, `agy`), rate-limit failover scheduler, sandboxed subprocess runner, and `FauxEngine`.
- `crates/lp-agent/` — Stateless per-turn `ProberAgent`, `TeacherAgent`, `PracticeAgent`, async workbench sub-agents (`research`, `pdf`, `visual`, `demo`), Wake pump, and 3-stage `Extractor -> Auditor -> Merger`.
- `crates/lp-ingest/` — Incremental `mtime+size` ingestor for local Claude (all accounts), Codex, and Antigravity (`with_snapshot` SQLite copy) transcripts + Git commits/diffs, decision distiller, and 4-choice FSRS card generator.
- `crates/lp-ui/` — Pure-Rust shadcn/Radix-inspired Dioxus 0.7 component library (`Button`, `Badge`, `Card`, `Dialog`, `Tabs`, `Tooltip`, `Progress`, `Slider`, `Switch`, `ScrollArea`, `CommandPalette`, Lucide SVG icons, OKLCH dark theme tokens).
- `crates/lp-app/` — Native macOS Dioxus 0.7 visual desktop app (`learnpuppy-app`) with interactive SVG Knowledge Graph Canvas, Classroom split-view, Plan DAG viewport, FSRS Digest, and Accounts/Slots modal.
- `crates/lp-tui/` — Headless CLI subcommands + legacy terminal UI (isolated on branch `legacy/tui-cli`).
- `.claude/skills/` & `.agents/skills/` — Installed project skills (`rpi`, `show-me`, `visual-pr`, `improve-claude-md`, `build-iterated-agentic-loop`, `design-control-loop`, `e2e`, `create-verification-skill`, `dioxus-desktop-ui`, `orca-model-matrix`).

<important if="you need to run commands to build, test, lint, validate OKF, or run E2E and code review">

Always prepend `/opt/homebrew/opt/rustup/bin` to `PATH` or run via `make`:

| Command | What it does |
|---|---|
| `make gate` | Run `fmt`, `clippy -D warnings`, `test --workspace`, and strict OKF validation (`brain/`) |
| `make ci` | Run `make gate` + `cargo deny check` |
| `make okf` | Validate `brain/` OKF knowledge bundle with `--strict` (0 errors, 0 warnings) |
| `cargo check -p lp-ui -p lp-app` | Fast typecheck of the Dioxus 0.7 component library and macOS desktop app |
| `cargo test --workspace --all-targets` | Run all unit, `proptest`, and SSR/component tests across the workspace |
| `npx e2e run` | Execute `tester-army/e2e` browser/agentic E2E test suite against `lp-app` |
| `ocr review` / `ocr scan` | Run Alibaba `open-code-review` (`ocr`) static + LLM diff audit |
</important>

<important if="you are creating or modifying Dioxus 0.7 UI components in crates/lp-ui or screens in crates/lp-app">

Read `.claude/skills/dioxus-desktop-ui/SKILL.md` and `brain/architecture/dioxus-ui.md` first:
- Use pure Rust `Signal<T>` and `Memo<T>` (`use_signal`, `use_memo`, `use_context`) for all interactive state (dialogs, tabs, command palette, graph pan/zoom camera, quiz selection).
- Never use `script { dangerous_inner_html: ... }` or `setTimeout` DOM polling to attach event listeners (the `rust-ui/dioxus-ui` anti-pattern).
- Never import `wasm_bindgen`, `web_sys`, `js_sys`, or `gloo_timers` in `lp-ui` or `lp-app` — they break native macOS desktop (`aarch64-apple-darwin`) compilation.
- Always call `event.prevent_default()` on `<form onsubmit>` handlers so the macOS WKWebView never reloads the page.
- Keep the root window container `h-screen w-screen overflow-hidden select-none flex flex-col bg-background text-foreground` to prevent double scrollbars; scroll only inside internal panes.
- Run blocking SQLite/filesystem reads inside `tokio::task::spawn_blocking` and async LLM turns inside `spawn(async move { ... })`.
- Add `data-testid` and semantic ARIA attributes (`role="dialog"`, `role="tab"`, `role="status"`, `aria-label`) on every interactive control for `tester-army/e2e`.
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
