# 04b — Preset facts (V00, verified 2026-10-31)

Command run in scratch: `bunx --bun shadcn@latest init --preset buFyzGS --base radix --template vite --rtl --pointer --name probe -y` (flags `--rtl`, `--pointer`, `-n`, `-y` all exist in shadcn 4.23).

| Item | Result |
|---|---|
| style | `radix-lyra` (square, `rounded-none` buttons) |
| iconLibrary | **lucide** (`lucide-react`), NOT hugeicons → decision D15 resolves to lucide; Hugeicons dropped |
| rtl | `true` in components.json; RTL provider + fonts need manual wiring (docs: ui.shadcn.com/docs/rtl/vite) |
| `--pointer` | adds `cursor: pointer` global rule in `index.css` for buttons |
| font | `@fontsource-variable/geist-mono`, `--font-mono: 'Geist Mono Variable'`, `--font-heading: var(--font-mono)` |
| radius | `--radius: 0.625rem` (components themselves use `rounded-none`) |
| menuAccent | `subtle` (was `bold` for the old lab) |
| baseColor | neutral; css at `src/index.css`; `rsc:false` |
| stack | react 19.3, vite 8, tailwind 4, radix-ui 1.8, tw-animate-css, shadcn 4.23 |
| scratch size | 251 MB node_modules (acceptable; disk free 7.1 GB at check time) |

Implications: use `lucide-react` icons everywhere; keep `components/ui/*` as generated; tokens override colors only.
