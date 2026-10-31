import { storage } from "@/lib/storage"

export type Theme = "light" | "dark" | "system"
export type ResolvedTheme = "light" | "dark"
export type Direction = "ltr" | "rtl"
export type Density = "comfortable" | "compact"

export const THEME_KEY = "lp-theme"
export const DIR_KEY = "lp-dir"
export const REDUCE_MOTION_KEY = "lp-reduce-motion"
export const DENSITY_KEY = "lp-density"
export const THEMES: readonly Theme[] = ["light", "dark", "system"]

export function isTheme(value: unknown): value is Theme {
  return value === "light" || value === "dark" || value === "system"
}

function prefersDark(): boolean {
  return (
    typeof window !== "undefined" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches
  )
}

export function resolveTheme(theme: Theme): ResolvedTheme {
  if (theme === "system") return prefersDark() ? "dark" : "light"
  return theme
}

/** `?dir=rtl|ltr` wins over the stored preference (used by e2e and screenshots). */
export function readDirection(
  search: string = window.location.search
): Direction {
  const query = new URLSearchParams(search).get("dir")
  if (query === "rtl" || query === "ltr") return query
  return storage.get(DIR_KEY) === "rtl" ? "rtl" : "ltr"
}

export function readTheme(): Theme {
  const stored = storage.get(THEME_KEY)
  return isTheme(stored) ? stored : "system"
}

export function applyResolvedTheme(resolved: ResolvedTheme): void {
  const root = document.documentElement
  root.classList.remove("light", "dark")
  root.classList.add(resolved)
  root.style.colorScheme = resolved
}

export function applyDirection(dir: Direction): void {
  document.documentElement.dir = dir
}

export function readDensity(): Density {
  return storage.get(DENSITY_KEY) === "compact" ? "compact" : "comfortable"
}

/** `html[data-density=compact]` tightens the spacing scale (styles/density.css). */
export function applyDensity(density: Density): void {
  if (density === "compact")
    document.documentElement.dataset.density = "compact"
  else delete document.documentElement.dataset.density
}

export function applyReduceMotion(on: boolean): void {
  if (on) document.documentElement.dataset.reduceMotion = "true"
  else delete document.documentElement.dataset.reduceMotion
}

function motionReduced(): boolean {
  return (
    document.documentElement.dataset.reduceMotion === "true" ||
    window.matchMedia("(prefers-reduced-motion: reduce)").matches
  )
}

/** Run a DOM-mutating update inside a View Transition (200 ms cross-fade); instant fallback. */
export function withViewTransition(update: () => void): void {
  if (typeof document.startViewTransition !== "function" || motionReduced()) {
    update()
    return
  }
  document.startViewTransition(update)
}
