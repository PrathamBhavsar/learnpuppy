import {
  CompassIcon,
  GitCommitHorizontalIcon,
  MessageSquareIcon,
  SparklesIcon,
  TerminalIcon,
  type LucideIcon,
} from "lucide-react"

import type { NodeSource } from "@/api/pending-types"
import { cn } from "@/lib/utils"

export const SOURCE_LABELS: Record<NodeSource, string> = {
  claude: "Claude",
  codex: "Codex",
  agy: "Antigravity",
  git: "Git",
  chat: "Chat",
}

const ICONS: Record<NodeSource, LucideIcon> = {
  claude: SparklesIcon,
  codex: TerminalIcon,
  agy: CompassIcon,
  git: GitCommitHorizontalIcon,
  chat: MessageSquareIcon,
}

const COLORS: Record<NodeSource, string> = {
  claude: "text-source-claude",
  codex: "text-source-codex",
  agy: "text-source-agy",
  git: "text-source-git",
  chat: "text-muted-foreground",
}

export function SourceIcon({
  source,
  className,
}: {
  source: NodeSource
  className?: string
}) {
  const Icon = ICONS[source] ?? MessageSquareIcon
  return (
    <Icon
      aria-hidden
      className={cn("size-4 shrink-0", COLORS[source], className)}
    />
  )
}
