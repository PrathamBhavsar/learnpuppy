import { BookOpenIcon } from "lucide-react"

import { cn } from "@/lib/utils"
import {
  WORKSPACE_ICONS,
  type WorkspaceIconKey,
  hueClass,
} from "@/lib/workspace-visual"

/**
 * Workspace avatar: lucide icon on the hue accent (docs/web/02). Same look as the shell chip's
 * avatar; kept local because features may not import each other.
 */
export function WorkspaceMark({
  icon,
  hue,
  className,
}: {
  icon: string
  hue: string
  className?: string
}) {
  const Icon = WORKSPACE_ICONS[icon as WorkspaceIconKey] ?? BookOpenIcon
  return (
    <span
      aria-hidden
      className={cn(
        "grid size-7 shrink-0 place-items-center rounded-none text-background",
        hueClass(hue),
        className
      )}
    >
      <Icon className="size-4" />
    </span>
  )
}
