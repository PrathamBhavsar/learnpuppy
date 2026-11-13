---
title: Security
group: Reference
order: 9
description: The local-only model, and what it does not protect against.
---

# Security

LearnPuppy is a single-user, local tool. The security model is to not be reachable from anywhere but your own browser.

## Network

- The server binds 127.0.0.1 (and `[::1]` when available). It tries the port you ask for, then the next ones up. The default start is 7878.
- `serve` prints a launch URL with a one-time token. The first request trades the token for an `HttpOnly`, `SameSite=Strict` cookie and removes it from the URL. The server stores only a hash of the token in its lock file.
- A request with a foreign `Host` is refused with 421. Writes must carry your own `Origin` and an `X-LP-CSRF` header.
- Without the cookie, API routes return 401 and the browser shows an "open from terminal" screen. Only the health check is public.

`serve --dev` skips the token. Use it only for development.

## Data

Everything is files under `~/.learnpuppy` (or `LEARNPUPPY_HOME`): append-only JSONL ledgers, plus SQLite for search. Nothing is sent to a LearnPuppy server, because there is none. Ingested transcripts, agy artifacts and git diffs pass through secret redaction first. The diagnostics you can copy from Settings are tested not to include tokens.

## What the tutor CLIs can reach

Each CLI runs with a cleared environment and an empty private working directory, with a hard timeout and the prompt on stdin.

| CLI | Isolation | What remains |
| --- | --- | --- |
| claude | Tools off, user settings and `CLAUDE.md` not loaded. | Nothing beyond the model. |
| codex | Read-only sandbox, user config ignored, ephemeral. | Read-only still means it can read any file you can. |
| agy | Sandbox flag. | The same class of residual as codex. |

Treat codex and agy turns as able to read your files. The details and the evidence are in `docs/web/12-cli-isolation-and-failures.md` in the repo.

## Artifacts

HTML produced by workbench jobs renders in a sandboxed iframe with no scripts.

## Reporting

The repository has no security policy file yet. Until it does, open an issue without exploit details and ask for a private channel.
