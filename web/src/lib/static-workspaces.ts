// Static stand-ins until the workspace/accounts/jobs APIs land (F10, F20, F24). Shapes mirror docs/web/02-03.
import {
  BookOpenIcon,
  CodeIcon,
  FlaskConicalIcon,
  type LucideIcon,
  PaletteIcon,
} from "lucide-react"

export type WorkspaceSummary = {
  id: string
  name: string
  icon: LucideIcon
  /** 1..8, indexes the `--cluster-N` hue. */
  hue: 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8
  due: number
}

export const WORKSPACES: readonly WorkspaceSummary[] = [
  { id: "learnpuppy", name: "LearnPuppy", icon: BookOpenIcon, hue: 1, due: 3 },
  { id: "rust-systems", name: "Rust systems", icon: CodeIcon, hue: 3, due: 0 },
  {
    id: "design-notes",
    name: "Design notes",
    icon: PaletteIcon,
    hue: 2,
    due: 12,
  },
  { id: "papers", name: "ML papers", icon: FlaskConicalIcon, hue: 4, due: 0 },
]

export const DEFAULT_WORKSPACE = WORKSPACES[0]!

export function findWorkspace(
  id: string | null | undefined
): WorkspaceSummary | undefined {
  return WORKSPACES.find((w) => w.id === id)
}

export type NavBadges = Partial<Record<"learn" | "review" | "ingest", number>>
export const NAV_BADGES: NavBadges = { learn: 1, review: 3, ingest: 2 }

export const ACCOUNTS_SUMMARY: { ready: number; total: number } = {
  ready: 2,
  total: 3,
}

export type JobSummary = { id: string; label: string; detail: string }
export const RUNNING_JOBS: readonly JobSummary[] = []
