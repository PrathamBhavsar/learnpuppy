# 03 — Server API

Base: `/api/v1`. JSON, camelCase on the wire (serde `rename_all = "camelCase"` in DTOs only; `lp-core` types untouched). Timestamps = epoch ms (i64). IDs are strings. All workspace routes are under `/workspaces/{ws}`.

## DTO rule
`lp-core`/`lp-store` types are **never** serialized directly. `lp-server/src/dto/*` defines `#[derive(Serialize, ToSchema)]` structs with `From<core>` impls. Reason: wire stability, camelCase, no `SmolStr`/internal fields leaking, OpenAPI derivation. Every DTO has a round-trip unit test and appears in `openapi.json`.

## Conventions
- Errors: `{ "error": { "code": "turn_in_progress", "message": "…", "details": {…}, "retryable": false, "requestId": "…" } }` with proper status. Codes are an enum in OpenAPI (`not_found, validation, conflict, turn_in_progress, workspace_archived, storage_full, engine_unavailable, rate_limited{retryAfterMs}, auth_required, internal`).
- Lists: `?limit&cursor` → `{items, nextCursor}` where lists can be large (chats, cards, decisions). Graph is a snapshot, not paged.
- Conditional GET: graph, review stats, accounts return `ETag`; client sends `If-None-Match`; 304 is cheap. gzip/br via `tower-http`.
- Mutations are idempotent where retried by the UI (`clientId` UUID in grade/turn/answer; duplicate `clientId` returns the original result).
- Validation errors list fields: `details.fields[{path,message}]` so forms can attach messages.

## Endpoints

### Bootstrap / global
| Method Path | Purpose |
|---|---|
| `GET /health` | `{ok, version, engine: "real"|"faux", dataDir}` (no auth; Host-checked) |
| `GET /bootstrap` | one call at app start: server version, deviceId, workspaces summary list, accounts summary, unread banners (migration done, etc.) |
| `GET /events?workspace=ws_x` | **SSE**, see below |
| `GET /search?q=&limit=` | fan-out FTS across live workspaces → `[{workspaceId, node{id,text,kind}, score}]` |
| `GET /accounts` | pool snapshot: id, provider, email, tier, status(`ready|cooldown|auth_expired|disabled`), cooldownUntil, requests, tokens in/out |
| `POST /accounts/rescan` | re-run discovery (new `CLAUDE_CONFIG_DIR` profiles etc.) |
| `POST /accounts/claude` | `{path}` add extra Claude profile dir; validates `.claude.json` |
| `PATCH /accounts/{id}` | `{disabled}` temporarily exclude from pool |
| `GET /settings` · `PUT /settings` | profiles: role→`{provider, model, effort?, fallback}`, pins, concurrency cap, theme default. `PUT` validates provider/model against `GET /settings/catalog` |
| `GET /settings/catalog` | known providers, model ids (from settings + discovered), effort levels |
| `GET /trash` · `POST /trash/{id}/restore` · `DELETE /trash/{id}` | deleted workspaces |
| `GET /downloads/{token}` | one-shot export file |
| `POST /diagnostics` | builds diagnostics text |

### Workspaces
| Method Path | Purpose |
|---|---|
| `GET /workspaces?archived=` | list + stats `{nodes, chains, chats, dueCards, lastActivityAt}` |
| `POST /workspaces` | `{name, icon, hue, description?, sources?}` |
| `GET /workspaces/{ws}` · `PATCH` | meta; PATCH edits name/icon/hue/description/sources/overrides |
| `POST /workspaces/{ws}/archive` · `/restore` | |
| `DELETE /workspaces/{ws}` | body `{confirmName}` → trash |
| `POST /workspaces/{ws}/export` | → `{jobId}`; completion event carries download token |
| `POST /workspaces/import` | multipart zip → `{workspaceId}` |

