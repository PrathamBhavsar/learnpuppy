# 06 — Screens (layout, states, interactions, keyboard)

Every screen must define and implement **five states**: loading (skeleton sized to content), empty, error (with retry + requestId), partial/degraded (e.g. engine down), and populated. "Done" = all five exist and have an E2E assertion.

## Global shell (`/w/:ws/*`)
```
┌──────────┬───────────────────────────────────────────────┐
│ ws chip ▾│ Topbar: breadcrumb · [⌘K search…] · jobs(●2) · accounts pill · theme │
│ Graph    ├───────────────────────────────────────────────┤
│ Learn    │                                               │
│ Review ③ │                 route content                 │
│ Digest   │                                               │
│ Ingest   │                                               │
│ ──────── │                                               │
│ Accounts │                                               │
│ Settings │                                               │
└──────────┴───────────────────────────────────────────────┘
```
- Sidebar 232 px (collapsed 56 px, `[` toggles). Items: icon + label + optional badge (Review = due count, Learn = chats awaiting answer, Ingest = new since last run). Active item has sliding indicator (shared layout).
- Workspace chip (top): avatar (icon+hue) + name + chevron → popover listing workspaces (search, ⌘1–9, "New workspace", "Manage all"), each with due badge.
- Topbar: breadcrumb (`Workspace / Section / Item`), search trigger (opens palette), **Jobs popover** (running chats/jobs/ingest across this workspace with cancel), **Accounts pill** (`2/3 ready`; amber on cooldown with countdown; red when none), theme toggle.
- `ConnectionBanner` under topbar when SSE is down >2 s.
- Hotkeys (global, listed in Shortcuts sheet `?`): `⌘K` palette · `g g/l/r/d/i` go-to (vim-style chords, 800 ms window) · `[` sidebar · `⌘1–9` workspace · `/` focus current screen search · `?` shortcuts · `Esc` close/blur.

## Home `/` — workspace picker & onboarding
- Header: "Workspaces" + New button + "Due today: N across M" (→ `/review-all`) + archived toggle.
- Grid of `WorkspaceCard`: icon/hue, name, description (2 lines), stats row (nodes · chains · chats), due badge, last activity relative time, `⋯` menu (rename, edit sources, export, archive, delete). Click opens at last visited section for that workspace (remembered per ws).
- **Empty / first run (onboarding, 3 steps in one page, not a wizard maze):** ① detected accounts (live from `/accounts`; if none: copy to run `claude login`, with Rescan) ② name your first workspace (icon/hue) ③ choose sources (detected project dirs with checkboxes, defaults none-checked-but-suggested) → "Create & start ingest". Skippable at ③.
- Damaged workspace card shows Repair/Reveal actions.
- Create/Edit dialog: name (live uniqueness check), icon grid (24), hue swatches (8), description, sources (optional now). Form via RHF+zod; server field errors mapped.

## Graph `/w/:ws/graph`
Layout: toolbar (top-start) · canvas (fills) · legend (bottom-start) · zoom/fit/relayout controls (bottom-end, square lyra buttons) · minimap (bottom-end above controls, toggle) · right **NodeDrawer** (400 px, opens on select) · bottom **SelectionBar** when ≥1 selected.
- Toolbar: search (FTS, results highlight + dim others, Enter flies to first), filters (source chips claude/codex/agy/git/chat, cluster/session dropdown, "min degree"), toggle hulls, toggle labels (auto/always/never), follow-new toggle, "Table" view toggle (chain table), count chip "812 nodes · 1.2k chains".
- Canvas interactions: drag=pan, wheel=zoom (ctrl/pinch precise), click=select, shift/⌘-click=toggle multi, drag with `Shift`=marquee, double-click empty=zoom in, double-click node=open in drawer + focus neighbors, right-click=context menu (Practice this, Copy text, Show source chat, Delete…), keyboard: arrows move selection to nearest neighbor in direction, `Tab` cycles, `Enter` opens drawer, `Esc` clears, `f` fit, `+/-` zoom, `Del` delete preview.
- **NodeDrawer:** full proposition text (editable? no — read-only, ledger is append-only), kind/source chips, birth session & date, provenance snippet with "Open source chat" link, incoming/outgoing chains list (click to hop), linked flashcards, mastery ring, actions: Practice this, Delete.
- **Chain table (3-level):** Level 1 groups (by source session/cluster), Level 2 chains, Level 3 propositions; sortable, virtualized, row click selects on canvas; export CSV.
- **SelectionBar:** "N selected" · Practice selected (creates practice chat via `/graph/practice`, then navigates) · Delete… (preview dialog showing blast radius with counts and list; require checkbox "I understand" when orphaned chains > 0) · Clear.
- States: empty (no nodes: explains graph is built from learning chats and ingest, two CTAs), layout pending (canvas with soft cluster silhouettes + "Computing layout…" and determinate progress if available), >10k nodes warning chip enabling "performance mode" (no edge alpha, labels off).
- a11y: below the canvas region an visually-hidden-but-focusable **GraphList** (virtualized listbox of nodes sorted by degree) mirrors selection/search; announced via `aria-live`.
- URL: `?node=<id>&q=<text>&src=claude,codex&cluster=<id>`.

