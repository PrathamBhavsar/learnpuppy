# 04 — Frontend Foundation (`web/`)

## Step 0: init (exact, after V00 verification)
```bash
cd /Users/pratham/code/learnpuppy
git rm -r web                                   # old Next lab (palette already captured in scripts/hex2oklch.ts + 05)
bunx --bun shadcn@latest init --preset buFyzGS --base radix --template vite --rtl --pointer
# if shadcn asks for a name/dir: `web`, package name learnpuppy-web
```
**V00 verification (before the real init, in the scratchpad dir):** run the same command, then record in `docs/web/04-preset-facts.md`: resulting `components.json` (style, iconLibrary, rtl flag), fonts it wires, radius, baseColor, what `--pointer` actually changes (expected: `cursor:pointer` on interactive elements; if the CLI rejects the flag, apply equivalent via a global rule), what `--rtl` changes (expected: logical-property classes `ms-/me-/ps-/pe-/start-/end-` and `rtl:` variants in generated components). Compare against the previous `buFzGJc` (base-lyra, Base UI) — radix primitives differ (`asChild`, `data-state` attrs, no `render` prop). Anything unexpected becomes a decision before W1.

## Stack (versions: latest stable at init, pinned by `bun.lock`, recorded in `web/README.md`)
React 19 · TypeScript (strict, `noUncheckedIndexedAccess`) · Vite · Tailwind CSS v4 (`@theme inline`, `tw-animate-css`) · shadcn (radix) · TanStack Router + Query · `openapi-fetch` + `openapi-typescript` · Zustand · `motion` · `lucide-react` · `sonner` (toasts, as shadcn) · `cmdk` (shadcn Command) · `react-hook-form` + `zod` (settings/workspace forms) · `react-markdown` + `remark-gfm` + `remark-math` + `rehype-katex` · `shiki` (lazy, per-language chunks) · `@dagrejs/dagre` · `d3-zoom`, `d3-quadtree`, `d3-ease`, `d3-interpolate` (modules only) · `date-fns` (or `Intl` only — prefer `Intl`) · Vitest · Testing Library · MSW · Playwright · `@axe-core/playwright` · ESLint (typescript-eslint, react-hooks, jsx-a11y, import/no-cycle) · Prettier · `knip` (dead code).
No: Redux, axios, lodash, moment, any CSS-in-JS, any CDN script.

## Source layout
```
web/src/
  main.tsx  app.tsx  router.tsx  routeTree.gen.ts
  routes/                      TanStack file routes (thin: params + feature component)
    __root.tsx  index.tsx  w.$ws.tsx (layout: shell)  w.$ws.graph.tsx  w.$ws.learn.tsx  w.$ws.learn.$chat.tsx
    w.$ws.review.tsx  w.$ws.digest.tsx  w.$ws.ingest.tsx  w.$ws.settings.tsx  accounts.tsx  settings.tsx  trash.tsx  review-all.tsx
  api/
    schema.d.ts (generated)  client.ts  errors.ts  keys.ts  events.ts (SSE manager)  queries/{workspaces,graph,chats,review,digest,ingest,accounts,settings}.ts
  features/
    shell/       Sidebar, WorkspaceSwitcher, Topbar, JobsPopover, AccountsPill, ConnectionBanner
    workspaces/  WorkspaceGrid, WorkspaceCard, WorkspaceDialog, Onboarding, DeleteDialog, TrashList
    graph/       GraphScreen, canvas/{renderer.ts,camera.ts,hit.ts,tween.ts,theme.ts}, NodeDrawer, ChainTable, SelectionBar, Legend, GraphList (a11y)
    classroom/   ChatList, Classroom, MessageList, Message, StreamingText, QuestionDock/{Probe,Recall,Quiz,Explain,AskUser,Problem}, ConfidenceSlider, PlanDag, PhaseStepper, JobsPanel, ArtifactViewer
    review/      ReviewScreen, ReviewCard, ChoiceList, RatingBar, SessionSummary, StatsStrip, Forecast, CardBrowser
    digest/      DigestScreen, DayGroup, SessionCard, DecisionDrawer
    ingest/      IngestScreen, SourcesEditor, RunPanel, RunHistory
    accounts/    AccountsScreen, AccountCard, RoleSlots, AddClaudeDialog
    settings/    SettingsScreen, ThemeToggle, ShortcutsSheet, DiagnosticsCard
    palette/     CommandPalette, registry.ts (commands contributed by features)
  components/
    ui/          shadcn-generated (never hand-edit structure; theme only via tokens)
    app/         PageHeader, EmptyState, ErrorState, Skeletons, Stat, Kbd, MasteryRing, CodeBlock, Markdown, CountUp, Hotkey, ShortcutHint
  lib/           cn.ts, motion.ts (variants/tokens), format.ts (Intl), keys.ts (hotkey registry), a11y.ts (announce), idle.ts, invariant.ts
  hooks/         useHotkey, useReducedMotion, useWorkspace, useLiveQuery, useResizeObserver, useEventSource
  stores/        graphView.ts (camera, selection, filters), palette.ts, ui.ts (sidebar collapsed, theme)
  styles/        index.css (imports), tokens.css (generated colors), motion.css, canvas.css
  test/          setup.ts, msw/{handlers.ts,fixtures/*.ts}, render.tsx
web/e2e/         playwright.config.ts, global-setup.ts, fixtures/, specs/*.spec.ts, visual/*.spec.ts
web/scripts/     hex2oklch.ts, gen-api.ts, seed-fixture.ts
```
Rules: `features/*` may import `components`, `lib`, `api`, `stores`; never each other (cross-feature via `palette` registry or route links). Enforced by ESLint `import/no-restricted-paths`. Max file ~300 lines; split by responsibility.

