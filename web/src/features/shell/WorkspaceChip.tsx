import { Link } from "@tanstack/react-router"
import { ChevronsUpDownIcon, PlusIcon, SearchIcon } from "lucide-react"
import { motion } from "motion/react"
import { useState } from "react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { useEndSide } from "@/hooks/use-end-side"
import { spring } from "@/lib/motion"
import { cn } from "@/lib/utils"

import { WORKSPACES, type WorkspaceSummary } from "@/lib/static-workspaces"

const HUE_BG = {
  1: "bg-cluster-1",
  2: "bg-cluster-2",
  3: "bg-cluster-3",
  4: "bg-cluster-4",
  5: "bg-cluster-5",
  6: "bg-cluster-6",
  7: "bg-cluster-7",
  8: "bg-cluster-8",
} as const

export function WorkspaceAvatar({
  workspace,
  className,
}: {
  workspace: WorkspaceSummary
  className?: string
}) {
  const Icon = workspace.icon
  return (
    <motion.span
      layoutId="workspace-avatar"
      transition={spring.gentle}
      aria-hidden
      className={cn(
        "grid size-7 shrink-0 place-items-center text-background",
        HUE_BG[workspace.hue],
        className
      )}
    >
      <Icon className="size-4" />
    </motion.span>
  )
}

type WorkspaceChipProps = { workspace: WorkspaceSummary; collapsed: boolean }

export function WorkspaceChip({ workspace, collapsed }: WorkspaceChipProps) {
  const endSide = useEndSide()
  const [open, setOpen] = useState(false)
  const [query, setQuery] = useState("")
  const matches = WORKSPACES.filter((w) =>
    w.name.toLowerCase().includes(query.trim().toLowerCase())
  )

  const trigger = (
    <Button
      variant="ghost"
      data-testid="workspace-chip"
      aria-label={`Workspace: ${workspace.name}`}
      className={cn(
        "h-10 w-full justify-start gap-2 px-1.5",
        collapsed && "justify-center"
      )}
    >
      <WorkspaceAvatar workspace={workspace} />
      {collapsed ? null : (
        <>
          <span className="min-w-0 flex-1 truncate text-start text-[13px]">
            {workspace.name}
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
    <Popover open={open} onOpenChange={setOpen}>
      {collapsed ? (
        <Tooltip>
          <TooltipTrigger asChild>
            <PopoverTrigger asChild>{trigger}</PopoverTrigger>
          </TooltipTrigger>
          <TooltipContent side={endSide}>{workspace.name}</TooltipContent>
        </Tooltip>
      ) : (
        <PopoverTrigger asChild>{trigger}</PopoverTrigger>
      )}
      <PopoverContent
        align="start"
        className="w-64 gap-2 p-2"
        data-testid="workspace-popover"
      >
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
          {matches.map((w, i) => (
            <li key={w.id}>
              <Link
                to="/w/$ws/graph"
                params={{ ws: w.id }}
                onClick={() => setOpen(false)}
                data-testid={`workspace-option-${w.id}`}
                aria-current={w.id === workspace.id ? "true" : undefined}
                className="flex items-center gap-2 px-1.5 py-1.5 text-[13px] hover:bg-muted aria-[current=true]:bg-muted"
              >
                <WorkspaceAvatar
                  workspace={w}
                  className="size-5 [&_svg]:size-3"
                />
                <span className="min-w-0 flex-1 truncate">{w.name}</span>
                {w.due > 0 ? (
                  <Badge variant="secondary" className="tabular-nums">
                    {w.due}
                  </Badge>
                ) : null}
                <span className="text-[11px] text-muted-foreground tabular-nums">
                  ⌘{i + 1}
                </span>
              </Link>
            </li>
          ))}
          {matches.length === 0 ? (
            <li className="px-1.5 py-2 text-[13px] text-muted-foreground">
              No workspace matches.
            </li>
          ) : null}
        </ul>
        <div className="flex flex-col border-t pt-2">
          <Link
            to="/"
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