## Learn `/w/:ws/learn` and `/learn/:chat`
Two-pane: **ChatList** (280 px; new chat button; each item: title, kind badge Teach/Practice, phase mini-stepper, unread/pending dot, relative time; context menu rename/export/delete) and **Classroom**.
- New chat dialog: kind (Teach/Practice), topic textarea (Teach) or selection summary (Practice), optional "use nodes from graph" picker, role/model preview of what will run (read from settings) + engine warning if no account ready.
- **Classroom split view (resizable, remembered):** left = conversation (MessageList + Composer); right = **Plan** panel (PlanDag + PhaseStepper + JobsPanel tabs). Collapsible on narrow widths.
- MessageList: user bubbles right-aligned (`end`), assistant left (`start`), tool-call rows (compact chips like "Quiz asked"), steer messages marked, compaction divider ("Earlier conversation summarized"), interrupted marker. Markdown rendering with code (shiki), math, tables, links external `rel=noopener`. Copy button per message. Stick-to-bottom behavior with "Jump to latest".
- **Composer:** textarea (autosize ≤ 8 lines), `Enter` send / `Shift+Enter` newline / `⌘Enter` also sends; disabled while a *question is pending* (the dock replaces it) — except "Steer" mode toggle allowing guidance during a running turn; attach PDF (→ pdf job); Stop button replaces Send while running (`Esc` stops).
- **QuestionDock** (replaces composer when `pending` set), per tool:
  - `probe`: free-text or choice; shows progress "Question 3 of ≤8".
  - `recall`: free text, "I don't know" button.
  - `quiz`: options A–D (keys `a–d`/`1–4`), confidence slider 1–5 before submit if `confidenceScale`; submit `Enter`; reveals verdict (#30 animation), explanation, "Next".
  - `explain`: textarea "explain in your own words", min length hint, submit.
  - `ask_user_question`: options and/or free text per payload.
  - `problem`: statement (markdown), mode (`ProblemMode`), answer editor (monospace), ladder rungs/hints if mode has them; Submit / "Give me a hint" (rung+1) per core gate; displays `Outcome`/`MissClass` feedback after conclude.
  Stale-question 409 ⇒ dock re-syncs silently with toast "Question updated".
- **PlanDag:** dagre layered layout (rankdir TB; LR on wide panels), nodes = squares w/ title (truncate + tooltip), status styling current/visited/upcoming, click node scrolls conversation to where it was taught (if mapped) or shows summary popover; zoom/pan with same d3-zoom helper, fit button.
- **PhaseStepper:** Probe · Plan · Teach (or Practice) with progress; read-only.
- **JobsPanel:** workbench jobs (research/pdf/visual/demo): status, elapsed, abort; artifacts open in **ArtifactViewer** sheet (markdown inline; html in sandboxed iframe `sandbox=""` with no scripts; images).
- Rate-limit/queue: banner in conversation "Waiting for an available model account (≈ 42 s)" with live countdown; cancel button.
- Error: turn error inline card with Retry (same `clientId`) and "Copy diagnostics".
- Empty list: friendly empty state with "Start learning something" input that creates chat in one step.

## Review `/w/:ws/review`
- Header strip (**StatsStrip**): due now, new today, reviewed today, streak, retention 30d; **Forecast** (14-day bars) collapsible.
- Start screen when due>0: big "Start review (N)" + options (limit, shuffle sources) ; due=0: "All caught up" with next due time and "Browse cards" + "Practice from graph".
- **Review session (focus mode, full-bleed, `Esc` exits with confirm if mid-card):** progress bar, card prompt (markdown, source decision link), 4 choices (`1–4` or `a–d`), after answering: correctness reveal + explanation + rating bar **Again/Hard/Good/Easy** with previewed intervals (`1–4` shortcuts, default highlight = server suggestion), `Space` = accept suggestion. Optimistic advance.
- Session summary: counts by rating, time, accuracy, mastery rings by topic, "next review" times, button back/again.
- **CardBrowser tab:** table with search, filter (state new/learning/review/relearning, due), card detail drawer (history of reviews with ratings/dates, stability/difficulty values in a "Details" disclosure).
- `/review-all`: walks workspaces sequentially with a workspace separator interstitial.

## Courses `/w/:ws/course` (`?id=` opens one)
- List plus "create from goal". Create runs placement (can take seconds; button shows pending, `503 engine_unavailable` toasts and stores nothing). Placement is one form of mcq/recall items, graded by the server in one `placement` call.
- **The wait:** while `status` is `writing` (polled every 2 s) the screen shows `misses` (question, answer, why) as a first mini lesson, plus "You got N of M right". `failed` shows the reason and Try again (`retry`). `ready` offers Start lesson.
- **Lesson loop:** each `next` returns a stored step. `explain` card (Got it -> `seen`), `ask` question (Answer -> feedback with answer and why; four self-grade buttons or Continue; "This looks wrong" -> `flag`; "Check my answer" -> `check` for missed recall), `waiting` (looks again every 4 s), `done` (summary with `usage` tokens). Ask-a-question (`ask`) sits under every step.
- Only `create`, `ask` and `check` need a model. Refused calls toast and leave the lesson as it was; explain/ask/answer/done keep working. No grading, shuffling or scheduling in TypeScript.

## Digest `/w/:ws/digest`
- Controls: date range (presets Today/7d/30d + custom), source filter chips, search.
- Day groups (sticky headers with counts) → SessionCard (source icon, project, time, one-line summary, decisions count, cards count) → expands to decisions list; click decision → **DecisionDrawer**: chosen option, basis (`DecisionBasis` rendered as chip + explanation), alternatives considered, related git commits, generated cards (open in Review browser), "Regenerate cards" with validation errors shown.
- Quiz-me CTA: "Review these 6 decisions" → starts a review session filtered to those cards.
- Empty: explains ingest, links to Ingest screen.

## Ingest `/w/:ws/ingest`
- **Sources:** toggles per tool (Claude/Codex/AGY/Git), allow-list editor (path chips + "Detect projects" dialog with counts and checkboxes + manual path input with validation `exists/readable`), "since" selector (7/30/90/all), overlap warnings.
- **Run panel:** big Run button; live progress (phase, sessions scanned/total, new decisions, new cards), cancel; one-at-a-time; last run summary.
- **History:** runs table with report details & errors (expandable), per-run "view new decisions" link into Digest filtered.
- Empty allow-list ⇒ explanatory state, Run disabled. Errors: unreadable path, AGY db locked (snapshot failed) etc., shown with actionable text.

## Accounts `/accounts` (global)
- Cards per provider group (Claude N, Codex, AGY): email, tier, status dot + label, requests/tokens, cooldown ring with remaining time, Disable/Enable switch, "Re-auth hint" with the exact command to run (`claude login` with `CLAUDE_CONFIG_DIR=…`) + copy button.
- "Add Claude profile" dialog (path validation), "Rescan".
- **Role slots:** table roles (prober, teacher, practice, extractor, auditor, pdf, research, visual, demo — from settings) × provider/model/effort/fallback with selects fed by catalog; Save with diff preview; "Test" button runs a 1-token ping through the chosen profile (uses FauxEngine in test mode) showing latency/result.
- Per-workspace overrides editable under `/w/:ws/settings` (same component, "inherit global" default).
- No account ready ⇒ prominent state with fix instructions.

## Settings `/settings` and `/w/:ws/settings`
- Global: Appearance (theme, direction, reduce motion override, density comfortable/compact), Behavior (confirm destructive, concurrency cap 1–6, auto-open browser), Keyboard shortcuts (read-only table), Data (data dir path + reveal, trash link, diagnostics copy), About (versions, engine mode).
- Workspace: general (name/icon/hue/description), sources link, overrides, export, archive, danger zone (delete with typed confirm).
- `/trash`: deleted workspaces with restore/purge (shows size, deleted at).

## Command palette (`⌘K`)
Groups: Navigate, Workspaces (switch/create), Actions (New chat, Run ingest, Start review, Relayout graph, Toggle theme), Search nodes (live FTS from current workspace, `Tab` to widen to all workspaces), Recent chats. Keyboard-only complete. Result rows show shortcut hints. Features contribute commands via `registry.ts`.

## Cross-cutting UI behaviors
- Every destructive action: confirm dialog stating consequence and (where relevant) counts; Esc cancels; focus returns to trigger.
- Every async button: pending state ≤ 100 ms to appear, prevents double-submit, uses `clientId`.
- Toasts: success short (3 s), errors persist with "Copy details" including requestId.
- Focus management: on route change focus moves to page `h1` (tabindex -1) and announces title via live region; dialogs trap focus; drawers restore focus.
- Scroll restoration per route; lists remember scroll when navigating back.
- Title: `document.title = "<Section> · <Workspace> · LearnPuppy"`.
- Favicon/dock icon simple; tab title shows `(3)` due count when hidden and cards due.
