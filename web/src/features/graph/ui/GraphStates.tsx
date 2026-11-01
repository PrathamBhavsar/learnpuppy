import { FilterXIcon, NetworkIcon, ZapIcon } from "lucide-react"

import { toApiError } from "@/api/errors"
import { EmptyState } from "@/components/app/empty-state"
import { ErrorState } from "@/components/app/error-state"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"

import { copy } from "../copy"

/** Skeleton sized like the populated screen: toolbar strip over a canvas field. */
export function GraphLoading() {
  return (
    <div
      role="status"
      aria-label="Loading graph"
      aria-busy
      data-testid="graph-loading"
      className="relative size-full"
    >
      <div className="absolute inset-x-3 top-3 flex gap-2">
        <SkeletonBlock className="h-8 w-60" />
        <SkeletonBlock className="h-8 w-72" />
      </div>
      <SkeletonBlock className="absolute inset-x-[18%] top-[22%] bottom-[18%] opacity-40" />
    </div>
  )
}

export function GraphEmpty({
  onLearn,
  onIngest,
}: {
  onLearn: () => void
  onIngest: () => void
}) {
  return (
    <EmptyState
      icon={NetworkIcon}
      title={copy.empty.title}
      description={copy.empty.description}
      className="absolute inset-0"
      action={
        <div className="flex items-center gap-2">
          <Button onClick={onLearn} data-testid="graph-empty-learn">
            {copy.empty.learn}
          </Button>
          <Button
            variant="outline"
            onClick={onIngest}
            data-testid="graph-empty-ingest"
          >
            {copy.empty.ingest}
          </Button>
        </div>
      }
    />
  )
}

export function GraphError({
  error,
  onRetry,
}: {
  error: unknown
  onRetry: () => Promise<unknown>
}) {
  const api = toApiError(error)
  return (
    <ErrorState
      title={copy.error.title}
      message={api.message || copy.error.message}
      requestId={api.requestId}
      onRetry={onRetry}
      className="absolute inset-0"
    />
  )
}

/** Shown over the soft cluster silhouettes while the server computes positions. */
export function LayoutPending() {
  return (
    <div
      role="status"
      data-testid="graph-layout-pending"
      className="pointer-events-none absolute inset-0 z-10 flex flex-col items-center justify-center gap-2 text-center"
    >
      <Spinner className="size-5 text-muted-foreground" />
      <p className="text-[14px] font-medium">{copy.pending.title}</p>
      <p className="max-w-xs text-[12px] text-muted-foreground">
        {copy.pending.description}
      </p>
    </div>
  )
}

export function NoVisible({ onClear }: { onClear: () => void }) {
  return (
    <div className="pointer-events-none absolute inset-0 z-10 flex items-center justify-center">
      <div
        role="status"
        data-testid="graph-no-visible"
        className="pointer-events-auto flex max-w-xs flex-col items-center gap-2 border border-border bg-background px-6 py-5 text-center"
      >
        <FilterXIcon
          aria-hidden
          className="size-6 text-muted-foreground"
          strokeWidth={1.5}
        />
        <p className="text-[14px] font-medium">{copy.noMatchesFilter.title}</p>
        <p className="text-[12px] text-muted-foreground">
          {copy.noMatchesFilter.description}
        </p>
        <Button variant="outline" size="sm" onClick={onClear}>
          {copy.clearFilters}
        </Button>
      </div>
    </div>
  )
}

/** Chip shown above 10k nodes; performance mode is on by default and can be switched off. */
export function PerformanceChip({
  nodes,
  on,
  onToggle,
}: {
  nodes: number
  on: boolean
  onToggle: () => void
}) {
  return (
    <div
      data-testid="graph-performance-chip"
      className="pointer-events-auto flex items-center gap-2 border border-warning/50 bg-background px-2 py-1 text-[11px]"
    >
      <ZapIcon aria-hidden className="size-3 text-warning" />
      <span>
        {on ? copy.performanceOn(nodes) : copy.performanceLarge(nodes)}
      </span>
      <Button
        variant="ghost"
        size="xs"
        onClick={onToggle}
        data-testid="graph-performance-toggle"
      >
        {on ? copy.performanceOff : copy.performanceOnAction}
      </Button>
    </div>
  )
}
