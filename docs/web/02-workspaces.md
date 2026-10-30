# 02 — Multiple Workspaces

## Definition
A **workspace** is an isolated knowledge base: its own Knowledge Graph, chats, workbench jobs/artifacts, FSRS cards, decisions, ingest cursors, and FTS index. Think "Rust async", "Distributed systems course", "Work repo X".

Shared across all workspaces (process-global): CLI accounts + rate-limit pool, settings profiles (role → provider/model/effort), device id, UI preferences, server token.

## Metadata (`workspace.json`)
```json
{
  "id": "ws_a1b2c3d4", "name": "Rust async", "slug": "rust-async",
  "icon": "book-open", "hue": "violet",
  "description": "", "created_at": 1761700000000, "last_opened_at": 1761700000000,
  "archived": false,
  "sources": { "claude": true, "codex": true, "agy": true, "git": true,
               "allow": ["/Users/pratham/code/foo"], "since_days": 30 },
  "overrides": { "profiles": {} },
  "schema": 1
}
```
- `icon` = key from a fixed Hugeicons allow-list (24 icons); `hue` = one of 8 palette accents (tints the workspace avatar/sidebar chip only; theme stays torlink violet). No emoji.
- `slug` unique, URL-safe, only for display; routing always uses `id`.

## Disk layout
```
~/.learnpuppy/
  state.json                 { last_workspace, device_id, schema }
  settings.json              global profiles/pins (existing file, unchanged)
  server.lock                pid/port while running
  logs/  .trash/
  workspaces/
    ws_a1b2c3d4/
      workspace.json
      graph/ nodes.jsonl chains.jsonl coverage.jsonl sessions.jsonl positions.json
      chats/<id>.json
      jobs/            artifacts/
      tracker/ index.db  sync-<device>.jsonl
```
Each workspace root is passed to the existing store constructors, so `lp-store` needs **no schema change** (verify in V02: constructors take a root dir; if any path is hardcoded to `~/.learnpuppy`, fix there).

## Migration (existing single-store users — you have data)
Trigger: on `serve`/`migrate`, if `~/.learnpuppy/graph` or `chats` exists at root and `workspaces/` does not.
1. Take lock; refuse if another LearnPuppy process (legacy TUI) holds JSONL files open.
2. Create `workspaces/ws_<id>/` named **"Default"**.
3. **Rename (move), not copy** `graph, chats, jobs, artifacts, tracker, index.db*` into it (same filesystem ⇒ atomic, zero extra disk — disk is tight).
4. Write `.migrated-v1` marker with the list of moved entries and old→new map; `learnpuppy migrate --revert` reverses by the marker.
5. Idempotent: marker present ⇒ no-op. Partial failure ⇒ marker `in-progress` lists done moves; rerun resumes.
6. Test with a golden fixture of the real layout, plus a kill-in-the-middle test.

## Registry & lifecycle (`lp-service::workspace`)
| Operation | Behavior |
|---|---|
| list | scan `workspaces/*/workspace.json`; stats (nodes, chains, chats, due cards, last activity) computed lazily + cached 30 s; archived hidden by default |
| create | validate name (1–60, unique case-insensitive among live); create dirs; optional "seed sources from detected repos"; returns handle |
| open | lazy `WorkspaceHandle`; idle-close after 10 min |
| rename / edit icon,hue,description,sources | PATCH; atomic write of `workspace.json` |
| archive / restore | flag; archived are read-only (mutations → 409 `workspace_archived`) |
| delete | requires `confirm_name` in body; running turns/jobs aborted first; dir moved to `.trash/<ts>-<id>/`; restore from Settings → Trash; purge button |
| export | `POST …/export` → job producing `artifacts-free` zip: `workspace.json`, ledgers, chats, tracker rows (as JSONL, not the sqlite file), manifest with checksums. Download via `GET /api/v1/downloads/{token}` |
| import | multipart zip → validate manifest/checksums/schema → new workspace id (never overwrites); rebuild `index.db` from JSONL |
| duplicate | export+import internally (v1 optional; skip if time) |

## Routing & switching
- URLs: `/` (home / workspace picker), `/w/:ws/graph`, `/w/:ws/learn`, `/w/:ws/learn/:chat`, `/w/:ws/review`, `/w/:ws/digest`, `/w/:ws/ingest`, `/w/:ws/settings`; global: `/accounts`, `/settings`, `/trash`.
- Switcher (sidebar header + ⌘K "Switch workspace…" + `⌘1…⌘9`): changes only the `:ws` param, keeps the current section (`/w/A/review` → `/w/B/review`). TanStack Query keys are namespaced `['ws', id, …]`, so switching never shows stale data; previous workspace cache is kept 5 min for instant back-switch.
- `last_workspace` in `state.json` decides where `/` redirects when exactly one live workspace exists or the user ticked "open last"; otherwise picker.
- Unknown/archived/deleted `:ws` ⇒ dedicated "Workspace not found" page with picker, never a blank screen.
- Two tabs on two workspaces work simultaneously (each has its own SSE stream filtered to its workspace + global events).

## Ingest scoping (the hard part)
Ingest reads **machine-global** transcripts (`~/.claude*/projects`, Codex, AGY, git). Workspace owns an allow-list of directories (`sources.allow`) and source toggles. Rules:
1. `ingest(allow, since_ms)` is called with the workspace's list; cursors live in the workspace's `index.db`, so each workspace tracks its own progress.
2. Overlap detection when saving sources: if a path equals/contains/is-contained-by an allow path of another live workspace, UI shows a warning ("these sessions will be ingested into both") — allowed, not blocked.
3. "Detect" button lists candidate project dirs (from transcript cwd's seen in the last N days) with counts, so the user ticks instead of typing paths.
4. Empty allow-list ⇒ ingest disabled with an explanatory empty state (never "ingest everything" by accident).

## Cross-workspace features (v1 = only these)
- Global ⌘K search fans out to each live workspace's FTS (`propositions_fts`), results grouped by workspace; selecting jumps to `/w/:ws/graph?node=…`.
- Home shows per-workspace due-card badge; global "Due today: N across M workspaces" with a one-click "Review all" that walks workspaces sequentially (route `/review-all`).
- Accounts/rate-limits are global: a cooldown hit in workspace A is visible in B.

## Edge cases to handle explicitly
- Delete workspace while a turn streams in another tab ⇒ that tab receives `workspace.deleted` and navigates to picker with a toast.
- Two workspaces with same name ⇒ rejected case-insensitively; imported name collision ⇒ suffix " (2)".
- Disk full during write ⇒ surfaced as `storage_full` error; ledger append is atomic-line, no partial JSONL corruption (existing atomic writes; add test).
- `workspace.json` corrupt ⇒ workspace listed as "damaged" with Repair (rewrite minimal metadata from dir name) / Reveal path actions.
- First run, zero workspaces ⇒ onboarding (06 §Home).
