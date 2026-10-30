import {
  DownloadIcon,
  GraduationCapIcon,
  LayersIcon,
  NetworkIcon,
  NewspaperIcon,
  type LucideIcon,
} from "lucide-react"

export type SectionKey = "graph" | "learn" | "review" | "digest" | "ingest"

export const SECTIONS: readonly {
  key: SectionKey
  label: string
  icon: LucideIcon
}[] = [
  { key: "graph", label: "Graph", icon: NetworkIcon },
  { key: "learn", label: "Learn", icon: GraduationCapIcon },
  { key: "review", label: "Review", icon: LayersIcon },
  { key: "digest", label: "Digest", icon: NewspaperIcon },
  { key: "ingest", label: "Ingest", icon: DownloadIcon },
]

export const SECTION_LABELS: Record<string, string> = {
  graph: "Graph",
  learn: "Learn",
  review: "Review",
  digest: "Digest",
  ingest: "Ingest",
  settings: "Settings",
  accounts: "Accounts",
  trash: "Trash",
  "review-all": "Review all",
  gallery: "Gallery",
}
