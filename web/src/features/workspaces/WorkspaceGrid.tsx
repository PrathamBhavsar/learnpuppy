import type { WorkspaceListItem } from "@/api/pending-types"

import { Card } from "@/components/ui/card"
import { Skeleton } from "@/components/ui/skeleton"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { useReducedMotion } from "@/hooks/use-reduced-motion"

import type { CardActions } from "./WorkspaceCard"
import { WorkspaceCard } from "./WorkspaceCard"

/** Loading grid: skeletons sized to a real card, same column layout as the filled grid. */
export function WorkspaceGridSkeleton({
  count = 6,
  testId = "workspace-grid-skeleton",
}: {
  count?: number
  testId?: string
}) {
  return (
    <ul
      role="status"
      aria-busy
      aria-label="Loading workspaces"
      data-testid={testId}
      className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3"
    >
      {Array.from({ length: count }, (_, i) => (
        <li key={i} aria-hidden>
          <Card className="flex min-h-40 flex-col gap-3 p-4">
            <div className="flex items-center gap-3">
              <Skeleton className="size-7" />
              <SkeletonBlock className="h-4 w-32" />
            </div>
            <SkeletonBlock className="h-4 w-full" />
            <SkeletonBlock className="h-4 w-2/3" />
            <div className="mt-auto flex w-full items-center justify-between pt-1">
              <SkeletonBlock className="h-3 w-40" />
              <SkeletonBlock className="h-3 w-16" />
            </div>
          </Card>
        </li>
      ))}
    </ul>
  )
}

type GridProps = {
  /** `aria-label` for the card list ("Workspaces", "Archived workspaces"). */
  label: string
  testId?: string
  workspaces: readonly WorkspaceListItem[]
  actions: CardActions
  /** Just-created workspace: its card flies in and pulses once. */
  freshId?: string | null
  /** Card whose action (e.g. repair) is pending. */
  busyId?: string | null
}

/**
 * The workspace grid (docs/web/06 §Home): cards enter with a stagger; a fresh card skips the delay
 * and springs in; motion-reduced users get opacity-only.
 */
export function WorkspaceGrid({
  label,
  testId,
  workspaces,
  actions,
  freshId,
  busyId,
}: GridProps) {
  const reduced = useReducedMotion()
  return (
    <ul
      role="list"
      aria-label={label}
      data-testid={testId ?? "workspace-grid"}
      className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3"
    >
      {workspaces.map((w, index) => (
        <WorkspaceCard
          key={w.id}
          workspace={w}
          index={index}
          count={workspaces.length}
          actions={actions}
          fresh={w.id === freshId}
          busy={busyId !== undefined && busyId !== null && busyId === w.id}
          reduced={reduced}
        />
      ))}
    </ul>
  )
}
