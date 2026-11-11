import {
  BookOpenIcon,
  CalendarIcon,
  DownloadIcon,
  GraduationCapIcon,
  LibraryIcon,
  LayersIcon,
  NetworkIcon,
  NewspaperIcon,
  UserRoundIcon,
  type LucideIcon,
} from "lucide-react"

export type SectionKey =
  | "graph"
  | "learn"
  | "review"
  | "course"
  | "textbook"
  | "digest"
  | "ingest"
  | "schedule"
  | "profile"

export const SECTIONS: readonly {
  key: SectionKey
  label: string
  icon: LucideIcon
}[] = [
  { key: "graph", label: "Graph", icon: NetworkIcon },
  { key: "learn", label: "Learn", icon: GraduationCapIcon },
  { key: "review", label: "Review", icon: LayersIcon },
  { key: "course", label: "Courses", icon: BookOpenIcon },
  { key: "textbook", label: "Textbooks", icon: LibraryIcon },
  { key: "schedule", label: "Schedule", icon: CalendarIcon },
  { key: "profile", label: "Profile", icon: UserRoundIcon },
  { key: "digest", label: "Digest", icon: NewspaperIcon },
  { key: "ingest", label: "Ingest", icon: DownloadIcon },
]

export const SECTION_LABELS: Record<string, string> = {
  graph: "Graph",
  learn: "Learn",
  review: "Review",
  course: "Courses",
  textbook: "Textbooks",
  schedule: "Schedule",
  profile: "Profile",
  digest: "Digest",
  ingest: "Ingest",
  settings: "Settings",
  accounts: "Accounts",
  trash: "Trash",
  "review-all": "Review all",
  gallery: "Gallery",
}

const WORKSPACE_SECTIONS = new Set<string>([
  ...SECTIONS.map((s) => s.key),
  "settings",
])

/** The section segment of `/w/:ws/<section>/…`, or null outside a workspace route. */
function sectionOfPath(pathname: string): string | null {
  const [root, , section] = pathname.split("/").filter(Boolean)
  return root === "w" && section && WORKSPACE_SECTIONS.has(section)
    ? section
    : null
}

/** Path that opens `ws` in the section the user is already on (docs/web/02 §Routing & switching). */
export function pathForWorkspace(ws: string, currentPathname: string): string {
  return `/w/${ws}/${sectionOfPath(currentPathname) ?? "graph"}`
}
