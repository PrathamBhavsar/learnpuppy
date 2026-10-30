---
type: doc
title: Environment, Tooling & CLI Auth Facts
description: Verified machine environment, Rust toolchain paths, discovered CLI accounts (Claude multi-account, Codex, Antigravity), installed project skills, and Orca orchestration commands.
tags: [environment, toolchain, rust, claude, codex, agy, orca, skills]
timestamp: 2026-10-28
---

# Environment, Tooling & CLI Auth Facts

Back to [Brain Index](/index.md). Related: [Time Traps](/wisdom/time-traps.md), [Pre-Merge Gate](/wisdom/pre-merge-gate.md), [Multi-Account CLI Pool](/architecture/cli-auth-pool.md).

## 1. Host & Toolchain Paths

- **OS / Arch**: macOS (`arm64` / Apple Silicon — primary build target `aarch64-apple-darwin`)
- **Repository Root**: `/Users/pratham/code/learnpuppy`
- **Isolated Legacy TUI/CLI Branch**: `legacy/tui-cli`
- **Reference Repositories on Disk**:
  - Upstream `learnharness` clone: `/tmp/learnharness-repo`
  - REA static analysis output: `/tmp/rea-learnharness.json`
  - Dioxus official components reference: `/tmp/dioxus-components` (`DioxusLabs/components`)
  - Community Dioxus UI reference: `/tmp/rust-ui-dioxus` (`rust-ui/dioxus-ui`)
  - TesterArmy E2E framework clone: `/tmp/tester-army-e2e` (`tester-army/e2e`)
  - Tracker codebase: `/Users/pratham/code/tracker`
  - Bunfiiliate reference OKF codebase: `/Users/pratham/code/bunfiiliate`
- **Rust Toolchain**:
  - Installed via Homebrew `rustup` at `/opt/homebrew/opt/rustup/bin`.
  - **CRITICAL**: Always prepend `/opt/homebrew/opt/rustup/bin` to `PATH` in Makefiles, scripts, and worker shells:
    ```sh
    export PATH="/opt/homebrew/bin:/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
    ```

## 2. Installed Per-Project Skills (`.claude/skills/` & `.agents/skills/`)

Installed from [`humanlayer/skills`](https://github.com/humanlayer/skills) and [`tester-army/e2e`](https://github.com/tester-army/e2e) plus LearnPuppy-specific project skills:
- `rpi`: Research -> PRD/TDD -> Outline -> Implement workflow across fresh contexts.
- `show-me`: Visual explanations using diagrams, code-shape sketches, and focused HTML artifacts.
- `visual-pr`: Visual PR summaries for human and agent review.
- `improve-claude-md`: `<important if="...">` conditional context structuring (applied to `CLAUDE.md`).
- `build-iterated-agentic-loop` & `design-control-loop`: Iterated agent control loops.
- `e2e` & `create-verification-skill`: Official `tester-army/e2e` browser/mobile agentic E2E test authoring and verification skills.
- `dioxus-desktop-ui`: Archived Dioxus 0.7 + `lp-ui` rules (the crates were removed by [ADR-0006](/decisions/0006-web-ui-over-dioxus.md)).
- `orca-model-matrix`: Project-specific model selection, effort levels, and Orca worktree dispatch rules.

## 3. Verified Local CLI Logins & Exact Model Capabilities

| CLI Binary | Version | Discovered Account Profiles | Exact Models & Best-Fit Responsibilities |
|---|---|---|---|
| `claude` (`/Users/pratham/.local/bin/claude`) | `2.1.114` | **Account 1**: `pratham.bhavsar501@gmail.com` (`~/.claude-accounts/account2`)<br>**Account 2**: `q97159745@gmail.com` (`~/.claude`)<br>**Work Profile**: `~/.claude-work` | - `opus` (`claude-opus-4-6-thinking`, `--effort high`/`max`): Master architectural planning, cross-crate deadlock/invariant audits, and final sign-off (`D60`).<br>- `sonnet` (`claude-sonnet-4-6`, `--effort high`): Dioxus 0.7 UI components (`lp-ui`), visual screens (`lp-app`), and stateful agent loops. |
| `codex` (`/opt/homebrew/bin/codex`) | `0.121.0` | `yadavkumar49900@gmail.com` (`chatgpt` mode, `~/.codex/auth.json`) | - `gpt-5.4` / `gpt-6-luna` (`-c model_reasoning_effort="high"`): Pure algorithmic Rust (`lp-core`), SQLite WAL/FTS5 (`lp-store`), Cargo workspace scaffolding (`D00`), and deterministic math/layout tests. |
| `agy` (`/Users/pratham/.local/bin/agy`) | `1.2.3` | Active Google OAuth (`~/.gemini/oauth_creds.json`) | - `claude-opus-4-6-thinking` (`--effort high`): Chief Planner & Auditor via AGY.<br>- `claude-sonnet-4-6` (`--effort high`): UI & feature worker via AGY.<br>- `gemini-3.1-pro-high` (`--effort high`): Fast E2E test harness (`tester-army/e2e`), `ocr` review runner, and OKF verification. |

> **NEVER run `security find-generic-password -w`** to read macOS Keychain tokens directly — it triggers a blocking macOS GUI password dialog that hangs headless workers.

## 4. Validation, E2E & Code Review Tools

- **OKF Bundle Validator**:
  ```sh
  python3 /Users/pratham/.claude/skills/okf/scripts/validate.py /Users/pratham/code/learnpuppy/brain --strict
  ```
- **Orca Multi-Agent Supervisor**: `/opt/homebrew/bin/orca` (`orca worktree create`, `orca terminal create`, `orca terminal wait`, `orca worktree rm`).
- **Open Code Review (`alibaba/open-code-review`)**: `/opt/homebrew/bin/ocr` (`@alibaba-group/open-code-review@1.13.3` — `ocr review`, `ocr scan`, `ocr delegate`).
- **E2E Harness**: the root `e2e.config.ts` + `tests/*.e2e.ts` Dioxus harness was removed with `lp-app` ([ADR-0006](/decisions/0006-web-ui-over-dioxus.md)); browser E2E moves to Playwright under `web/e2e/` (`docs/web/07-testing-and-quality.md`).