## Data layer
- `client.ts`: `createClient<paths>({ baseUrl: '/api/v1', credentials: 'same-origin', headers: {'X-LP-CSRF':'1'} })`; middleware maps error envelope → typed `ApiError` (code, requestId, fields).
- Query keys (in `keys.ts`, typed factory): `['bootstrap']`, `['ws', id, 'graph'|'chats'|'chat', chatId|'review','queue'|…]`, `['accounts']`, `['settings']`.
- Defaults: `staleTime: 30s`, `gcTime: 5min`, `retry: (n, e) => e.retryable && n < 3`, `refetchOnWindowFocus: false` (SSE is the freshness mechanism), `networkMode: 'always'`.
- **SSE manager** (`events.ts`): one `EventSource` per tab for the *current* workspace route (re-opened on `:ws` change); exponential reconnect (0.5 → 8 s, jitter); on reconnect sends `Last-Event-ID` (native). Maps events → `queryClient.invalidateQueries` per table in 03, and feeds transient streams to `useStreamingTurn(chatId)`. Connection state exposed to `ConnectionBanner` ("Reconnecting… your work is safe").
- Optimistic updates only where latency is felt: review grade (advance to next card immediately, rollback + toast on error), chat user message echo, workspace rename. Everything else waits for server truth.
- Errors: global `ErrorBoundary` per route; `ApiError` with `auth_required` ⇒ full-screen "open from terminal link"; `engine_unavailable` ⇒ inline banner with link to Accounts.

## State rules
- Server data: only in TanStack Query. Never copied into Zustand.
- Zustand only for: graph camera/selection/filters, palette open/query, sidebar collapse, local UI prefs (persisted `localStorage` with try/catch fallback).
- URL is state for: workspace, section, chat, selected graph node (`?node=`), review mode, digest range (`?from&to`), ingest source tab. Back/forward must work; shareable within the machine.

## Conventions
- Every interactive element: `data-testid` (kebab, feature-prefixed: `graph-zoom-in`, `quiz-option-A`), proper role/label. A lint rule (custom, tiny) fails on `<button>/<a>/<input>` without accessible name or testid in `features/**`.
- Logical CSS only (`ms-*, me-*, ps-*, pe-*, start-*, end-*, text-start`); a lint grep in CI bans `ml-|mr-|pl-|pr-|left-|right-|text-left|text-right` in `src/` (exceptions allow-listed with comment, e.g. canvas).
- Dates/numbers via `lib/format.ts` (`Intl`, locale from browser); no string-concatenated plurals (use `Intl.PluralRules`).
- Copy: sentence case, no emoji, no exclamation marks, concrete verbs. Strings centralised per feature in `copy.ts` for consistency (not full i18n).
- Root layout invariants: `h-dvh w-dvw overflow-hidden`, only inner panes scroll, `scrollbar-gutter: stable` on panes, no layout shift on load (skeletons sized to content).
- Never block the main thread >50 ms: markdown/shiki/layout heavy work is lazy/chunked (`requestIdleCallback`/worker for graph hit index build if >5k nodes).

## Build & bundle budgets
- `vite build` main chunk ≤ 220 kB gzip (shell + router + query); route chunks lazy; `shiki`, `katex`, `dagre`, graph canvas lazy; fonts subset to latin (+ latin-ext) woff2.
- Source maps in dev only. Hashed assets served `immutable`.
- `bun run check` = `tsc -b && eslint . && knip && vitest run && vite build` (the web half of `make gate`).

## Dev proxy (vite.config.ts)
`server.proxy['/api'] = { target: 'http://127.0.0.1:7878', changeOrigin: false, configure: set Host+Origin to target }`; SSE needs `proxy.timeout: 0` and no buffering. `--dev` server mode skips the token.
