import { InfoTip } from "@/components/app/info/info"
import { Link } from "@tanstack/react-router"
import {
  ArchiveIcon,
  ArchiveRestoreIcon,
  DownloadIcon,
  FolderSearchIcon,
  MoreHorizontalIcon,
  PencilIcon,
  SlidersHorizontalIcon,
  Trash2Icon,
  TriangleAlertIcon,
  WrenchIcon,
} from "lucide-react"
import { motion } from "motion/react"

import type { WorkspaceListItem } from "@/api/pending-types"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { Spinner } from "@/components/ui/spinner"
import { dur, ease, spring, staggerDelay } from "@/lib/motion"
import { cn } from "@/lib/utils"

import { openProps, relativeTime } from "./model"
import { WorkspaceMark } from "./WorkspaceAvatar"

import { HOME_INFO } from "./help"

export type CardActions = {
  onEdit: (workspace: WorkspaceListItem) => void
  onDelete: (workspace: WorkspaceListItem) => void
  onExport: (workspace: WorkspaceListItem) => void
  onSetArchived: (workspace: WorkspaceListItem, archived: boolean) => void
  onRepair: (workspace: WorkspaceListItem) => void
  onReveal: (workspace: WorkspaceListItem) => void
}

type Props = {
  workspace: WorkspaceListItem
  index: number
  count: number
  actions: CardActions
  /** Just created: flies in and pulses once. */
  fresh?: boolean
  busy?: boolean
  /** Motion-reduced: opacity-only enter, no scale or lift movement. */
  reduced?: boolean
}

function Stat({ value, label }: { value: number; label: string }) {
  return (
    <bdi className="tabular-nums">
      <span className="text-foreground">{value.toLocaleString()}</span> {label}
    </bdi>
  )
}

function CardMenu({
  workspace,
  actions,
}: {
  workspace: WorkspaceListItem
  actions: CardActions
}) {
  const id = workspace.id
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon-sm"
          aria-label={`Actions for ${workspace.name}`}
          data-testid={`workspace-menu-${id}`}
          className="absolute end-2 top-2 z-10"
        >
          <MoreHorizontalIcon />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent
        align="end"
        data-testid={`workspace-menu-list-${id}`}
      >
        {workspace.damaged ? (
          <>
            <DropdownMenuItem
              onSelect={() => actions.onRepair(workspace)}
              data-testid={`workspace-repair-${id}`}
            >
              <WrenchIcon /> Repair
            </DropdownMenuItem>
            <DropdownMenuItem
              onSelect={() => actions.onReveal(workspace)}
              data-testid={`workspace-reveal-${id}`}
            >
              <FolderSearchIcon /> Reveal path
            </DropdownMenuItem>
          </>
        ) : (
          <>
            {workspace.archived ? null : (
              <DropdownMenuItem
                onSelect={() => actions.onEdit(workspace)}
                data-testid={`workspace-rename-${id}`}
              >
                <PencilIcon /> Rename &amp; edit
              </DropdownMenuItem>
            )}
            {workspace.archived ? null : (
              <DropdownMenuItem asChild>
                <Link
                  to="/w/$ws/ingest"
                  params={{ ws: id }}
                  data-testid={`workspace-sources-${id}`}
                >
                  <SlidersHorizontalIcon /> Edit sources
                </Link>
              </DropdownMenuItem>
            )}
            <DropdownMenuItem
              onSelect={() => actions.onExport(workspace)}
              data-testid={`workspace-export-${id}`}
            >
              <DownloadIcon /> Export
            </DropdownMenuItem>
            <DropdownMenuItem
              onSelect={() =>
                actions.onSetArchived(workspace, !workspace.archived)
              }
              data-testid={`workspace-archive-${id}`}
            >
              {workspace.archived ? <ArchiveRestoreIcon /> : <ArchiveIcon />}
              {workspace.archived ? "Restore" : "Archive"}
            </DropdownMenuItem>
          </>
        )}
        <DropdownMenuSeparator />
        <DropdownMenuItem
          variant="destructive"
          onSelect={() => actions.onDelete(workspace)}
          data-testid={`workspace-delete-${id}`}
        >
          <Trash2Icon /> Delete…
        </DropdownMenuItem>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}

