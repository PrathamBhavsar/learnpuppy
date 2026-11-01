# 12. CLI isolation and the failure taxonomy

Owner: `crates/lp-cli-driver`. Verified on this machine, 2026-11-01 (claude 2.x, codex, agy 1.4.8, kiro-cli 2.37, opencode 2.2.3).

## What each tutor CLI can reach

The tutor must have no tools and none of the user's own context. Every CLI runs with `env_clear`, an empty private cwd
(`lp-sandbox-*`), `kill_on_drop`, a hard timeout, and the prompt on stdin.

| CLI | Tools | User context | What remains reachable |
|---|---|---|---|
| claude | `--tools ""`, `--strict-mcp-config` | `--setting-sources project,local` drops `~/.claude/CLAUDE.md`, output style and user settings (**#413**; proven: a haiku call quoted the user's global `graphify` section before the flag, answered NO after) | Nothing beyond the model; auth is unaffected (multi-account `CLAUDE_CONFIG_DIR` still works) |
| codex | `-s read-only` | `--ignore-user-config` (no `config.toml` MCP servers / notify hooks), `--ephemeral` | **#414 residual:** read-only still means *read everything*. A codex turn ran `ls ~/.ssh` and returned the listing. The CLI has no flag to narrow reads, and a private `HOME` hung codex (auth), so it is not used. Treat codex turns as able to read any file the user can. |
| agy | `--sandbox` | none configurable | Same class of residual as codex (terminal sandbox, not a read jail). |
| kiro | private `HOME` containing only a tool-less agent (`.kiro/agents/lp-tutor.json`: `tools: []`, no MCP, no resources) selected with `--agent lp-tutor`; `--trust-tools=` | the private `HOME` hides `~/.kiro` skills, steering, MCP config, session history (28 user skills and an MCP server were loaded before) | Symlinks to kiro's own binaries and login store only. `--trust-tools=` alone is **not** enough: kiro still read a file in the cwd; the empty agent is what removes the tools (proven: no `tool_call` events, the secret was not returned). |
| opencode | `OPENCODE_CONFIG_CONTENT` = `permission: {"*": "deny"}` (no `--auto`), `--standalone` | private `HOME`/`XDG_*` (hides `~/.config/opencode` MCP servers, plugins, skills), `OPENCODE_DISABLE_CLAUDE_CODE=1` | `--standalone` is required: without it the already-running background service answers with the **user's** config and ignores ours (proven: a file read succeeded). Effective permissions checked with `opencode debug agents --standalone` (`* deny` is the last rule). The live "read a file" proof could not be repeated after the free tier's 429 (see below). |

Kiro refuses to run logged out by opening a browser login. The pool must therefore only spawn kiro for accounts that
discovery found logged in (`kiro-cli whoami -f json` has an `email`).

## Failure taxonomy

`DriverError::failure(now_ms) -> FailureKind` for all five CLIs; fixtures are real captures in
`crates/lp-cli-driver/src/engine/fixtures/`.

| Kind | Examples | Pool reaction |
|---|---|---|
| `NotInstalled` | spawn `NotFound` | status `Missing` |
| `NotLoggedIn` | claude "Not logged in · Please run /login", 401 | `NeedsLogin` |
| `Offline` | `ECONNREFUSED`, "waiting for network", DNS, proxy | **global** backoff 5s..120s, no account cooled, every row reads `Offline` |
| `NoModel` | opencode `provider.no-route`, kiro "model … is not available", codex `model_not_found`, claude "issue with the selected model" | that model is benched, siblings keep serving |
| `RateLimited{resets_at}` | agy "Resets in 44h14m34s", opencode `retryAfterMs`, "try again in 20 minutes", "retry after N seconds", monthly credits "Resets on 2027-02-01", HTTP 429/402 | cool down until `resets_at` (backoff 60s..1h if unknown) |
| `TimedOut` | hard timeout | no state change |
| `Other(text)` | everything else | no state change |

Real-world notes: opencode's free tier shares one quota (`provider.free-limit`, ~5.5h reset) across all `opencode/*-free`
models, so a quota message cools the whole account; a plain 429 on one model cools only that model.
Claude reports errors as an `is_error` result line (exit 0 or 1), not on stderr.
