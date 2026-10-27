---
type: adr
title: "ADR-0002: Zero-Config Multi-Account CLI Auth & Failover Pool"
description: Auto-discover local claude (multiple subscription accounts via CLAUDE_CONFIG_DIR), codex, and agy CLIs with per-account quota tracking and failover.
tags: [adr, auth, claude, codex, agy, multi-account, rate-limits]
timestamp: 2026-10-27
status: accepted
---

# ADR-0002: Zero-Config Multi-Account CLI Auth & Failover Pool

Back to [Decisions Index](/decisions/index.md). Related: [Multi-Account CLI Pool](/architecture/cli-auth-pool.md), [Environment](/environment.md).

## Context
Upstream `learnharness` failed to detect existing local CLI logins because it only implemented custom PKCE OAuth (`chatgpt` on port 1455), GitHub Copilot device flow, and Electron `BrowserWindow` cookie scraping (`grok`). Meanwhile, the user's machine already has authenticated `claude`, `codex`, and `agy` CLIs — including **multiple distinct Claude subscriptions** stored in separate config directories (`~/.claude` for `q97159745@gmail.com` and `~/.claude-accounts/account2` for `pratham.bhavsar501@gmail.com`).

## Decision
1. Implement zero-config CLI credential discovery in `lp-cli-driver` that inspects `~/.claude`, `~/.claude-accounts/*`, `~/.claude-work`, `$CLAUDE_CONFIG_DIR`, `~/.codex/auth.json`, and `~/.gemini/oauth_creds.json`.
2. Deduplicate Claude accounts by `oauthAccount.accountUuid` in `.claude.json` and treat **each distinct UUID as an independent provider account** (`claude:<email>`) with its own 5-hour/24-hour rate-limit counters and cooldown state.
3. Execute each Claude account by passing its specific `CLAUDE_CONFIG_DIR` to the sandboxed `claude -p` child process, automatically failing over to the next healthy Claude account (or `codex`/`agy`) when a rate limit is hit.

## Consequences
- **Pros**: Zero login friction on any developer machine; doubles/triples effective Claude subscription throughput by pooling separate accounts; never triggers macOS Keychain GUI popups.
- **Cons**: Subprocess execution must carefully strip nested session variables (`CLAUDECODE`) and run in an empty sandbox directory (`/tmp/learnpuppy-sandbox`) so project hooks do not interfere.
