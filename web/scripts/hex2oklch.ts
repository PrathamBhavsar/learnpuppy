// Single source of truth for the LearnPuppy palette (torlink hex values).
// Usage: bun scripts/hex2oklch.ts            -> rewrites src/styles/tokens.css
//        bun scripts/hex2oklch.ts --check    -> exits 1 if tokens.css is stale
import { readFileSync, writeFileSync } from "node:fs"

import {
  type Oklch,
  compositeOver,
  contrast,
  fitContrast,
  hexToOklch,
  toCss,
} from "./color.ts"

export { hexToOklch }

export const HEX = {
  bg: "#0a0810",
  text: "#e9e4f5",
  alt: "#b9a7e6",
  bright: "#d8b4fe",
  accent: "#a78bfa",
  good: "#86d6a2",
  sprout: "#5ae87a",
  warn: "#f0c560",
  bad: "#ee7d92",
  rule: "#6b6577",
  teal: "#5fd0c5",
  orange: "#f6a55c",
  blue: "#7db8f0",
  lightPrimary: "#7c5cd6",
} as const

/** Opacity of the tinted fill behind destructive text (`bg-destructive/10`, hover `/20`). */
export const DESTRUCTIVE_TINT = 0.2

export type TokenMap = Record<string, Oklch>

const o = hexToOklch
const bg = o(HEX.bg)
const lift = (c: Oklch, dl: number): Oklch => ({ ...c, l: c.l + dl })
const alpha = (c: Oklch, a: number): Oklch => ({ ...c, a })

/** The eight workspace hues (also graph cluster hues). Order is stable: cluster-N = HUES[N-1]. */
export const HUES = [
  ["violet", HEX.accent],
  ["teal", HEX.teal],
  ["orange", HEX.orange],
  ["blue", HEX.blue],
  ["sprout", HEX.sprout],
  ["rose", HEX.bad],
  ["gold", HEX.warn],
  ["lilac", HEX.bright],
] as const

/** Dark theme: the torlink palette as-is, surfaces lifted from the page background. */
function darkTokens(): TokenMap {
  const text = o(HEX.text)
  const t: TokenMap = {
    background: bg,
    foreground: text,
    card: lift(bg, 0.04),
    "card-foreground": text,
    popover: lift(bg, 0.04),
    "popover-foreground": text,
    primary: o(HEX.accent),
    "primary-foreground": bg,
    secondary: lift(bg, 0.08),
    "secondary-foreground": text,
    muted: lift(bg, 0.08),
    "muted-foreground": o(HEX.alt),
    accent: o(HEX.bright),
    "accent-foreground": bg,
    destructive: o(HEX.bad),
    border: alpha(o(HEX.rule), 0.3),
    input: alpha(o(HEX.rule), 0.4),
    ring: o(HEX.accent),
    "chart-1": o(HEX.accent),
    "chart-2": o(HEX.bright),
    "chart-3": o(HEX.teal),
    "chart-4": o(HEX.orange),
    "chart-5": o(HEX.good),
    success: o(HEX.sprout),
    warning: o(HEX.orange),
    info: o(HEX.blue),
    sidebar: lift(bg, 0.04),
    "sidebar-foreground": text,
    "sidebar-primary": o(HEX.accent),
    "sidebar-primary-foreground": bg,
    "sidebar-accent": lift(bg, 0.08),
    "sidebar-accent-foreground": text,
    "sidebar-border": alpha(o(HEX.rule), 0.3),
    "sidebar-ring": o(HEX.accent),
    "mastery-0": o(HEX.rule),
    "mastery-1": o(HEX.accent),
    "mastery-2": o(HEX.blue),
    "mastery-3": o(HEX.teal),
    "mastery-4": o(HEX.sprout),
    "source-claude": o(HEX.orange),
    "source-codex": o(HEX.teal),
    "source-agy": o(HEX.blue),
    "source-git": o(HEX.bright),
  }
  HUES.forEach(([, hex], i) => (t[`cluster-${i + 1}`] = o(hex)))
  return t
}