export function WorkspaceCard({
  workspace,
  index,
  count,
  actions,
  fresh = false,
  busy = false,
  reduced = false,
}: Props) {
  const { stats, damaged, archived } = workspace
  const id = workspace.id
  const body = (
    <>
      <div className="flex items-center gap-3 pe-8">
        <WorkspaceMark icon={workspace.icon} hue={workspace.hue} />
        <h2 className="min-w-0 flex-1 truncate font-heading text-[14px] font-medium">
          {workspace.name}
        </h2>
      </div>
      {damaged ? (
        <p className="flex items-start gap-1.5 text-[12px] text-destructive">
          <TriangleAlertIcon aria-hidden className="mt-0.5 size-3.5 shrink-0" />
          workspace.json is unreadable. Repair rewrites minimal metadata from
          the folder name.
        </p>
      ) : (
        <p className="line-clamp-2 min-h-[2.4em] text-[12px] leading-[1.2] text-muted-foreground">
          {workspace.description || "No description."}
        </p>
      )}
      {damaged ? null : (
        <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] text-muted-foreground">
          <Stat value={stats.nodes} label="nodes" />
          <Stat value={stats.chains} label="chains" />
          <Stat value={stats.chats} label="chats" />
          <InfoTip
            info={HOME_INFO.numbers}
            className="pointer-events-auto relative z-10"
            testId={`info-numbers-${id}`}
          />
        </div>
      )}
      <div className="mt-auto flex items-center justify-between gap-2 pt-1 text-[11px] text-muted-foreground">
        <bdi data-testid={`workspace-activity-${id}`}>
          {damaged
            ? "Needs repair"
            : relativeTime(stats.lastActivityAt ?? null)}
        </bdi>
        <span className="flex items-center gap-1.5">
          {archived ? <Badge variant="outline">Archived</Badge> : null}
          {damaged ? <Badge variant="destructive">Damaged</Badge> : null}
          {stats.dueCards > 0 && !archived ? (
            <Badge
              variant="secondary"
              className="tabular-nums"
              data-testid={`workspace-due-${id}`}
            >
              {stats.dueCards} due
            </Badge>
          ) : null}
        </span>
      </div>
    </>
  )

  const surface = cn(
    "relative flex h-full min-h-40 flex-col gap-3 border bg-card p-4 text-card-foreground",
    "transition-[border-color,translate] duration-[var(--dur-fast)] ease-[var(--ease-out)]",
    "hover:-translate-y-0.5 hover:border-ring motion-reduce:transition-colors motion-reduce:hover:translate-y-0",
    "has-[a:focus-visible]:ring-1 has-[a:focus-visible]:ring-ring",
    (archived || damaged) && "opacity-90",
    damaged && "border-destructive/50"
  )

  return (
    <motion.li
      layout="position"
      initial={
        reduced ? { opacity: 0 } : { opacity: 0, y: 8, scale: fresh ? 0.94 : 1 }
      }
      animate={
        reduced
          ? { opacity: 1 }
          : fresh
            ? { opacity: 1, y: 0, scale: [0.94, 1.03, 1] }
            : { opacity: 1, y: 0, scale: 1 }
      }
      exit={{
        opacity: 0,
        scale: reduced ? 1 : 0.94,
        transition: { duration: dur.fast * 0.7, ease: ease.in },
      }}
      transition={
        reduced
          ? {
              duration: dur.base,
              delay: fresh ? 0 : staggerDelay(index, count),
            }
          : {
              duration: dur.base,
              ease: ease.out,
              delay: fresh ? 0 : staggerDelay(index, count),
              layout: spring.gentle,
            }
      }
      data-testid={`workspace-item-${id}`}
      data-state={damaged ? "damaged" : archived ? "archived" : "live"}
    >
      <div className={surface}>
        {damaged ? (
          body
        ) : (
          <Link
            {...openProps(id)}
            data-testid={`workspace-card-${id}`}
            aria-label={`Open ${workspace.name}`}
            className="absolute inset-0 z-0 outline-none"
          />
        )}
        {damaged ? null : (
          <div className="pointer-events-none flex h-full flex-col gap-3">
            {body}
          </div>
        )}
        <CardMenu workspace={workspace} actions={actions} />
        {damaged ? (
          <div className="flex gap-2">
            <Button
              size="sm"
              variant="outline"
              onClick={() => actions.onRepair(workspace)}
              disabled={busy}
              data-testid={`workspace-card-repair-${id}`}
            >
              {busy ? <Spinner data-icon="inline-start" /> : <WrenchIcon />}
              Repair
            </Button>
            <Button
              size="sm"
              variant="ghost"
              onClick={() => actions.onReveal(workspace)}
              data-testid={`workspace-card-reveal-${id}`}
            >
              <FolderSearchIcon /> Reveal path
            </Button>
            <InfoTip info={HOME_INFO.repair} testId={`info-repair-${id}`} />
          </div>
        ) : null}
      </div>
    </motion.li>
  )
}