### Graph
| | |
|---|---|
| `GET …/graph` | `{version, nodes[{id,text,kind,sessionId,provenance,createdAt,degree}], chains[{id,from,to,kind,text?}], clusters[{id,label,nodeIds}], positions{id:[x,y]}|null, layoutVersion}`; `positions:null` + `layoutPending:true` while computing |
| `GET …/graph/nodes/{id}` | full node: text, provenance (chat, message index, snippet), incoming/outgoing chains, related nodes, linked cards |
| `GET …/graph/search?q=` | FTS (`search_propositions`) |
| `POST …/graph/relayout` | force recompute (ForceAtlas2) → emits `graph.changed` |
| `POST …/graph/delete-preview` | `{nodeIds}` → `{deleted, orphanedChains, blastRadius[]}` using `deletion_blast_radius` |
| `DELETE …/graph/nodes` | `{nodeIds, previewToken}` applies (token binds to preview content to prevent stale confirm) |
| `GET …/graph/chains?selection=a,b` | `chains_for_selection` → the 3-level chain table data |
| `POST …/graph/practice` | `{nodeIds}` → creates Practice chat seeded from selection → `{chatId}` |

### Learn (chats)
| | |
|---|---|
| `GET …/chats` | summaries `{id, kind, title, phase, updatedAt, messageCount, pending, jobs}` |
| `POST …/chats` | `{kind:"teach"|"practice", topic, seedNodeIds?}` → `{chatId}` and auto-starts first turn |
| `GET …/chats/{id}` | `ClassroomView` (below) |
| `PATCH …/chats/{id}` | rename |
| `DELETE …/chats/{id}` | to workspace-local trash (`chats/.trash`) |
| `POST …/chats/{id}/turn` | body is a tagged union: `{type:"user", text, clientId}` \| `{type:"answer", callId, answer, clientId}` \| `{type:"steer", text, clientId}`; → `202 {turnId}`; `409 turn_in_progress` |
| `POST …/chats/{id}/cancel` | cancels active turn; keeps partial assistant text marked `interrupted` |
| `GET …/chats/{id}/export.md` | markdown transcript |
| `GET …/jobs` · `POST …/jobs/{id}/abort` | workbench sub-agent jobs (`research|pdf|visual|demo`) |
| `GET …/artifacts/{id}` | job output (markdown/html/png) with correct content-type, `Content-Security-Policy: sandbox` for html |
| `POST …/sources/pdf` | multipart PDF → pdf workbench job attached to chat (`?chat=`) |

`ClassroomView` (single source the UI renders; derived server-side by `classroom_view` + `derive_plan_statuses`):
```
{ id, kind, title, phase: "probe"|"plan"|"teach"|"practice"|"done", viewVersion,
  messages[ { id, role: user|assistant|tool|steer|compaction, text, thinking?, interrupted?, calls[{id,name}] , createdAt } ],
  pending: null | { callId, tool: "probe"|"recall"|"quiz"|"explain"|"ask_user_question"|"problem",
                    prompt, options?[{key,text}] (already shuffled by core), allowFreeText, confidenceScale?,
                    problem?{mode, statement, rungs?} },
  plan: null | { nodes[{id,title,status:"current"|"visited"|"upcoming"}], edges[{from,to}], currentId },
  probeProgress: {asked, max},   // MAX_PROBE_QUESTIONS = 8
  verdicts: [{callId, correct, explanation?}],   // from core `verdict`
  jobs[{id,kind,status,title,artifactId?}],
  turn: {state:"idle"|"running"|"queued"|"error", turnId?, startedAt?} }
```
The server never sends the correct answer for an unanswered `quiz` (core already hides it; add a test that the DTO for pending questions has no `correct` field).

### Review (FSRS)
| | |
|---|---|
| `GET …/review/queue?limit=` | due cards (server `now`) `{id, front, options?[{key,text}] (4-choice), kind, sourceDecisionId?, due, state}`; answers withheld |
| `POST …/review/{card}/answer` | `{choiceKey, clientId, confidence?}` → `{correct, correctKey, explanation, rating, next:{due, intervalDays}, preview}`; server maps correctness+latency→rating suggestion; UI then may `grade` |
| `POST …/review/{card}/grade` | `{rating:1..4, clientId}` → updated card; uses `commit_review` (atomic card + log) |
| `GET …/review/preview/{card}` | `[again,hard,good,easy]` intervals (`fsrs::preview`) |
| `GET …/review/stats` | `{dueNow, dueToday, newToday, streak, reviewedToday, retention30d, forecast[14]}` |
| `GET …/review/cards?state=&q=` | browse |
| `POST …/review/sync` | `review::sync` + `export_cards` |