/** Light theme = same hues, inverted lightness ladder, accents fitted to the light surfaces. */
function lightTokens(): TokenMap {
  const text = o(HEX.text)
  const alt = o(HEX.alt)
  const page: Oklch = { l: 0.985, c: 0.006, h: bg.h }
  const raised: Oklch = { l: 0.997, c: 0.003, h: bg.h }
  const sunken: Oklch = { l: 0.955, c: 0.012, h: bg.h }
  const ink: Oklch = { l: 0.2, c: 0.025, h: text.h }
  const primary = o(HEX.lightPrimary)
  const white: Oklch = { l: 0.99, c: 0, h: 0 }
  const text45 = (hex: string, on: Oklch) => fitContrast(o(hex), on, 4.5)
  /** Destructive text sits on its own tint (hover state): fit against that composite, not the page. */
  const fitDestructive = (surface: Oklch) => {
    const base = o(HEX.bad)
    let l = base.l
    while (l > 0.05) {
      const color = { ...base, l }
      const tint = compositeOver({ ...color, a: DESTRUCTIVE_TINT }, surface)
      if (contrast(color, tint) >= 4.5 && contrast(color, surface) >= 4.5)
        return color
      l -= 0.005
    }
    return { ...base, l }
  }
  const gfx = (hex: string) => fitContrast(o(hex), sunken, 3)
  const t: TokenMap = {
    background: page,
    foreground: ink,
    card: raised,
    "card-foreground": ink,
    popover: raised,
    "popover-foreground": ink,
    primary,
    "primary-foreground": white,
    secondary: sunken,
    "secondary-foreground": ink,
    muted: sunken,
    "muted-foreground": fitContrast({ ...alt, l: 0.5 }, sunken, 4.5),
    accent: { l: 0.92, c: 0.05, h: o(HEX.bright).h },
    "accent-foreground": ink,
    destructive: fitDestructive(sunken),
    border: alpha(o(HEX.rule), 0.3),
    input: alpha(o(HEX.rule), 0.4),
    ring: primary,
    "chart-1": gfx(HEX.accent),
    "chart-2": gfx(HEX.bright),
    "chart-3": gfx(HEX.teal),
    "chart-4": gfx(HEX.orange),
    "chart-5": gfx(HEX.good),
    success: text45(HEX.sprout, sunken),
    warning: text45(HEX.orange, sunken),
    info: text45(HEX.blue, sunken),
    sidebar: { l: 0.97, c: 0.009, h: bg.h },
    "sidebar-foreground": ink,
    "sidebar-primary": primary,
    "sidebar-primary-foreground": white,
    "sidebar-accent": sunken,
    "sidebar-accent-foreground": ink,
    "sidebar-border": alpha(o(HEX.rule), 0.3),
    "sidebar-ring": primary,
    "mastery-0": gfx(HEX.rule),
    "mastery-1": gfx(HEX.accent),
    "mastery-2": gfx(HEX.blue),
    "mastery-3": gfx(HEX.teal),
    "mastery-4": gfx(HEX.sprout),
    "source-claude": gfx(HEX.orange),
    "source-codex": gfx(HEX.teal),
    "source-agy": gfx(HEX.blue),
    "source-git": gfx(HEX.bright),
  }
  HUES.forEach(([, hex], i) => (t[`cluster-${i + 1}`] = gfx(hex)))
  return t
}

export function buildTokens(): { dark: TokenMap; light: TokenMap } {
  return { dark: darkTokens(), light: lightTokens() }
}

const EXTRA_THEME_KEYS = (keys: string[]) =>
  keys
    .filter((k) => /^(success|warning|info|mastery-|source-|cluster-)/.test(k))
    .map((k) => `  --color-${k}: var(--${k});`)
    .join("\n")

const block = (selector: string, tokens: TokenMap) =>
  `${selector} {\n${Object.entries(tokens)
    .map(([k, v]) => `  --${k}: ${toCss(v)};`)
    .join("\n")}\n}`

export function renderTokensCss(): string {
  const { dark, light } = buildTokens()
  return [
    "/* Generated by scripts/hex2oklch.ts from the torlink hex palette. Do not edit. */",
    `@theme inline {\n${EXTRA_THEME_KEYS(Object.keys(dark))}\n}`,
    block(":root, .light", light),
    block(".dark", dark),
    "",
  ].join("\n\n")
}

const TOKENS_PATH = new URL("../src/styles/tokens.css", import.meta.url)

if (import.meta.main) {
  const next = renderTokensCss()
  if (process.argv.includes("--check")) {
    if (readFileSync(TOKENS_PATH, "utf8") !== next) {
      console.error("tokens.css is stale: run `bun run tokens`")
      process.exit(1)
    }
    console.log("tokens.css up to date")
  } else {
    writeFileSync(TOKENS_PATH, next)
    console.log(
      `tokens.css written (${Object.keys(buildTokens().dark).length} tokens x 2 themes)`
    )
  }
}
