import { Link, useRouterState } from "@tanstack/react-router"
import {
  BookOpenIcon,
  ChevronsUpDownIcon,
  PlusIcon,
  SearchIcon,
} from "lucide-react"
import { motion } from "motion/react"
import { useState } from "react"

import type { WorkspaceListItem } from "@/api/pending-types"

import { InfoTip } from "@/components/app/info/info"
import { SHELL_INFO } from "@/components/app/info/shell-content"
import { Kbd } from "@/components/app/kbd"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import { Skeleton } from "@/components/ui/skeleton"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { useEndSide } from "@/hooks/use-end-side"
import { spring } from "@/lib/motion"
import { useActiveWorkspace } from "@/hooks/use-workspace"
import { pathForWorkspace } from "@/lib/nav"
import { cn } from "@/lib/utils"
import {
  WORKSPACE_ICONS,
  type WorkspaceIconKey,
  hueClass,
} from "@/lib/workspace-visual"

function WorkspaceAvatar({
  workspace,
  className,
  shared = false,
}: {
  workspace: Pick<WorkspaceListItem, "icon" | "hue">
  className?: string
  /** Animate between mounts (sidebar chip). Only one avatar may use the shared layout id at a time. */
  shared?: boolean
}) {
  const Icon =
    WORKSPACE_ICONS[workspace.icon as WorkspaceIconKey] ?? BookOpenIcon
  return (
    <motion.span
      layoutId={shared ? "workspace-avatar" : undefined}
      transition={spring.gentle}
      aria-hidden
      className={cn(
        "grid size-7 shrink-0 place-items-center text-background",
        hueClass(workspace.hue),
        className
      )}
    >
      <Icon className="size-4" />
    </motion.span>
  )
}

type WorkspaceChipProps = {
  workspace: WorkspaceListItem | undefined
  collapsed: boolean
}

export function WorkspaceChip({ workspace, collapsed }: WorkspaceChipProps) {
  const endSide = useEndSide()
  const { workspaces, isLoading } = useActiveWorkspace()
  const pathname = useRouterState({ select: (s) => s.location.pathname })
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState("")
  const matches = workspaces.filter((w) =>
    w.name.toLowerCase().includes(query.trim().toLowerCase())
  )
  const name = workspace?.name ?? (isLoading ? "Loading…" : "No workspace")

  const trigger = (
    <Button
      variant="ghost"
      data-testid="workspace-chip"
      data-tour="workspace-chip"
      aria-label={`Workspace: ${name}`}
      className={cn(
        "h-10 w-full justify-start gap-2 px-1.5",
        collapsed && "justify-center"
      )}
    >
      {workspace ? (
        <WorkspaceAvatar workspace={workspace} shared />
      ) : (
        <Skeleton className="size-7 shrink-0" />
      )}
      {collapsed ? null : (
        <>
          <span className="min-w-0 flex-1 truncate text-start text-[13px]">
            {name}
          </span>
          <ChevronsUpDownIcon
            aria-hidden
            className="size-3.5 text-muted-foreground"
          />
        </>
      )}
    </Button>
  )

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next)
        if (!next) setQuery("")
      }}
    >
      {collapsed ? (
        <Tooltip>
          <TooltipTrigger asChild>
            <PopoverTrigger asChild>{trigger}</PopoverTrigger>
          </TooltipTrigger>
          <TooltipContent side={endSide}>{name}</TooltipContent>
        </Tooltip>
      ) : (
        <PopoverTrigger asChild>{trigger}</PopoverTrigger>
      )}
      <PopoverContent
        align="start"
        className="w-72 gap-2 p-2"
        data-testid="workspace-popover"
      >
        <p className="flex items-center gap-1.5 px-1.5 pt-0.5 text-[12px] font-medium">
          Workspaces
          <InfoTip info={SHELL_INFO.workspaceChip} />
        </p>
        <div className="relative">
          <SearchIcon
            aria-hidden
            className="pointer-events-none absolute inset-y-0 start-2 my-auto size-3.5 text-muted-foreground"
          />
          <Input
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            placeholder="Find workspace"
            aria-label="Find workspace"
            data-testid="workspace-search"
            className="ps-7"
          />
        </div>
        <ul className="flex flex-col" role="list">
          {matches.map((w) => (
            <li key={w.id}>
              <Link
                to={pathForWorkspace(w.id, pathname)}
                onClick={() => setOpen(false)}
                data-testid={`workspace-option-${w.id}`}
                aria-current={w.id === workspace?.id ? "true" : undefined}
                className="flex items-center gap-2 px-1.5 py-1.5 text-[13px] hover:bg-muted aria-[current=true]:bg-muted"
              >
                <WorkspaceAvatar
                  workspace={w}
                  className="size-5 [&_svg]:size-3"
                />
                <span className="min-w-0 flex-1 truncate">{w.name}</span>
                {w.stats.dueCards > 0 ? (
                  <Badge variant="secondary" className="tabular-nums">
                    {w.stats.dueCards}
                  </Badge>
                ) : null}
                {workspaces.indexOf(w) < 9 ? (
                  <Kbd combo={`mod+${workspaces.indexOf(w) + 1}`} />
                ) : null}
              </Link>
            </li>
          ))}
          {matches.length === 0 ? (
            <li className="px-1.5 py-2 text-[13px] text-muted-foreground">
              {isLoading ? "Loading workspaces…" : "No workspace matches."}
            </li>
          ) : null}
        </ul>
        <div className="flex flex-col border-t pt-2">
          <Link
            to="/"
            search={{ new: true }}
            onClick={() => setOpen(false)}
            data-testid="workspace-new"
            className="flex items-center gap-2 px-1.5 py-1.5 text-[13px] hover:bg-muted"
          >
            <PlusIcon aria-hidden className="size-3.5" />
            New workspace
          </Link>
          <Link
            to="/"
            onClick={() => setOpen(false)}
            data-testid="workspace-manage"
            className="px-1.5 py-1.5 text-[13px] text-muted-foreground hover:bg-muted hover:text-foreground"
          >
            Manage all
          </Link>
        </div>
      </PopoverContent>
    </Popover>
  )
}
