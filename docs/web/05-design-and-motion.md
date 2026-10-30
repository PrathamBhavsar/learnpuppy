# 05 — Design System & Motion

## Principles
1. Dense, calm, technical. Square-ish "lyra" geometry (keep the preset's radius decisions; do not round things up). Mono typography gives the identity — don't add decoration to compensate.
2. Color is information (status, mastery, source), not decoration. One accent (violet), used sparingly.
3. Motion explains change (where did that come from / go) and rewards completion. Never decorative loops except loading.
4. Everything works with keyboard, in light and dark, in LTR and RTL, with reduced motion.

## Tokens
- Structure (radius, spacing, font stack, heading font var, menu accent) = **exactly the preset** as generated. We do not edit `components/ui/*` structure.
- Colors only are overridden in `src/styles/tokens.css`, **generated** by `scripts/hex2oklch.ts` from the hex source of truth (reuse the existing script/map from `docs/DESIGN_SYSTEM_PLAN.md`: bg `#0a0810`, fg `#e9e4f5`, primary/ring `#a78bfa` on `#0a0810` text, rule `#6b6577`, sprout `#5ae87a`, teal `#5fd0c5`, orange `#f6a55c`, blue `#7db8f0`).
- Semantic additions (names fixed now): `--success` (sprout), `--warning` (orange), `--info` (blue), `--mastery-0..4` (neutral→violet→teal→sprout ramp for mastery rings), `--source-claude|codex|agy|git` (graph/ingest chips), `--cluster-1..8` (graph hulls, from the 8 workspace hues).
- Light theme: derived via the same script with an inverted-lightness map; contrast verified by a script (`scripts/contrast.ts`) — every text/background pair ≥ 4.5:1 (≥ 3:1 for large/graphics); build fails otherwise.
- Theme switch: `light | dark | system`, stored in `localStorage` (+ `prefers-color-scheme`), applied by an inline pre-paint script in `index.html` (no flash). Switch animates with a 200 ms color cross-fade via View Transitions (falls back to instant).
- Canvas colors are read from CSS variables at theme change (`canvas/theme.ts`), never hard-coded.

## Typography
Preset font (Geist Mono) for body + headings (`--font-heading` = mono per preset). Scale (px): 11 meta / 12 small / 13 body-dense / 14 body / 16 lead / 20 h3 / 24 h2 / 32 h1. Line-height 1.45 body, 1.2 headings. Tabular numerals (`font-variant-numeric: tabular-nums`) on every number that changes (counters, intervals, timers).

## RTL & pointer
- Layout via logical properties only; icons that imply direction (chevrons, arrows, "back") flip with `rtl:-scale-x-100`; icons that don't (play, check, search) never flip.
- Canvas graph and plan DAG are `dir="ltr"` containers (spatial data); their surrounding chrome mirrors.
- Message bodies and user input use `dir="auto"` so an Arabic/Hebrew answer renders correctly inside an LTR UI. Code blocks always LTR.
- Keyboard: arrow-key navigation in lists/tabs follows reading direction (Radix handles it given `dir` on the provider; wrap app in Radix `DirectionProvider`).
- `--pointer`: interactive elements show pointer cursor (per V00 findings); disabled = `not-allowed`; draggable canvas = `grab/grabbing`.
- E2E runs the whole smoke suite once with `dir=rtl` (toggle in Settings → "Layout direction", also `?dir=rtl` in test builds) and screenshot-diffs against mirrored expectations.

## Motion system (`lib/motion.ts` + `styles/motion.css`)
Tokens:
| Name | Value | Use |
|---|---|---|
| `--dur-instant` | 80 ms | press feedback, hover color |
| `--dur-fast` | 140 ms | tooltips, popovers in, dim/highlight |
| `--dur-base` | 220 ms | dialogs, drawers, tab content, list item enter |
| `--dur-slow` | 360 ms | route/page transition, plan status change, card flip |
| `--dur-hero` | 600 ms | one-off celebratory (review complete, first node) — max one per screen |
| `--ease-out` | `cubic-bezier(.22,1,.36,1)` | entering |
| `--ease-in` | `cubic-bezier(.64,0,.78,0)` | exiting (exit = 70% of enter duration) |
| `--ease-inout` | `cubic-bezier(.65,0,.35,1)` | position/size changes |
| spring `snappy` | stiffness 520, damping 38 | draggable/slider/press |
| spring `gentle` | stiffness 220, damping 26 | layout reflow, camera |

Rules (enforced in review):
- Animate only `transform`, `opacity` (+ `filter: blur` ≤ 8px sparingly, `clip-path`, SVG `stroke-dashoffset`). Never width/height/top/left (use `layout` animations from `motion` which use transforms).
- Reduced motion (`prefers-reduced-motion` **or** Settings toggle): `MotionConfig reducedMotion="user"`; CSS durations → 0.01 ms; all transforms become opacity cross-fades ≤ 120 ms; canvas tweens jump; no autoplaying decoration. Test: Playwright `emulateMedia({reducedMotion:'reduce'})` asserts no running animations > 150 ms (`document.getAnimations()`).
- Stagger: 24 ms per item, capped at 8 items (rest appear together). Lists > 30 items never stagger.
- Interrupts: every animation is interruptible; a new route/selection mid-animation retargets instead of queueing.
- Frame budget: no animation may drop a frame budget >2 dropped frames/sec in the Playwright trace perf check on the fixture workspace (07).

## Animation catalogue (complete list; each is a task acceptance item)
**Global / shell**
1. Route change: outgoing fades+shifts 6px (instant-fast), incoming enters from 8px with fade (`--dur-base`, `--ease-out`); sidebar/topbar persist (no re-animate). Uses `motion` `AnimatePresence mode="popLayout"` keyed by route section.
2. Workspace switch: content cross-fades with 8px blur-to-sharp (View Transition named `ws-content`); sidebar workspace chip morphs (shared `layoutId`) hue/icon.
3. Sidebar collapse/expand: width spring `gentle`; labels fade at 50% of travel; tooltips appear when collapsed.
4. Dialog / Sheet / Popover / Dropdown / Tooltip / Command: Radix `data-state` + tw-animate (fade + 4% scale + 4px offset toward origin; sheet slides from `start`/`end` edge — mirrors in RTL); overlay fades. Exit faster than enter.
5. Toasts (sonner): slide+fade from `end-bottom`, swipe-to-dismiss, progress hairline for timed ones.
6. Command palette: results list items re-order with `layout` spring; selected row indicator slides (shared `layoutId`).
7. Connection banner: height-less slide (transform) from top; "reconnected" state flashes success tint 600 ms then leaves.
8. Skeleton shimmer: 1.6 s linear gradient sweep on transform, paused when reduced motion (static pulse-less block).
9. Buttons: press scale 0.97 (`instant`, snappy); loading state swaps label for spinner with width locked (no shift); success tick morph (check draws via dashoffset) for save actions.
10. Numbers (stats, counts, streak): `CountUp` tween 400 ms on change, tabular-nums, `aria-live=off` (value announced once on settle).
11. Focus ring: 120 ms ease; never animates position (avoids smear).

**Workspaces**
12. Cards enter staggered grid; hover lifts border color + 2px translateY; create-dialog → new card "flies" into grid slot (shared layout id) then pulses once.
13. Delete confirm: typed-name field enables destructive button with 140 ms color transition; card collapses (height via `layout`) on success.

**Graph**
14. Initial load: nodes fade in cluster-by-cluster (cluster order by size, 40 ms apart, cap 600 ms total); edges draw after nodes at 40% alpha.
15. New node (after merge/ingest): appears at its settled position with scale 0→1 spring + 600 ms accent glow ring decaying; camera does **not** jump unless user toggles "follow new".
16. Layout change (relayout or merge shifts positions): every node tweens old→new position over 600 ms `--ease-inout` (positions diff computed client-side from two layoutVersions); canvas rAF clock, auto-skips if >5k nodes (snap + fade).
17. Hover node: node scales 1.15 (instant), non-neighbors dim to 25% over `--dur-fast`, incident edges brighten and thicken; label for hovered always visible. Mouse-out reverses.
18. Select node: ring pulse once; drawer slides from `end`; camera eases to center node if it's off-center by >35% of viewport (fly-to 500 ms `--ease-inout`, zoom-adjust only if tiny).
19. Pan/zoom: inertial pan (friction 0.92/frame), wheel zoom smoothed (lerp 0.2/frame), pinch supported; double-click zoom-in 1.8× eased; "fit" button animates to bbox.
20. Multi-select (shift/marquee): marquee rectangle fades in; selected nodes get staggered ring (max 30 ms stagger cap 300 ms); selection bar springs up from bottom.
21. Cluster hulls: fade in below nodes at zoom-out, fade to 0 at zoom > 2.5 (LOD cross-fade); labels likewise.
22. Delete preview → confirm: affected nodes pulse red-tint, on confirm they shrink+fade (200 ms) before data refetch removes them.
23. Chain table (3-level): rows expand/collapse with height via `layout`, chevron rotates 90°(auto-flips RTL).

**Classroom**
24. Messages: new message enters with 8px rise + fade; assistant streaming text: tokens appear via opacity fade (per chunk, 120 ms), blinking-free caret (static block that gently pulses 1.2 s); auto-scroll sticks to bottom unless user scrolled up (then "Jump to latest" pill springs in).
25. Thinking block: collapsed "Thinking…" row with animated three-dot wave; expanding uses `layout` height; auto-collapses on completion.
26. Phase stepper (Probe → Plan → Teach/Practice): active step ring fills (stroke dash 360 ms); completed steps check-draw; transition into Plan triggers the plan DAG reveal (#27).
27. Plan DAG reveal: nodes appear along topological order (40 ms apart), edges draw via `stroke-dashoffset` after their source node.
28. Plan status change (upcoming→current→visited): node fill cross-fades, current node gets a slow 2 s soft pulse ring (disabled for reduced motion), visited gets check-draw; edge into current animates a travelling dash once.
29. Question dock enter: slides up 12px + fade, options stagger 24 ms; option hover = border accent; select = press + radio dot scale-in (snappy).
30. Quiz reveal: correct option flashes success tint + check draw; chosen-wrong shakes (±4px, 3 cycles, 240 ms — skipped under reduced motion → static red border + icon) while correct is revealed with a sweep highlight; explanation expands with `layout`.
31. Confidence slider: thumb snappy spring, value label follows with tabular-nums tween, track fill color interpolates low→high.
32. Probe progress: segmented bar (8 segments) fills one segment per answer with 200 ms scaleX from `start`.
33. Workbench job chips: status icon swaps (queued → spinner → check/alert) with scale/opacity morph; completed artifact chip bounces in once (spring) and opens viewer as Sheet.

**Review**
34. Card transitions: current card exits translating 40px toward the grading side (rating colored hint) + fade; next card enters from below with 0.96→1 scale (spring `gentle`). Under 200 ms from grade to next card visible (optimistic).
35. Answer reveal: card flips (rotateY 180°, 360 ms, `perspective 1200px`, backface hidden); for 4-choice, the choices stay and result overlay slides in instead of flipping (flip reserved for open-recall cards if introduced).
36. Rating bar: four buttons show previewed interval (e.g. "10m", "3d"); keypress 1–4 triggers the button's press animation so the keyboard feels physical.
37. Progress/streak: top progress bar eases; streak flame-free counter does CountUp + a subtle sparkle ring when it increments (once per day).
38. Session complete: summary card scales in (`--dur-hero`), mastery rings fill staggered, forecast bars grow from baseline (stagger 20 ms).
39. Stats forecast bars/heat cells: grow on mount, hover tooltip follows pointer with spring.

**Digest / Ingest / Accounts / Settings**
40. Day groups collapse/expand with `layout`; sticky date headers cross-fade as they stick; session cards lift on hover; decision drawer slides from `end`, alternatives list staggers.
41. Ingest run: progress bar determinate with sheen sweep; phase labels cross-fade; per-source counters CountUp; success state check-draw + summary slide-down; errors list expands with attention shake once.
42. Accounts: status dot cross-fades color; cooldown ring countdown (stroke-dashoffset linear over actual remaining time, updates every second); rate-limit event pulses the card border once; role-slot selects morph (popover).
43. Settings: theme toggle sun/moon morph (path interpolation or icon swap with rotate+fade 220 ms); direction toggle mirrors the page via View Transition (fade, not flip).

**Empty/error states**
44. Empty states: illustration-free icon + text; icon does a one-time 600 ms gentle float-in; CTA button focus-visible by default after 300 ms.
45. Error states: icon shake once; "Retry" shows spinner then success fade.

## Performance verification for motion
Playwright perf spec records a Chrome trace for: graph pan/zoom on 5k-node fixture, quiz reveal, review grade flow, route switch; asserts p95 frame ≤ 20 ms and no long task > 80 ms during the animation window (07).
