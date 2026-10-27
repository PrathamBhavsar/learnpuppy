---
type: spec
title: Multi-Account CLI Auth Discovery & Quota Pool (lp-cli-driver)
description: Zero-config discovery of local Claude (multiple subscription accounts via CLAUDE_CONFIG_DIR), Codex, and Antigravity CLIs, rate-limit failover pool, and FauxEngine.
tags: [auth, cli-driver, claude, codex, agy, multi-account, rate-limit, failover]
timestamp: 2026-10-27
---

# Multi-Account CLI Auth Discovery & Quota Pool (`lp-cli-driver`)

Back to [Architecture Index](/architecture/index.md). Related: [ADR-0002 Multi-Account CLI Pool](/decisions/0002-cli-multi-account-pool.md), [Environment](/environment.md).

## 1. Zero-Config Multi-Account Discovery (`discover_accounts`)

On startup, `lp-cli-driver` scans local CLI configuration directories without triggering any macOS Keychain password dialog:

### 1.1 Claude Code Multi-Account Enumeration
Scans candidate config directories in order:
1. Default `~/.claude` (paired with `~/.claude.json` or `~/.claude/.claude.json`).
2. Every subdirectory of `~/.claude-accounts/*` (e.g., `~/.claude-accounts/account2`).
3. `~/.claude-work` (if present).
4. Current `$CLAUDE_CONFIG_DIR` environment variable (if set).
5. Any additional profile paths registered via `learnpuppy auth add-claude <path>`.

For each directory containing `.claude.json` with an `oauthAccount` object:
- Extracts `accountUuid`, `emailAddress`, `displayName`, `seatTier`, `userRateLimitTier`, and `organizationUuid`.
- Deduplicates by `accountUuid` so symlinked or aliased directories never double-count the same subscription, while **distinct accounts (e.g., `pratham.bhavsar501@gmail.com` and `q97159745@gmail.com`) are registered as separate `ClaudeAccount` instances with independent quota state**.

### 1.2 Codex CLI Discovery
- Reads `~/.codex/auth.json` (`auth_mode: "chatgpt"` or API key) and `~/.codex/config.toml` (`model = "gpt-5.4"`).
- Decodes the unverified JWT payload of `tokens.id_token` to read `email`, `chatgpt_plan_type`, and token expiration.

### 1.3 Antigravity (`agy`) Discovery
- Reads `~/.gemini/oauth_creds.json` and `~/.gemini/google_accounts.json`.
- Registers supported models: `claude-opus-4-6-thinking`, `claude-sonnet-4-6`, `gemini-3.1-pro-high`, `gemini-3-flash`.

## 2. Per-Account Quota Ledger & Automatic Failover (`AccountPool`)

- **Independent Rate-Limit Buckets**:
  - Each `AccountId` tracks `health: Healthy | Cooldown { until: DateTime<Utc>, reason: String } | AuthExpired`, plus rolling 5-hour and 24-hour counters (`requests`, `input_tokens`, `output_tokens`).
- **Failover Routing**:
  - When invoking a `claude` turn, `AccountPool::acquire(RoleSlot)` picks the highest-priority `Healthy` Claude account (or round-robins across healthy accounts with the lowest 5-hour token utilization).
  - If the subprocess exits with a rate-limit / quota error (`"rate_limit_error"`, `"5-hour limit"`, `"429"`, or `"hit your limit"`), `lp-cli-driver` marks that specific Claude account as `Cooldown`, logs the switch, and **immediately retries the request on the next healthy Claude account** (or falls back to `codex` / `agy` per slot policy).

## 3. Sandboxed Subprocess Invocation Rules

Every CLI subprocess call (`claude`, `codex`, `agy`) enforces:
1. **Isolated Working Directory**: Runs in `/tmp/learnpuppy-sandbox` (an empty directory with no `.git` or `.claude/settings.json` hooks) so local project hooks or `CLAUDE.md` files never pollute or block the prompt.
2. **Environment Isolation**: Strips `CLAUDECODE`, `ORCA_*`, and nested session variables; sets `CLAUDE_CONFIG_DIR` explicitly for the chosen Claude account; omits `CLAUDE_CONFIG_DIR` when targeting the default `~/.claude` account.
3. **No Tool Use Inside Structured Calls**: Passes `--tools ""` and `--strict-mcp-config` to `claude -p` and `--sandbox` to `agy -p` when running `complete_structured<T>`, ensuring pure JSON schema output.
4. **Zombie Prevention**: Sets `Command::kill_on_drop(true)` and selects against a `CancellationToken` + per-call timeout.
