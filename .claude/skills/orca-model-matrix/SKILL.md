---
name: orca-model-matrix
description: Exact model names, effort levels, CLI commands, and task routing matrix across Claude, Codex, and Antigravity (agy) for LearnPuppy Orca multi-worktree orchestration.
---

# Orca Multi-Agent Model & Effort Matrix

Use this skill when dispatching or auditing parallel workers across git worktrees in `/Users/pratham/code/learnpuppy`.

## 1. Exact Model Roster & Effort Levels

| Tier & Role | Provider / Binary | Exact Model ID | Effort Level | Best Used For |
|---|---|---|---|---|
| **Chief Planner & Final Sign-Off Auditor** | `agy` (or `claude`) | `claude-opus-4-6-thinking` (claude alias: `opus`) | `high` (`max` on final gate) | Cross-crate architectural review, race/deadlock detection, OKF strict audit (`T00-audit`), and Phase 6 final sign-off (`T60`). |
| **Rust Systems, Math & Storage Specialist** | `codex` (`/opt/homebrew/bin/codex`) | `gpt-5.4` (or `gpt-6-luna` via `-c model_reasoning_effort="high"`) | `high` | Pure algorithmic code (`lp-core` graph merger, `simplify_chains`, ForceAtlas2 layout, FSRS v5, Mermaid DAG parser), SQLite WAL/FTS5 (`lp-store`), and workspace Cargo plumbing (`D00`). |
| **UI Component & Stateful Feature Engineer** | `agy` (or `claude`) | `claude-sonnet-4-6` (claude alias: `sonnet`) | `high` | Building `lp-ui` shadcn-style Dioxus 0.7 components (`D10`, `D11`), `lp-app` desktop screens (Classroom `D21`, Interactive SVG Knowledge Graph `D22`, FSRS Digest `D23`), and agent prompt loops (`lp-agent`). |
| **Fast E2E, Subprocess & Harness Specialist** | `agy` (`/Users/pratham/.local/bin/agy`) | `gemini-3.1-pro-high` (or `gemini-3-flash` for quick checks) | `high` / `medium` | Multi-account CLI auth discovery (`lp-cli-driver`), `tester-army/e2e` browser/agentic test suites, `alibaba/open-code-review` (`ocr`) runs, and OKF validation (`D30`, `D60`). |

## 2. Worktree Isolation & Merge Protocol

1. **Create Isolated Worktree off `staging`**:
   ```sh
   orca worktree create --repo path:/Users/pratham/code/learnpuppy --name <TASK_ID> --base-branch staging --no-parent --json
   ```
2. **Disjoint File Ownership**:
   - Parallel workers in the same wave MUST own disjoint crates or files so `--no-ff` merges into `staging` have zero conflicts.
3. **Mandatory Worker Gate Before Commit**:
   ```sh
   export PATH="/opt/homebrew/bin:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
   make gate
   ```
4. **Short Commit Convention**:
   - `feat: <what you did>`, `fix: <what you did>`, `test: <what you did>`, `docs: <what you did>`.
