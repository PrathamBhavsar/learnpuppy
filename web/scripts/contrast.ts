// WCAG gate: every text/background token pair >= 4.5:1, graphics >= 3:1, both themes.
// Usage: bun scripts/contrast.ts   (exits 1 on any failure)
import { compositeOver, contrast } from "./color.ts"
import { DESTRUCTIVE_TINT, type TokenMap, buildTokens } from "./hex2oklch.ts"

const TEXT_PAIRS: [fg: string, bg: string][] = [
  ["foreground", "background"],
  ["card-foreground", "card"],
  ["popover-foreground", "popover"],
  ["primary-foreground", "primary"],
  ["secondary-foreground", "secondary"],
  ["muted-foreground", "muted"],
  ["muted-foreground", "background"],
  ["accent-foreground", "accent"],
  ["sidebar-foreground", "sidebar"],
  ["sidebar-primary-foreground", "sidebar-primary"],
  ["sidebar-accent-foreground", "sidebar-accent"],
  ["primary", "background"],
  ["primary", "card"],
  ["destructive", "background"],
  ["destructive", "card"],
  ["success", "background"],
  ["success", "card"],
  ["warning", "background"],
  ["warning", "card"],
  ["info", "background"],
  ["info", "card"],
]
const GRAPHIC_FG = [
  "ring",
  ...[1, 2, 3, 4, 5].map((i) => `chart-${i}`),
  ...[0, 1, 2, 3, 4].map((i) => `mastery-${i}`),
  ...["claude", "codex", "agy", "git"].map((s) => `source-${s}`),
  ...[1, 2, 3, 4, 5, 6, 7, 8].map((i) => `cluster-${i}`),
]
const GRAPHIC_BG = ["background", "card"]

export function failures(theme: string, tokens: TokenMap): string[] {
  const out: string[] = []
  const check = (fg: string, bg: string, min: number) => {
    const f = tokens[fg]
    const b = tokens[bg]
    if (!f || !b) {
      out.push(`${theme}: missing token ${!f ? fg : bg}`)
      return
    }
    const ratio = contrast(f, b)
    if (ratio < min)
      out.push(`${theme}: ${fg} on ${bg} = ${ratio.toFixed(2)} (< ${min})`)
  }
  TEXT_PAIRS.forEach(([fg, bg]) => check(fg, bg, 4.5))
  // Destructive text/badge/button sit on a tint of themselves over the page or a card.
  const destructive = tokens["destructive"]
  if (destructive) {
    for (const surface of ["background", "card", "muted"]) {
      const base = tokens[surface]
      if (!base) continue
      const ratio = contrast(
        destructive,
        compositeOver({ ...destructive, a: DESTRUCTIVE_TINT }, base)
      )
      if (ratio < 4.5)
        out.push(
          `${theme}: destructive on destructive/${DESTRUCTIVE_TINT * 100} over ${surface} = ${ratio.toFixed(2)} (< 4.5)`
        )
    }
  }
  GRAPHIC_FG.forEach((fg) => GRAPHIC_BG.forEach((bg) => check(fg, bg, 3)))
  return out
}

if (import.meta.main) {
  const { dark, light } = buildTokens()
  const errors = [...failures("dark", dark), ...failures("light", light)]
  if (errors.length > 0) {
    console.error(
      `contrast: ${errors.length} failing pair(s)\n${errors.join("\n")}`
    )
    process.exit(1)
  }
  const pairs = TEXT_PAIRS.length + 3 + GRAPHIC_FG.length * GRAPHIC_BG.length
  console.log(
    `contrast: ${pairs} pairs x 2 themes pass (text >= 4.5, graphics >= 3)`
  )
}
