# 09 — Grilling the Plan: Issues Found & Resolutions

Each item: what could go wrong → resolution (already folded into 00–08 unless marked **OPEN**).

## Security & integrity
1. **Any website can call `localhost:7878` and drive paid CLI accounts / read your learning data** (CSRF, DNS rebinding). → Token+cookie, Host allowlist, Origin+`X-LP-CSRF` on mutations, no CORS, CSP (01). Test list in 03.
2. **Two processes appending JSONL** (second `serve`, leftover legacy TUI, CLI). → `server.lock`; migration refuses when files are held; single-instance reuse.
3. **Delete is irreversible loss of history.** → Trash (D19), typed-name confirm, blast-radius preview with token binding (stale confirm impossible).
4. **Artifacts (HTML from sub-agents) could run scripts with our origin.** → iframe `sandbox=""` + `Content-Security-Policy: sandbox` on artifact responses + separate content-type handling.
5. **PDF upload abuse.** → size/type caps, stored inert, parsed only by existing pdf workbench job.
6. **Secrets in logs/diagnostics.** → reuse `redact`; diagnostics builder tested with planted fake tokens.

## Correctness
7. **Domain logic drifting into TS** (e.g. computing "correct", due dates, plan status). → D8 + A70 grep audit; server sends `verdict`, `preview`, `status`, positions; pending question DTO has no answer key (test).
8. **Streaming text vs persisted truth mismatch.** → Deltas are transient; `chat.rest` triggers refetch; seq-gap ⇒ discard; test with dropped/duplicated events.
9. **Double-submit** (answer, grade, turn) on slow network or Enter-mash. → `clientId` idempotency on server + disabled pending state in UI; test duplicates return same result.
10. **Stale question** (answered in another tab). → 409 `stale_question` carrying fresh view; dock re-syncs.
11. **Clock skew** affecting FSRS due. → server clock only (D20); test hook for time.
12. **Server restart mid-turn** leaves chat inconsistent. → Stateless agents rebuild from transcript; chat opens as `error{interrupted}` with Resume; test.
13. **SSE gaps after laptop sleep.** → `Last-Event-ID` replay, ring overflow ⇒ `resync` ⇒ invalidate all; heartbeat detects half-open (client watchdog 35 s ⇒ reconnect).
14. **HTTP/1.1 6-connection cap per origin breaks multi-tab SSE.** → one multiplexed stream per tab (D7); known limit ≈5 concurrent tabs documented; if ever an issue, add BroadcastChannel leader election (not v1).

## Workspaces
15. **Server-global "current workspace" breaks multi-tab.** → id in URL (D9).
16. **Migration of your real data destroys it.** → rename-not-copy (disk-safe), marker file, revert command, golden test, A70 verifies on a *copy* first; also take `tar` of `~/.learnpuppy` only if ≥ size free (check printed before run).
17. **Ingest ambiguity: which workspace owns a session?** → explicit allow-lists, overlap warning, empty list = disabled (02). No auto-assignment magic.
18. **Shared AccountPool vs per-workspace jobs starving each other.** → global semaphore, fair queue (FIFO per workspace round-robin), visible `queued` states.
19. **Many workspaces open = memory/file handles.** → lazy open, idle close, cap of 8 open handles (LRU).
20. **Global search latency across N workspaces.** → parallel FTS with 300 ms deadline per ws; partial results flagged.

## Frontend risk
21. **Preset mismatch**: previous lab used `buFzGJc` (Base UI/lyra); now `buFyzGS` + radix + `--rtl --pointer` — unknown deltas. → V00 verifies before anything is built; palette applies to color vars only; any structure change is a recorded decision.
22. **Canvas graph hurts a11y/testability.** → parallel virtualized GraphList, `window.__lp` hooks in test builds, keyboard nav spec'd (06).
23. **Animation jank / reduced motion ignored.** → transform/opacity only, central tokens, `MotionConfig`, `getAnimations()` test, perf traces with budgets.
24. **RTL regressions from physical classes.** → lint ban + Playwright RTL run + canvas explicitly LTR.
25. **Bundle bloat (shiki, katex, dagre, motion).** → lazy chunks per feature, budgets enforced in `web-check`.
26. **Fonts/icons loading from network** in an offline app. → fontsource local, Hugeicons tree-shaken; CSP blocks external anyway (will fail loudly in tests).
27. **SVG→Canvas rewrite of earlier graph code** loses proven interaction details from Dioxus version. → carry over behaviors list (cubic-bezier edges, hulls, provenance drawer, 3-level chain table) as acceptance items; layout math stays in Rust.
28. **Graph snapshot size at 10k nodes.** → gzip+ETag, performance mode, payload budget, positions separated (`positions:null` until ready) to not block first paint.

## Process / environment
29. **Disk: 1.4 GB free.** → V00 gate requires cleanup plan; `cargo clean` after dropping Dioxus; system Chrome for Playwright; disk-guard script (07). **Do not start V01+ before this is resolved** (a full disk mid-migration is the worst case).
30. **Unknown LLM output shapes break UI** (tool args malformed). → core `heal_encoded_arguments` already; DTO validation on server; UI renders unknown tool names as generic "tool" chips instead of crashing; Faux scripts include malformed cases.
31. **Playwright can't test the real CLIs reliably (quota, nondeterminism).** → Faux in CI, manual real-smoke script before release; the verification statement in A70 must say which were real vs faux (no overclaiming — lesson from the Dioxus session where headless Chrome ≠ real window).
32. **Scope creep** (workspace sharing, cloud, mobile). → non-goals list (00); new ideas go to `docs/web/backlog.md`.
33. **Docs/brain drift** (OKF strict gate fails). → D60 explicitly updates brain; `make okf` is part of gate from V01 on.
34. **Dev loop slowness** (two watchers). → `make dev` one command; `--dev` skips token; MSW mode `bun run dev:mock` lets frontend work with no backend/CLI at all.

## OPEN questions (defaults chosen; tell me to change)
1. **Directory name**: `web/` (replace the Next lab) — default. Alternative `apps/web`.
2. **Backend names**: `lp-service` + `lp-server`, binary `learnpuppy` — default.
3. **Playwright browsers**: system Chrome only (disk-friendly) — default; add WebKit later if you want Safari-engine coverage.
4. **Shall `legacy/tui-cli` stay on remote as is** (yes by default) and `lp-tui`'s headless CLI subcommands (auth/config/chat/graph/ingest/review) be dropped too, replaced only by `learnpuppy serve|migrate|seed|doctor|openapi`? Default: dropped, per "drop the tui and tui app part".
