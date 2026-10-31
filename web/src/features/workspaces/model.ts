import { z } from "zod"

import type { WorkspaceListItem } from "@/api/pending-types"
import { storage } from "@/lib/storage"
import { WORKSPACE_ICONS, type WorkspaceIconKey } from "@/lib/workspace-visual"

/** The 8 palette accents a workspace can wear (`--cluster-1..8`, docs/web/02). */
export const HUES = [
  "violet",
  "teal",
  "orange",
  "blue",
  "sprout",
  "rose",
  "gold",
  "lilac",
] as const

export const ICON_KEYS = Object.keys(WORKSPACE_ICONS) as WorkspaceIconKey[]

export const NAME_MAX = 60
export const DESCRIPTION_MAX = 280

/** Case-insensitive name collision among the given live workspaces (docs/web/02 §Registry). */
export function isNameTaken(
  name: string,
  workspaces: readonly Pick<WorkspaceListItem, "id" | "name" | "archived">[],
  exceptId?: string
): boolean {
  const wanted = name.trim().toLowerCase()
  return workspaces.some(
    (w) => !w.archived && w.id !== exceptId && w.name.toLowerCase() === wanted
  )
}

/** Form schema; `taken` is evaluated on every validation so uniqueness is live against the list. */
export function workspaceSchema(taken: (name: string) => boolean) {
  return z.object({
    name: z
      .string()
      .trim()
      .min(1, "Give the workspace a name")
      .max(NAME_MAX, `Use at most ${NAME_MAX} characters`)
      .refine((v) => !taken(v), "A workspace with this name already exists"),
    icon: z.enum(ICON_KEYS as [WorkspaceIconKey, ...WorkspaceIconKey[]]),
    hue: z.enum(HUES),
    description: z
      .string()
      .max(DESCRIPTION_MAX, `Use at most ${DESCRIPTION_MAX} characters`),
  })
}

export type WorkspaceFormValues = z.infer<ReturnType<typeof workspaceSchema>>

// ───────────────────────── last visited section, per workspace ─────────────────────────

const SECTION_KEY = (ws: string) => `lp-section:${ws}`
const SECTIONS = ["graph", "learn", "review", "digest", "ingest"] as const
export type WorkspaceSection = (typeof SECTIONS)[number]

export function rememberSection(ws: string, section: string): void {
  if ((SECTIONS as readonly string[]).includes(section))
    storage.set(SECTION_KEY(ws), section)
}

export function lastSection(ws: string): WorkspaceSection {
  const value = storage.get(SECTION_KEY(ws))
  return SECTIONS.find((s) => s === value) ?? "graph"
}

const SECTION_ROUTES = {
  graph: "/w/$ws/graph",
  learn: "/w/$ws/learn",
  review: "/w/$ws/review",
  digest: "/w/$ws/digest",
  ingest: "/w/$ws/ingest",
} as const satisfies Record<WorkspaceSection, string>

/** `<Link>` props that open workspace `ws` in the section last visited there. */
export function openProps(ws: string) {
  return { to: SECTION_ROUTES[lastSection(ws)], params: { ws } }
}

// ───────────────────────── display helpers ─────────────────────────

const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
  ["day", 86_400_000],
  ["hour", 3_600_000],
  ["minute", 60_000],
]

/** "3 days ago" / "yesterday"; "No activity yet" for null. */
export function relativeTime(at: number | null, now = Date.now()): string {
  if (at === null) return "No activity yet"
  const diff = at - now
  const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" })
  for (const [unit, ms] of UNITS) {
    if (Math.abs(diff) >= ms) return rtf.format(Math.round(diff / ms), unit)
  }
  return rtf.format(0, "minute")
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  const units = ["KB", "MB", "GB"]
  let value = bytes / 1024
  let i = 0
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024
    i += 1
  }
  return `${value.toFixed(value < 10 ? 1 : 0)} ${units[i]}`
}
