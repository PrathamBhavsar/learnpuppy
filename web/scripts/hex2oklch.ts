// Converts the torlink hex palette to OKLCH and rewrites the palette block in app/globals.css.
// Usage: bun scripts/hex2oklch.ts
import { readFileSync, writeFileSync } from "node:fs"

type Lch = { l: number; c: number; h: number }

function hexToRgb(hex: string): [number, number, number] {
  const n = parseInt(hex.replace("#", ""), 16)
  return [(n >> 16) & 255, (n >> 8) & 255, n & 255]
}
const lin = (v: number) => {
  const s = v / 255
  return s <= 0.04045 ? s / 12.92 : ((s + 0.055) / 1.055) ** 2.4
}
export function hexToOklch(hex: string): Lch {
  const [r, g, b] = hexToRgb(hex).map(lin) as [number, number, number]
  const l = Math.cbrt(0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b)
  const m = Math.cbrt(0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b)
  const s = Math.cbrt(0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b)
  const L = 0.2104542553 * l + 0.793617785 * m - 0.0040720468 * s
  const A = 1.9779984951 * l - 2.428592205 * m + 0.4505937099 * s
  const B = 0.0259040371 * l + 0.7827717662 * m - 0.808675766 * s
  const C = Math.hypot(A, B)
  const H = C < 1e-4 ? 0 : ((Math.atan2(B, A) * 180) / Math.PI + 360) % 360
  return { l: L, c: C, h: H }
}
const f = (n: number, d = 3) => String(+n.toFixed(d))
const css = ({ l, c, h }: Lch, alpha?: number) =>
  `oklch(${f(l)} ${f(c)} ${f(h, 2)}${alpha !== undefined ? ` / ${alpha * 100}%` : ""})`
const lift = (hex: string, dl: number) => {
  const x = hexToOklch(hex)
  return { ...x, l: x.l + dl }
}


/** Lower OKLCH lightness (keeping chroma and hue) until the color reaches `min` WCAG contrast on white. */
export function darkenForWhite(hex: string, min = 3.2): string {
  const base = hexToOklch(hex);
  const toLin = (L: number, C: number, H: number) => {
    const a = C * Math.cos((H * Math.PI) / 180);
    const b = C * Math.sin((H * Math.PI) / 180);
    const l = (L + 0.3963377774 * a + 0.2158037573 * b) ** 3;
    const m = (L - 0.1055613458 * a - 0.0638541728 * b) ** 3;
    const s = (L - 0.0894841775 * a - 1.291485548 * b) ** 3;
    const r = 4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s;
    const g = -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s;
    const bl = -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s;
    return [r, g, bl].map((v) => Math.min(1, Math.max(0, v)));
  };
  const contrast = (L: number) => {
    const [r, g, b] = toLin(L, base.c, base.h);
    return 1.05 / (0.2126 * r + 0.7152 * g + 0.0722 * b + 0.05);
  };
  let L = base.l;
  while (L > 0.2 && contrast(L) < min) L -= 0.005;
  const n = (v: number, d = 6) => Number(v.toFixed(d));
  return `oklch(${n(L)} ${n(base.c)} ${n(base.h, 3)})`;
}

// torlink palette (src/ui/theme.ts)
const P = {
  bg: "#0a0810", text: "#e9e4f5", alt: "#b9a7e6", bright: "#d8b4fe", accent: "#a78bfa",
  good: "#86d6a2", warn: "#f0c560", bad: "#ee7d92", rule: "#6b6577",
  teal: "#5fd0c5", orange: "#f6a55c", blue: "#7db8f0", lightPrimary: "#7c5cd6",
}

