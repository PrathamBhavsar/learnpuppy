import {
  ArchiveIcon,
  BookOpenIcon,
  BrainIcon,
  CalculatorIcon,
  CameraIcon,
  CodeIcon,
  CompassIcon,
  CpuIcon,
  DatabaseIcon,
  FlaskConicalIcon,
  GlobeIcon,
  GraduationCapIcon,
  HeartPulseIcon,
  LandmarkIcon,
  LanguagesIcon,
  LeafIcon,
  LightbulbIcon,
  type LucideIcon,
  MusicIcon,
  NetworkIcon,
  PaletteIcon,
  RocketIcon,
  ScaleIcon,
  ServerIcon,
  TerminalIcon,
} from "lucide-react"

/** The 24-icon allow-list for workspace avatars (docs/web/02: fixed keys, no emoji). */
export const WORKSPACE_ICONS = {
  "book-open": BookOpenIcon,
  code: CodeIcon,
  "flask-conical": FlaskConicalIcon,
  palette: PaletteIcon,
  network: NetworkIcon,
  brain: BrainIcon,
  rocket: RocketIcon,
  globe: GlobeIcon,
  database: DatabaseIcon,
  server: ServerIcon,
  cpu: CpuIcon,
  terminal: TerminalIcon,
  music: MusicIcon,
  camera: CameraIcon,
  leaf: LeafIcon,
  landmark: LandmarkIcon,
  scale: ScaleIcon,
  "heart-pulse": HeartPulseIcon,
  calculator: CalculatorIcon,
  languages: LanguagesIcon,
  archive: ArchiveIcon,
  compass: CompassIcon,
  lightbulb: LightbulbIcon,
  "graduation-cap": GraduationCapIcon,
} as const satisfies Record<string, LucideIcon>

export type WorkspaceIconKey = keyof typeof WORKSPACE_ICONS

/** Hue names in `--cluster-N` order (N = index + 1); matches scripts/hex2oklch.ts. */
type WorkspaceHue =
  "violet" | "teal" | "orange" | "blue" | "sprout" | "rose" | "gold" | "lilac"

export function workspaceIcon(key: string): LucideIcon {
  return (WORKSPACE_ICONS as Record<string, LucideIcon>)[key] ?? BookOpenIcon
}

/** Tailwind background class for a hue name; unknown names fall back to the first hue. */
const HUE_CLASS: Record<WorkspaceHue, string> = {
  violet: "bg-cluster-1",
  teal: "bg-cluster-2",
  orange: "bg-cluster-3",
  blue: "bg-cluster-4",
  sprout: "bg-cluster-5",
  rose: "bg-cluster-6",
  gold: "bg-cluster-7",
  lilac: "bg-cluster-8",
}

export function hueClass(hue: string): string {
  return (HUE_CLASS as Record<string, string>)[hue] ?? HUE_CLASS.violet
}
