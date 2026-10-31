import type { IngestRun, IngestSourcesBody } from "@/api/pending-types"

/** "All" is the longest window the server accepts (it validates 1..=3650 days). */
const ALL_DAYS = 3650
const SINCE_OPTIONS = [
  { days: 7, label: "7 days" },
  { days: 30, label: "30 days" },
  { days: 90, label: "90 days" },
  { days: ALL_DAYS, label: "All time" },
] as const

export function sinceOptionsFor(days: number) {
  return SINCE_OPTIONS.some((o) => o.days === days)
    ? SINCE_OPTIONS
    : [...SINCE_OPTIONS, { days, label: `${days} days` }]
}

/** Client-side shape check only; the server decides whether the directory exists and is readable. */
export function validatePath(
  raw: string,
  existing: readonly string[]
): string | null {
  const path = raw.trim()
  if (path === "") return "Enter a directory path"
  if (!path.startsWith("/") && !path.startsWith("~"))
    return "Use an absolute path, such as /Users/you/code/project"
  if (existing.includes(normalizePath(path))) return "Already in the allow-list"
  return null
}

export function normalizePath(raw: string): string {
  const path = raw.trim()
  return path.length > 1 ? path.replace(/\/+$/, "") : path
}

export function sourcesEqual(
  a: IngestSourcesBody,
  b: IngestSourcesBody
): boolean {
  return (
    a.claude === b.claude &&
    a.codex === b.codex &&
    a.agy === b.agy &&
    a.git === b.git &&
    a.sinceDays === b.sinceDays &&
    a.allow.length === b.allow.length &&
    a.allow.every((p, i) => p === b.allow[i])
  )
}

const PHASE_LABEL: Record<string, string> = {
  scanning: "Scanning sessions",
  extracting: "Extracting decisions",
  auditing: "Auditing",
  cards: "Writing cards",
}

export function phaseLabel(phase: string): string {
  return PHASE_LABEL[phase] ?? phase.charAt(0).toUpperCase() + phase.slice(1)
}

export function percent(done: number, total: number): number {
  if (total <= 0) return 0
  return Math.min(100, Math.round((done / total) * 100))
}

export function formatDuration(ms: number): string {
  const total = Math.max(0, Math.round(ms / 1000))
  const m = Math.floor(total / 60)
  const s = total % 60
  return m > 0 ? `${m}m ${String(s).padStart(2, "0")}s` : `${s}s`
}

/** What to do about a failure the run reported, or null when there is nothing specific to say. */
export function actionFor(error: string): string | null {
  if (/locked|snapshot/i.test(error))
    return "Close Antigravity and run again. LearnPuppy reads a copy of its database, so it only needs the file to be unlocked."
  if (/not readable|permission|denied/i.test(error))
    return "Grant read access to that folder, or remove it from the allow-list."
  if (/cancel/i.test(error))
    return "You stopped this run. Start another to pick up where it left off."
  return null
}

function ymd(ms: number): string {
  const d = new Date(ms)
  const mm = String(d.getMonth() + 1).padStart(2, "0")
  const dd = String(d.getDate()).padStart(2, "0")
  return `${d.getFullYear()}-${mm}-${dd}`
}

/** Digest filtered to the days a run covered. */
export function digestPath(ws: string, run: IngestRun): string {
  const end = run.finishedAt ?? run.startedAt
  return `/w/${ws}/digest?from=${ymd(run.startedAt)}&to=${ymd(end)}`
}

export function isRunning(run: IngestRun): boolean {
  return run.finishedAt === null || run.finishedAt === undefined
}

export const SOURCE_LABEL: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  agy: "Antigravity",
  git: "Git",
}
