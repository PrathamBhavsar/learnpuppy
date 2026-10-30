# 07 — Testing & Quality

## Pyramid
| Layer | Tool | Scope | Runs |
|---|---|---|---|
| Rust unit/prop | cargo test | `lp-core` etc. unchanged (must stay green), `lp-service` logic | `make gate` |
| Rust integration | tower `oneshot`, tempdir, `FauxEngine` | every endpoint, auth, SSE, turn FSM, migration | `make gate` |
| Contract | `openapi.json` drift check; MSW built from it; response-vs-schema validation in Playwright (ajv) | no FE/BE drift | `make api-check` |
| FE unit/component | Vitest + Testing Library + MSW | hooks, stores, format, each component's states (5 states rule), keyboard behavior | `bun run test` |
| E2E | Playwright (system Chrome; WebKit if disk allows) vs real server (`--fixture --engine faux --port 0`) | journeys in 10 | `make e2e` |
| Visual | Playwright screenshots per screen/state × light/dark × LTR/RTL, threshold 0.1% | regression on every PR touching `web/` | `make e2e` |
| A11y | `@axe-core/playwright` on every screen/state; zero serious/critical | | `make e2e` |
| Perf | Playwright traces + custom metrics on 5k/10k-node fixtures | budgets below | `make perf` (nightly/manual) |
| Real-engine smoke | manual script `scripts/real-smoke.md`: one real `claude` turn in a scratch workspace | not in CI | before release |

## Fixtures
- `learnpuppy seed --fixture small|medium|large|huge --out <dir>`: deterministic (seeded RNG) workspaces: small = 40 nodes; medium = 800 nodes/1.2k chains/6 chats/120 cards/30 decisions; large = 5k; huge = 10k. Includes: a chat at each phase (probe pending, plan, teach with pending quiz, practice with problem, done), jobs in each status, cards in each FSRS state, ingest runs with errors. Two workspaces + one archived + one damaged.
- `FauxEngine` scripts (JSON) driving full scenarios incl. rate-limit then recover, malformed tool args (argument_heal), engine crash mid-turn.
- Time control: server `--now <epoch>` / `POST /__test/clock` in test builds only (compiled behind `cfg(feature="test-hooks")`, off in release) so FSRS due logic and relative times are deterministic.

## Browser test hooks
Test builds expose `window.__lp = { graph: {nodeCount, selection, camera}, sse: {state}, motion: {running()} }`, tree-shaken from production (`import.meta.env.MODE==='test'`). Canvas nodes are asserted via hooks + the a11y GraphList, never pixel guessing (pixel diffs only in visual specs).

## Budgets (fail the gate)
| Metric | Budget |
|---|---|
| Main JS gzip | ≤ 220 kB; any route chunk ≤ 120 kB (graph, classroom may be ≤ 180 kB) |
| LCP on medium fixture (cold, localhost) | ≤ 1.2 s; TTI ≤ 1.8 s |
| Graph 5k nodes | pan/zoom p95 frame ≤ 16.7 ms (≤ 20 ms hard); first paint of layout ≤ 400 ms after data |
| Graph 10k nodes | performance mode auto; p95 ≤ 20 ms |
| Route switch | ≤ 100 ms to first meaningful paint (cached), no long task > 50 ms |
| Review grade → next card visible | ≤ 200 ms |
| Streaming | main thread long tasks ≤ 50 ms while 30 deltas/s render |
| Graph payload (5k) | ≤ 1.5 MB gzip; server `GET /graph` p95 ≤ 150 ms warm |
| Memory | 30 min session on medium fixture: heap growth < 30 MB (leak check for SSE/graph) |

## Gates
`make gate` (existing) extends to:
```
gate: fmt clippy test okf api-check web-check
web-check: cd web && bun install --frozen-lockfile && bun run check   # tsc, eslint, knip, vitest, vite build, bundle-size, contrast, rtl-lint
e2e: build web, cargo build -p lp-server, playwright test
```
`make ci` = gate + deny + e2e. Pre-merge rule unchanged: run `make gate` (and `make e2e` when `web/` or `lp-server` changed) before merge to `staging`.

## Lint rules worth automating (cheap, high value)
1. No physical-direction Tailwind classes (RTL).
2. No `fetch(`/`EventSource(` outside `src/api` (single data path).
3. No hex/rgb/oklch literals outside `styles/tokens.css` (and `canvas/theme.ts` reads vars).
4. Every `<button>`/`<input>` in features has name/label (jsx-a11y) + `data-testid` for primary controls.
5. No `animate`/`transition` on layout properties (stylelint-style regex on `transition-(all|\[?width|height|top|left)`).
6. `import/no-cycle`, feature isolation, `knip` dead exports.
7. Server: `#![forbid(unsafe_code)]`, clippy pedantic subset, no `unwrap()` in routes.

## Definition of "tested" for a feature
Unit/component test for non-trivial logic · MSW-backed component test for the 5 states · one E2E journey step · axe clean · visual baseline in light+dark (+RTL for shell and one data-heavy screen). PR without these is not done.

## Disk-aware CI note
Disk is tight (1.4 GB free at plan time). Before large builds: after dropping Dioxus run `cargo clean` once (frees the large `aarch64-apple-darwin` dioxus artifacts), keep one shared `CARGO_TARGET_DIR`, use `channel:'chrome'` in Playwright (no browser download), and add `scripts/disk-guard.sh` which aborts `make e2e` / `make perf` if < 3 GB free with a clear message.