### Digest
| | |
|---|---|
| `GET …/digest?from=&to=&source=` | decisions grouped by day → session `{sessionId, source, project, summary, decisions[{id, title, basis, alternatives[], chosen, cardIds}]}` |
| `GET …/digest/decisions/{id}` | detail incl. git commits (`commits_for_session`) |
| `POST …/digest/decisions/{id}/cards` | (re)generate 4-choice cards (`validate_card_shape` errors surfaced) |

### Ingest
| | |
|---|---|
| `GET …/ingest/sources` | current allow-list + detection candidates `[{path, sessions, lastSeen, source}]` |
| `PUT …/ingest/sources` | save (with overlap warnings in response) |
| `POST …/ingest/run` | `{sinceDays?}` → `{runId}`; progress via `ingest.progress`; one run per workspace at a time |
| `GET …/ingest/runs` | history `{id, startedAt, finishedAt, report{sessions, newDecisions, newCards, skipped, errors[]}}` (persisted in workspace) |

## SSE protocol
`GET /events?workspace=ws_x` (workspace optional ⇒ global events only). Response `text/event-stream`, heartbeat comment every 15 s.
- Each event: `id: <u64 monotonic per connection-lineage>`, `event: <type>`, `data: <json>`. Server keeps a ring buffer (4096) per workspace stream; `Last-Event-ID` replays; if older than ring ⇒ first event is `resync` and client invalidates **all** queries.
- Types: `hello{serverTime, version}`, `workspace.created|updated|deleted`, `accounts.changed`, `graph.changed{version}`, `chat.created|updated`, `chat.delta{chatId,turnId,seq,kind:"text"|"thinking",text}`, `chat.toolCall{chatId,turnId,call}`, `chat.rest{chatId,viewVersion}`, `chat.error{chatId,turnId,error}`, `job.updated{job}`, `ingest.progress{runId,done,total,phase}`, `ingest.done{runId,report}`, `review.changed`, `export.ready{jobId,token}`, `resync`.
- Deltas are throttled server-side to ≤ 30/s per chat (coalesced text) to protect the main thread.
- Client rule: `chat.delta` appends to a **transient** streaming buffer keyed by `turnId`; `chat.rest` discards the buffer and refetches `ClassroomView`. If a delta `seq` gap is seen ⇒ discard buffer, refetch.

## Turn state machine (per chat)
```
idle ──POST turn──▶ running ──run_to_rest ok──▶ idle (pending question maybe set)
                      │  ├─ cancel ───────────▶ idle (partial kept, interrupted=true)
                      │  ├─ engine error ─────▶ error (retryable? → POST same clientId again)
                      │  └─ rate_limited ─────▶ queued (pool cooldown) → auto-resume when slot frees, or surface after 60 s
```
- `answer` is only valid when `pending.callId` matches (else `409 stale_question` with the fresh view embedded).
- Server restart mid-turn ⇒ chat opens in `error{interrupted}` with "Resume" (re-runs last step; stateless agents rebuild from transcript, so this is safe).
- Steers typed while running are queued (`Msg::Steer`), delivered by the existing Wake pump.

## OpenAPI pipeline
`learnpuppy openapi > web/openapi/openapi.json` (utoipa). CI (`make api-check`) regenerates and `git diff --exit-code`s. `web/scripts/gen-api.ts` → `src/api/schema.d.ts`. MSW handlers are generated from the same file (`msw-auto-mock` or hand wrappers typed by schema) plus hand-written realistic fixtures.

## Server tests (Rust)
- Router tests with `tower::ServiceExt::oneshot` on a temp data dir + `FauxEngine`: one per endpoint (happy + 1 error).
- Auth tests: no cookie 401, wrong Host 421, cross-Origin POST 403, missing CSRF header 403.
- SSE tests: ordering, replay from `Last-Event-ID`, resync on overflow, heartbeat.
- Turn tests: full Probe→Plan→Teach scripted with `FauxEngine`; cancel; double-POST 409; stale answer 409; restart mid-turn.
- Migration + workspace lifecycle tests (02).
