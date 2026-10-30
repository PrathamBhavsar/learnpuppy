# 01 — Architecture

## Process model
```
Browser tab(s)  ──HTTP /api/v1 + SSE──▶  learnpuppy serve  (one process, tokio)
 Vite SPA (dev: :5173 proxy)               ├─ lp-server   axum, auth, SSE hub, OpenAPI, embedded SPA
 prod: served by lp-server                 ├─ lp-service  WorkspaceRegistry, services (chat/graph/review/digest/ingest/accounts/settings)
                                           ├─ lp-agent / lp-ingest / lp-cli-driver / lp-store / lp-core   (existing, reused)
                                           └─ subprocesses: claude / codex / agy  (sandboxed, kill_on_drop, existing runner)
```
- One server process per user. `~/.learnpuppy/server.lock` (pid, port, token hash). A second `serve` finds a live lock and just opens the browser at the running instance.
- Closing the browser does **not** stop turns/jobs/ingest; the server owns them. Reopening resumes via snapshot + SSE replay.

## Crates after surgery
```
crates/
  lp-core        (unchanged, pure sync)
  lp-store       (unchanged API; gains `Workspace::open(root)` helper only if needed)
  lp-cli-driver  (unchanged)
  lp-agent       (unchanged)
  lp-ingest      (unchanged)
  lp-service     NEW  logic, no HTTP
  lp-server      NEW  axum binary `learnpuppy` (subcommands: serve, migrate, doctor, seed, openapi)
```
`lp-service` modules:
| Module | Origin | Notes |
|---|---|---|
| `workspace/{registry,meta,migrate,trash,export}` | new | see 02 |
| `engine.rs` | `lp-tui/src/engine.rs` (`CliPoolEngine`, role routing) | `FauxEngine` selectable via `LP_ENGINE=faux` |
| `settings.rs` | `lp-tui` config module | profiles, pins; global (not per workspace) with optional per-workspace override file |
| `chat/{turn,view,answer}.rs` | `lp-tui/src/session.rs` (`run_to_rest`, `render_answer`, `resolve_option`, `verdict`, `classroom_view`) | produces `ClassroomView` DTO source |
| `review.rs` | `lp-tui/src/review.rs` | `due_cards`, `grade`, `streak`, `sync`, `export_cards` |
| `graph.rs` | `lp-app` backend + `lp-core` | snapshot, positions cache, delete preview, search |
| `digest.rs` | `lp-app/views/digest_fsrs.rs` data prep | decisions grouped by session/day |
| `ingest.rs` | `lp-ingest` wrapper | progress events, source resolution, scoping |
| `accounts.rs` | `lp-cli-driver::accounts` wrapper | process-wide `Arc<AccountPool>` |
| `events.rs` | new | typed event enum + per-stream ring buffer |

`lp-server` modules: `main.rs`, `cli.rs`, `app.rs` (router), `auth.rs`, `error.rs`, `sse.rs`, `openapi.rs`, `assets.rs` (rust-embed + SPA fallback), `routes/{workspaces,graph,chats,review,digest,ingest,accounts,settings,search,artifacts}.rs`, `dto/*.rs`.

## Repo layout (end state)
```
learnpuppy/
  Cargo.toml  Makefile  CLAUDE.md  AGENTS.md  deny.toml
  crates/…  (7 crates above)
  web/
    package.json bun.lock vite.config.ts tsconfig*.json components.json eslint.config.js index.html
    openapi/openapi.json            (generated, committed, drift-checked)
    src/ …                          (see 04)
    e2e/                            (Playwright)
    scripts/{hex2oklch.ts,gen-api.ts,seed-fixture.ts}
  docs/web/…  brain/…  (OKF updated: ADR 0006, architecture/web-ui.md, supersede 0005)
```

## Data flow (3 canonical paths)
1. **Read**: component → `useQuery(['ws',ws,'graph'])` → `openapi-fetch` → axum → `lp-service::graph` → `lp-store` (spawn_blocking) → DTO → JSON (gzip, ETag).
2. **Command**: component → `useMutation` → POST → service mutates stores → emits `*.changed` event → SSE → client `invalidateQueries` (no manual cache patching except optimistic review grading).
3. **Turn**: POST `/chats/{id}/turn` → `202 {turn_id}` immediately → spawned task runs `run_to_rest` → `chat.delta/tool_call` SSE events → `chat.rest` with new `view_version` → client refetches `ClassroomView`. Deltas are display-only; persisted truth is the `ChatRecord` at rest.

## Concurrency rules (server)
- `WorkspaceRegistry`: `DashMap<WsId, Arc<WorkspaceHandle>>`, lazily opened, idle-closed after 10 min with no streams/jobs. Handle owns stores, `Workbench`, event bus, per-chat `tokio::Mutex`.
- All sync store/FS work in `spawn_blocking`. `lp-core` stays pure.
- Global `Semaphore` (default 3, setting) caps concurrent CLI subprocesses across all workspaces; waiters visible as `queued` in job/turn status.
- One active turn per chat; second POST → `409 turn_in_progress`.
- Graceful shutdown (SIGINT/SIGTERM): stop accepting, cancel turns (tokens), flush JSONL, mark running jobs `interrupted` (existing `recover()` resumes next boot).

## Security model (local but real)
- Bind `127.0.0.1` only (IPv6 `::1` too). Never `0.0.0.0`.
- Launch prints and opens `http://127.0.0.1:PORT/?t=<token>`. `GET /` with valid `t` sets cookie `lp_session` (HttpOnly, SameSite=Strict, Path=/) and redirects to `/` without the token. No cookie → API returns 401 and SPA shows "Open from the terminal link" screen.
- Every request: `Host` must be `127.0.0.1:PORT` or `localhost:PORT` (defeats DNS rebinding). Mutations require `Origin` equal to own origin **and** header `X-LP-CSRF: 1` (cookie-only is not enough). No CORS headers at all. Dev proxy rewrites Host/Origin.
- CSP on SPA: `default-src 'self'; img-src 'self' data:; style-src 'self' 'unsafe-inline'; connect-src 'self'; frame-ancestors 'none'`. No external hosts.
- Responses never contain OAuth tokens or keychain data; account DTO exposes provider, email, tier, status, cooldown only. Paths shown are user's own, allowed.
- Uploads (PDF): size cap 50 MB, extension+magic check, stored under workspace `artifacts/`, never executed.
- Subprocess sandbox rules unchanged (empty `/tmp/learnpuppy-sandbox`, stripped env).

## Run modes
| Mode | Command | Notes |
|---|---|---|
| Dev | `make dev` = `cargo watch -x 'run -p lp-server -- serve --dev'` + `bun --cwd web dev` | Vite proxies `/api` and SSE; `--dev` disables token (127.0.0.1 only, prints warning) and enables `LP_ENGINE=faux` by default unless `--real` |
| Prod | `cargo build --release -p lp-server` after `bun run build` | SPA embedded via `rust-embed` (debug builds read from disk) |
| Test | `learnpuppy serve --fixture <dir> --engine faux --port 0` | Playwright global-setup starts it, reads port from stdout JSON line |

## Observability
- `tracing` JSON logs to `~/.learnpuppy/logs/server-<date>.log`; request id per request (`X-Request-Id`) echoed in error DTOs; UI "Copy diagnostics" button (Settings) bundles version, workspace stats, last 200 log lines (secrets redacted by existing `redact`).
