// Canvas colors come from the CSS tokens, read once per theme change (docs/web/05 "never hard-coded").
// Canvas accepts the same CSS color strings the tokens hold, including oklch(), and alpha is applied
// with globalAlpha, so no color math or literals are needed here.
import { SOURCES } from "../model/scene"

export type CanvasTheme = {
  background: string
  foreground: string
  muted: string
  mutedForeground: string
  primary: string
  destructive: string
  success: string
  border: string
  /** One per entry of SOURCES. */
  sources: string[]
  /** --cluster-1..8 */
  clusters: string[]
  font: string
}

const SOURCE_VARS: Record<string, string> = {
  claude: "--source-claude",
  codex: "--source-codex",
  agy: "--source-agy",
  git: "--source-git",
  // Chat-born nodes have no source hue of their own; they take the single accent.
  chat: "--primary",
}

const FALLBACK = "gray"

export function readCanvasTheme(root: Element = document.documentElement): CanvasTheme {
  const style = getComputedStyle(root)
  const read = (name: string): string => style.getPropertyValue(name).trim() || FALLBACK
  const font = read("--font-mono")
  return {
    background: read("--background"),
    foreground: read("--foreground"),
    muted: read("--muted"),
    mutedForeground: read("--muted-foreground"),
    primary: read("--primary"),
    destructive: read("--destructive"),
    success: read("--success"),
    border: read("--border"),
    sources: SOURCES.map((s) => read(SOURCE_VARS[s] ?? "--primary")),
    clusters: Array.from({ length: 8 }, (_, i) => read(`--cluster-${i + 1}`)),
    font: font === FALLBACK ? "monospace" : `${font}, monospace`,
  }
}
