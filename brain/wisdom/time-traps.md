---
type: guide
title: Known Time-Traps & Workarounds
description: Hard-won operational lessons on macOS Keychain prompts, Homebrew rustup PATH, CLI subprocess sandboxing, and Orca worktree management.
tags: [wisdom, time-traps, macos, rustup, claude, codex, agy, orca]
timestamp: 2026-10-27
---

# Known Time-Traps & Workarounds

Back to [Wisdom Index](/wisdom/index.md). Related: [Environment](/environment.md), [Multi-Account CLI Pool](/architecture/cli-auth-pool.md).

## 1. `security find-generic-password -w` Hangs Forever in Headless Shells
- **Trap**: Calling `security find-generic-password -s "Claude Code-credentials" -w` pops a native macOS GUI password dialog that blocks background shells indefinitely.
- **Fix**: Never read macOS Keychain directly. Read account metadata (`accountUuid`, `emailAddress`, `seatTier`) directly from `<config_dir>/.claude.json` and let the `claude` CLI binary read its own keychain entry when spawned with `CLAUDE_CONFIG_DIR=<config_dir>`.

## 2. Homebrew `rustup` Missing from Default Non-Interactive `PATH`
- **Trap**: Fresh non-login zsh/sh subprocesses fail with `cargo: command not found` because `rustup` lives in `/opt/homebrew/opt/rustup/bin`.
- **Fix**: Every `Makefile`, test script, and Orca worker command must export:
  ```sh
  export PATH="/opt/homebrew/opt/rustup/bin:$HOME/.cargo/bin:$PATH"
  ```

## 3. Nested `claude -p` or `codex exec` Inheriting Parent Agent Env Vars or Hooks
- **Trap**: When `lp-cli-driver` spawns `claude -p` or `codex exec` from inside a directory with `.claude/settings.json` hooks or an active `CLAUDECODE=1` environment variable, the child CLI either refuses to start or blocks waiting for interactive hook approval.
- **Fix**:
  1. Always set `.current_dir("/tmp/learnpuppy-sandbox")` (an empty directory created on startup).
  2. Strip `CLAUDECODE`, `CLAUDE_CODE_ENTRYPOINT`, and `ORCA_*` from the child process environment.
  3. Pass `--strict-mcp-config` and `--tools ""` on structured schema calls.

## 4. Single-Line Multi-Megabyte JSON Outputs
- **Trap**: Running `head` or `cat` on minified JSON files (like REA outputs or large `chats/<id>.json`) floods the terminal buffer.
- **Fix**: Always inspect JSON files with `jq` or `python3 -m json.tool | head -n 50`.
