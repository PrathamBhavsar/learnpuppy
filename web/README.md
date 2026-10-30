# LearnPuppy web

Vite + React 19 + TypeScript (strict) + Tailwind 4 + shadcn (radix, `radix-lyra`). Spec: `docs/web/`.

## Run
```bash
bun install --frozen-lockfile
bun run dev          # http://localhost:5173, /api proxied to 127.0.0.1:7878
bun run check        # tsc -b, eslint, knip, tokens:check, contrast, vitest, vite build, bundle-size
bun run e2e          # Playwright against system Chrome (starts the dev server)
LP_SHOTS=1 bun x playwright test screenshots   # regenerate docs-screenshots/
```

## Created with
`bunx --bun shadcn@latest init --preset buFyzGS --base radix --template vite --rtl --pointer --name web -y`
(run in an empty directory, then moved to `web/`, because the repo root has its own `package.json`).
Versions are pinned by `bun.lock`: react 19.3, vite 8, tailwindcss 4, radix-ui 1.8, shadcn 4.23, TanStack Router/Query, zustand, motion, vitest 5, Playwright.

## Tokens
`scripts/hex2oklch.ts` is the single source of truth (hex palette) and writes `src/styles/tokens.css` (`bun run tokens`).
`scripts/contrast.ts` fails the build if any text pair is under 4.5:1 (graphics 3:1) in either theme, including destructive text on its own tint.
`src/index.css` is the preset's file with only the color variables removed (they live in tokens.css); radius, fonts and the `@theme` map are untouched.

## Deviations from generated shadcn output
- `components/ui/sonner.tsx`: reads our `useTheme` instead of `next-themes`.
- `components/ui/slider.tsx`: thumbs take the root's `aria-label`/`aria-labelledby` (axe: unnamed slider).
- `components/ui/scroll-area.tsx`: viewport is `tabIndex={0}` (axe: scrollable region not focusable).
- Sheets must be given `side={useEndSide()}`; Radix `side` is physical, so it does not flip in RTL on its own.
- `public/theme-init.js` is a blocking same-origin script, not inline: the production CSP (`default-src 'self'`) forbids inline scripts.

## Conventions enforced by lint
No physical-direction Tailwind classes, no raw `fetch`/`EventSource` outside `src/api`, no color literals, no layout-property transitions, features may not import each other, no import cycles, jsx-a11y.
