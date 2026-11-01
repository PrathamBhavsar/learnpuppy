# 11. Help and the first-run tour

Audience: a first-time learner, not a developer. Every piece of copy uses plain words and explains jargon
(workspace, ingest, due card, role slot, cooldown) the first time it appears.

## Pieces

| Piece | Where | What it does |
|---|---|---|
| `InfoTip` | `components/app/info/info.tsx` | A small (i). Hover or focus shows one sentence; click or Enter opens a dialog with `body` and `tips`. |
| `ScreenGuide` | same file | "About this screen" header button; a dialog with a summary and every part listed. Pass `open`/`onOpenChange` to open it from a menu instead. |
| `HelpMenu` | `components/app/help-menu.tsx` | Top bar button (also on Home): Replay the tour, What is everything here?, Keyboard shortcuts. |
| Tour | `components/app/tour/` | Welcome splash, then a spotlight tour. Mounted once in `routes/__root.tsx`. |

## Adding an InfoTip

1. Write the content next to the screen as an `InfoContent` (`{ title, short, body?, tips? }`) in that feature's `help.ts`
   (`features/*/help.ts`). Features cannot import each other, so content shared by several places (the app frame)
   lives in `components/app/info/shell-content.ts`.
2. Render `<InfoTip info={...} />` beside the label or control. The button never triggers its neighbour.
3. Add the same entry to the screen's `ScreenGuide` parts so touch users and the "About this screen" dialog cover it.
4. Check the claim against the code: copy must describe what the app really does.

Sidebar items are the exception: a (i) is too heavy there, so they use the collapsed-sidebar tooltip and are
explained in the Help menu guide (`SHELL_GUIDE_PARTS`).

## Adding a tour step

1. Put `data-tour="my-target"` on the real element (it must be visible and have a size).
2. Add an entry to `TOUR_STEPS` in `tour-steps.ts`: `{ id, title, body, targets: ["my-target"] }`.
   - A step that lists `targets` is skipped when none is on screen (so it adapts to Home versus inside a workspace).
   - A step without `targets` is a centred text card; `show(has)` can gate it (see `sections-overview`).
3. Steps are resolved once when the tour starts. Keep titles short and explanations to one to three sentences.

## Behaviour

- First visit (no `lp-tour` in localStorage): splash (logo and staged text via `lib/motion.ts` tokens; plain fade
  when motion is reduced), then the tour. Finishing, skipping or pressing Esc stores `lp-tour=done`.
- `?notour=1` never starts it. Replay: Settings, Help section, or the Help menu (no splash).
- Keyboard: Right/Left (mirrored in RTL) move, Enter activates, Esc skips; focus is trapped in the card and
  restored on close; each step is announced through a polite live region.
- Tests: vitest `renderApp()` marks the tour done unless called with `{ tour: true }`. Playwright configs set
  `lp-tour=done` through `storageState`; `e2e/specs/tour.spec.ts` clears it.