const dark: Record<string, string> = {
  background: css(hexToOklch(P.bg)),
  foreground: css(hexToOklch(P.text)),
  card: css(lift(P.bg, 0.04)),
  "card-foreground": css(hexToOklch(P.text)),
  popover: css(lift(P.bg, 0.04)),
  "popover-foreground": css(hexToOklch(P.text)),
  primary: css(hexToOklch(P.accent)),
  "primary-foreground": css(hexToOklch(P.bg)),
  secondary: css(lift(P.bg, 0.08)),
  "secondary-foreground": css(hexToOklch(P.text)),
  muted: css(lift(P.bg, 0.08)),
  "muted-foreground": css(hexToOklch(P.alt)),
  accent: css(hexToOklch(P.bright)),
  "accent-foreground": css(hexToOklch(P.bg)),
  destructive: css(hexToOklch(P.bad)),
  border: css(hexToOklch(P.rule), 0.3),
  input: css(hexToOklch(P.rule), 0.4),
  ring: css(hexToOklch(P.accent)),
  "chart-1": css(hexToOklch(P.accent)),
  "chart-2": css(hexToOklch(P.bright)),
  "chart-3": css(hexToOklch(P.teal)),
  "chart-4": css(hexToOklch(P.orange)),
  "chart-5": css(hexToOklch(P.good)),
  success: css(hexToOklch(P.good)),
  warning: css(hexToOklch(P.warn)),
  info: css(hexToOklch(P.blue)),
  "mastery-novice": css(hexToOklch(P.bad)),
  "mastery-learning": css(hexToOklch(P.warn)),
  "mastery-mastered": css(hexToOklch(P.good)),
  sidebar: css(lift(P.bg, 0.04)),
  "sidebar-foreground": css(hexToOklch(P.text)),
  "sidebar-primary": css(hexToOklch(P.accent)),
  "sidebar-primary-foreground": css(hexToOklch(P.bg)),
  "sidebar-accent": css(lift(P.bg, 0.08)),
  "sidebar-accent-foreground": css(hexToOklch(P.text)),
  "sidebar-border": css(hexToOklch(P.rule), 0.3),
  "sidebar-ring": css(hexToOklch(P.accent)),
}
// Light mode keeps preset neutrals; only primary/ring swapped + semantic extras.
const light: Record<string, string> = {
  primary: css(hexToOklch(P.lightPrimary)),
  "primary-foreground": "oklch(0.985 0 0)",
  ring: css(hexToOklch(P.lightPrimary)),
  // Darker light-mode variants: >=3:1 on white.
  "chart-1": darkenForWhite(P.accent),
  "chart-2": darkenForWhite(P.bright),
  "chart-3": darkenForWhite(P.teal),
  "chart-4": darkenForWhite(P.orange),
  "chart-5": darkenForWhite(P.good),
  success: darkenForWhite(P.good),
  warning: darkenForWhite(P.warn),
  info: darkenForWhite(P.blue),
  "mastery-novice": darkenForWhite(P.bad),
  "mastery-learning": darkenForWhite(P.warn),
  "mastery-mastered": darkenForWhite(P.good),
}
const block = (sel: string, o: Record<string, string>) =>
  `${sel} {\n${Object.entries(o).map(([k, v]) => `  --${k}: ${v};`).join("\n")}\n}`

const theme = `@theme inline {
  --color-success: var(--success);
  --color-warning: var(--warning);
  --color-info: var(--info);
  --color-mastery-novice: var(--mastery-novice);
  --color-mastery-learning: var(--mastery-learning);
  --color-mastery-mastered: var(--mastery-mastered);
}`

const START = "/* palette:start (generated by scripts/hex2oklch.ts) */"
const END = "/* palette:end */"
const out = `${START}\n${theme}\n\n${block(":root", light)}\n\n${block(".dark", dark)}\n${END}`

const path = new URL("../app/globals.css", import.meta.url).pathname
let src = readFileSync(path, "utf8")
const re = new RegExp(`${START.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}[\\s\\S]*?${END.replace(/[.*+?^${}()|[\]\\]/g, "\\$&")}`)
src = re.test(src) ? src.replace(re, out) : `${src.trimEnd()}\n\n${out}\n`
writeFileSync(path, src)
console.log(`palette written: ${Object.keys(dark).length} dark vars, ${Object.keys(light).length} light vars`)
