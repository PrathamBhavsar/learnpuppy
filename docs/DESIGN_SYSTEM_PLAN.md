# Design System Plan — shadcn `buFzGJc` (base-lyra) + torlink palette

Goal: LearnPuppy gets one design system. Rust app stays working. Web lab proves it visually. Tokens/icons/font flow into `lp-ui`.

## Facts (verified 2026-10-28)
- Preset `buFzGJc` = `base-lyra`: Base UI, baseColor `neutral`, radius `0.625rem`, iconLibrary `hugeicons`, font Geist Mono, `--font-heading: var(--font-mono)`, menuAccent `bold`.
- Torlink palette (`src/ui/theme.ts`): accent `#a78bfa`, text `#e9e4f5`, alt `#b9a7e6`, bright `#d8b4fe`, good `#86d6a2`, warn `#f0c560`, bad `#ee7d92`, rule `#6b6577`, bg `#0a0810`, sprout `#5ae87a`; source hues teal `#5fd0c5`, orange `#f6a55c`, blue `#7db8f0`.
- humanlayer/skills has no design skill (only loop/rpi/show-me/visual-pr/claude-md/react-props). Three already installed in `.claude/skills`. Install `narrow-react-prop-types` for the web lab only.
- Hugeicons = icons only. Font = preset Geist Mono (user confirmed).

## Architecture (hybrid)
1. `web/` — bun + Next lab, created with `bunx --bun shadcn@latest init --preset buFzGJc --template next`. Torlink palette overrides colour vars only; structure/radius/font/icons stay preset-exact. Page `/` = component gallery (Button, Badge, Card, Dialog, Tabs, Tooltip, Progress, Slider, Switch, Command, graph-node chip).
2. `crates/lp-ui` — port tokens (`theme.rs` OKLCH), Geist Mono (embedded font asset), Hugeicons SVG paths (`icons.rs`). Same class names, no API break.
3. Truth check — screenshot web gallery vs `lp-app` per screen; `npx e2e run` stays green.

## Token map (dark-first, converted to OKLCH by script, not by hand)
| shadcn var | torlink |
|---|---|
| background | #0a0810 |
| foreground | #e9e4f5 |
| card/popover | bg lifted ~+4% L |
| primary / ring | #a78bfa (fg #0a0810) |
| accent | #d8b4fe (fg #0a0810) |
| secondary / muted | bg +8% L; muted-fg #b9a7e6 |
| border / input | #6b6577 at 30% / 40% alpha |
| destructive / success / warning / info | #ee7d92 / #86d6a2 / #f0c560 / #7db8f0 |
| chart-1..5 | #a78bfa #d8b4fe #5fd0c5 #f6a55c #86d6a2 |
| mastery novice/learning/mastered | #ee7d92 / #f0c560 / #86d6a2 |
Light mode: preset neutral light kept, primary swapped to #7c5cd6 for contrast.

## Waves (disjoint files per wave)
| ID | Owner | Model / effort | Files | Done when |
|---|---|---|---|---|
| W1 web-init | Sonnet 5.5, high | claude `sonnet` | `web/**` | `bun run build` ok, gallery renders, preset diff = only palette |
| W2 tokens | Codex gpt-5.4, high | codex | `scripts/hex2oklch.ts`, `crates/lp-ui/src/theme.rs` | script emits tokens, Rust test asserts every var present, `cargo test -p lp-ui` |
| W3 icons+font | Codex gpt-5.4, high | codex | `scripts/export-icons.ts`, `crates/lp-ui/src/icons.rs`, font asset | all existing `Icon*` fns kept, now Hugeicons paths; SSR test per icon |
| W4 gui test | Antigravity `gemini-3.1-pro-high`, high | agy | `tests/**`, `e2e.config.ts` | e2e covers web gallery + lp-app screens; screenshots saved |
| W5 audit | Opus (claude) high | claude `opus` | read-only | `make gate`, e2e green, palette/preset diff, no `wasm_bindgen`/emoji icons |

Order: W1 ‖ W2 → W3 → W4 → W5. Orchestrator (me) merges `--no-ff` to `staging`, never touches `prod`.

## Rules
- bun only in `web/` (no npm there). Root `package.json` e2e stays as is.
- No hand-picked OKLCH: always from hex via script.
- `lp-ui` forbids wasm/js crates; font via `include_bytes!`/data URI.
- Commit: short lowercase `feat:`/`fix:`/`test:`/`docs:`.
