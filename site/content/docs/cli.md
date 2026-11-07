---
title: CLI
group: Reference
order: 10
description: learnpuppy doctor, serve and migrate.
---

# CLI

```text
learnpuppy <command>
```

## doctor

```bash
learnpuppy doctor
```

Checks the data folder is writable, whether a server is already running, whether the port is free, free disk space, which of `claude`, `codex` and `agy` are installed and logged in (kiro-cli and opencode too), and whether you are online. Each line starts with `ok`, `warn` or `FAIL`. It exits non-zero on a failure. Not finding any CLI login is a failure.

## serve

```bash
learnpuppy serve [--port 7878] [--no-open]
```

Starts the server and prints the launch URL.

| Flag | Meaning |
| --- | --- |
| `--port N` | First port to try. Busy ports count up. `0` picks a free one. |
| `--no-open` | Do not open the browser. |
| `--dev` | No launch token, scripted engine unless `--real`. For development. |
| `--real` | With `--dev`, use the real CLI pool. |
| `--engine real\|faux` | Choose the engine. Also `LP_ENGINE`. |
| `--fixture DIR` | Serve a fixture data folder instead of `~/.learnpuppy`. |

A second `serve` finds the running one and reuses it.

## migrate

```bash
learnpuppy migrate
learnpuppy migrate --revert
```

One time: moves an old single-store data folder into a workspace named "Default". It is idempotent, and `--revert` undoes it.

## Other

| Command | Use |
| --- | --- |
| `seed --fixture small\|medium\|large\|huge --out DIR` | Writes a deterministic fixture data folder (40, 800, 5k or 10k nodes) for tests and demos. |
| `openapi [--out FILE]` | Prints the OpenAPI document of the local API. |

## Environment

| Variable | Meaning |
| --- | --- |
| `LEARNPUPPY_HOME` | Data folder. Default `~/.learnpuppy`. |
| `LP_ENGINE` | Default engine for `serve`. |
| `CLAUDE_CONFIG_DIR` | An extra Claude profile to discover. |
