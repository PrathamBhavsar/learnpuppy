---
type: doc
title: Environment, Tooling & CLI Auth Facts
description: Verified machine environment, Rust toolchain paths, discovered CLI accounts (Claude multi-account, Codex, Antigravity), and Orca orchestration commands.
tags: [environment, toolchain, rust, claude, codex, agy, orca]
timestamp: 2026-10-27
---

# Environment, Tooling & CLI Auth Facts

Back to [Brain Index](/index.md). Related: [Time Traps](/wisdom/time-traps.md), [Pre-Merge Gate](/wisdom/pre-merge-gate.md), [Multi-Account CLI Pool](/architecture/cli-auth-pool.md).

## 1. Host & Toolchain Paths

- **OS / Arch**: macOS (`arm64` / Apple Silicon)
- **Repository Root**: `/Users/pratham/code/learnpuppy`
- **Reference Repositories on Disk**:
  - Upstream `learnharness` clone: `/tmp/learnharness-repo`
  - REA static analysis output: `/tmp/rea-learnharness.json`
  - Tracker codebase: `/Users/pratham/code/tracker`
  - Bunfiiliate reference OKF codebase: `/Users/pratham/code/bunfiiliate`
- **Rust Toolchain**:
  - Installed via Homebrew `rustup` at `/opt/homebrew/opt/rustup/bin`.
  - **CRITICAL**: Always prepend `/opt/homebrew/opt/rustup/bin` to `PATH` in Makefiles, scripts, and worker shells:
    ```sh
    export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
    ```
  - Verified compiler: `rustc 1.95+` / `cargo` with `clippy` and `rustfmt`.

## 2. Verified Local CLI Logins on This Machine

LearnPuppy's `lp-cli-driver` discovers and pools these exact local CLI installations without any browser OAuth prompt:

| CLI Binary | Version | Discovered Account Profiles | Config / Auth Location |
|---|---|---|---|
| `claude` (`/Users/pratham/.local/bin/claude`) | `2.1.114` | **Account 1**: `pratham.bhavsar501@gmail.com`<br>**Account 2**: `q97159745@gmail.com`<br>**Work Profile**: `~/.claude-work` | Account 1: `CLAUDE_CONFIG_DIR="$HOME/.claude-accounts/account2"` (`.claude.json`)<br>Account 2: `~/.claude` + `~/.claude.json` (`CLAUDE_CONFIG_DIR` unset)<br>Shell helper in `~/.zshrc`: `claude-use 1\|2` |
| `codex` (`/opt/homebrew/bin/codex`) | `0.121.0` | `yadavkumar49900@gmail.com` (`chatgpt` mode, default model `gpt-5.4`) | `~/.codex/auth.json` + `~/.codex/config.toml` |
| `agy` (`/Users/pratham/.local/bin/agy`) | `1.2.3` | Active Google OAuth (`claude-opus-4-6-thinking`, `claude-sonnet-4-6`, `gemini-3.1-pro-high`, `gemini-3-flash`) | `~/.gemini/oauth_creds.json` + `~/.gemini/google_accounts.json` |

> **NEVER run `security find-generic-password -w`** to read macOS Keychain tokens directly — it triggers a blocking macOS GUI password dialog that hangs headless workers. Instead, read account metadata from `.claude.json` / `auth.json` and invoke the CLI binaries (`claude`, `codex`, `agy`) with the appropriate `CLAUDE_CONFIG_DIR` environment variable.

## 3. Validation, E2E & Code Review Tools

- **OKF Bundle Validator**:
  ```sh
  python3 /Users/pratham/.claude/skills/okf/scripts/validate.py /Users/pratham/code/learnpuppy/brain --strict
  python3 /Users/pratham/.claude/skills/okf/scripts/visualize.py /Users/pratham/code/learnpuppy/brain
  ```
- **Orca Multi-Agent Supervisor**: `/opt/homebrew/bin/orca` (`orca status`, `orca spawn`, `orca logs`, `orca wait`).
- **Open Code Review (`alibaba/open-code-review`)**: `ocr` (`@alibaba-group/open-code-review`).
- **E2E Harness (`tester-army/e2e`)**: Driven via `Makefile` (`make e2e`) + `expectrl` PTY tests.
