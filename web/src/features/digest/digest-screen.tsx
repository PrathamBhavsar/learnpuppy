import { Link } from "@tanstack/react-router"
import {
  ListChecksIcon,
  NewspaperIcon,
  SearchXIcon,
  TriangleAlertIcon,
} from "lucide-react"
import { useMemo, useState } from "react"

import { toApiError } from "@/api/errors"
import type { NodeSource } from "@/api/pending-types"
import { EmptyState } from "@/components/app/empty-state"
import { ErrorState } from "@/components/app/error-state"
import { PageHeader } from "@/components/app/page-header"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { useUrlSearch } from "./use-url-search"
import { usePageTitle } from "@/hooks/use-page-title"
import { useWorkspaceName } from "@/hooks/use-workspace"

import { DIGEST_COPY } from "./copy"
import { useDigest } from "./api"
import { DayGroup } from "./day-group"
import { DecisionDrawer } from "./decision-drawer"
import { DigestControls } from "./digest-controls"
import { filterDays, reviewableDecisionIds } from "./filter"
import { type RangePreset, resolveRange } from "./range"

export type DigestFilters = {
  range: RangePreset
  from?: string
  to?: string
  sources: NodeSource[]
  q: string
  decision?: string
}

type Props = {
  ws: string
  filters: DigestFilters
  onFilters: (next: Partial<DigestFilters>) => void
}

export function DigestScreen({ ws, filters, onFilters }: Props) {
  const workspaceName = useWorkspaceName(ws)
  usePageTitle(DIGEST_COPY.title, workspaceName)

  const [now] = useState(() => Date.now())
  const { from, to } = resolveRange(
    filters.range,
    { from: filters.from, to: filters.to },
    now
  )
  const source = filters.sources.length ? filters.sources.join(",") : undefined
  const digest = useDigest(ws, { from, to, source })

  const [draft, setDraft] = useUrlSearch(filters.q, (q) => onFilters({ q }))

  const days = useMemo(
    () => filterDays(digest.data ?? [], filters.q),
    [digest.data, filters.q]
  )
  const reviewable = useMemo(() => reviewableDecisionIds(days), [days])
  const [collapsed, setCollapsed] = useState<ReadonlySet<string>>(new Set())
  const toggleDay = (day: string) =>
    setCollapsed((prev) => {
      const next = new Set(prev)
      if (!next.delete(day)) next.add(day)
      return next
    })

  const filtered =
    filters.q !== "" || filters.sources.length > 0 || filters.range !== "7d"
  const clearFilters = () => {
    setDraft("")
    onFilters({
      range: "7d",
      from: undefined,
      to: undefined,
      sources: [],
      q: "",
    })
  }

  return (
    <section
      data-testid="screen-digest"
      aria-busy={digest.isPending}
      className="flex min-h-full flex-col gap-5 p-6"
    >
      <PageHeader
        title={DIGEST_COPY.title}
        description={DIGEST_COPY.description}
        actions={
          reviewable.length > 0 ? (
            <Button asChild variant="outline" data-testid="digest-review-these">
              <Link
                to="/w/$ws/review"
                params={{ ws }}
                search={{ decisions: reviewable.join(",") }}
              >
                <ListChecksIcon data-icon="inline-start" />
                {DIGEST_COPY.reviewThese(reviewable.length)}
              </Link>
            </Button>
          ) : null
        }
      />

      <DigestControls
        range={filters.range}
        from={filters.from ?? from}
        to={filters.to ?? to}
        sources={filters.sources}
        query={draft}
        onRange={(range) => onFilters({ range })}
        onCustom={(custom) => onFilters({ range: "custom", ...custom })}
        onSources={(sources) => onFilters({ sources })}
        onQuery={setDraft}
      />

      {digest.isError && !digest.data ? (
        <ErrorState
          message={DIGEST_COPY.errorMessage}
          requestId={toApiError(digest.error).requestId}
          onRetry={() => digest.refetch()}
          className="flex-1"
        />
      ) : digest.isPending ? (
        <div role="status" aria-label="Loading" className="flex flex-col gap-3">
          <SkeletonBlock className="h-9 w-1/4" />
          <SkeletonBlock className="h-[68px] w-full" />
          <SkeletonBlock className="h-[68px] w-full" />
          <SkeletonBlock className="h-9 w-1/4" />
          <SkeletonBlock className="h-[68px] w-full" />
        </div>
      ) : (
        <>
          {digest.isError ? (
            <Alert data-testid="digest-stale">
              <TriangleAlertIcon />
              <AlertDescription className="flex items-center justify-between gap-3">
                {DIGEST_COPY.staleBanner}
                <Button
                  variant="outline"
                  size="sm"
                  onClick={() => void digest.refetch()}
                >
                  {DIGEST_COPY.retry}
                </Button>
              </AlertDescription>
            </Alert>
          ) : null}
          {days.length === 0 ? (
            filtered ? (
              <EmptyState
                icon={SearchXIcon}
                title={DIGEST_COPY.noMatchTitle}
                description={DIGEST_COPY.noMatchDesc}
                action={
                  <Button
                    variant="outline"
                    onClick={clearFilters}
                    data-testid="digest-clear-filters"
                  >
                    {DIGEST_COPY.clearFilters}
                  </Button>
                }
                className="flex-1"
              />
            ) : (
              <EmptyState
                icon={NewspaperIcon}
                title={DIGEST_COPY.emptyTitle}
                description={DIGEST_COPY.emptyDesc}
                action={
                  <Button asChild>
                    <Link
                      to="/w/$ws/ingest"
                      params={{ ws }}
                      data-testid="digest-go-ingest"
                    >
                      {DIGEST_COPY.goToIngest}
                    </Link>
                  </Button>
                }
                className="flex-1"
              />
            )
          ) : (
            <div data-testid="digest-days" className="flex flex-col gap-1">
              {days.map((day) => (
                <DayGroup
                  key={day.day}
                  day={day}
                  now={now}
                  collapsed={collapsed.has(day.day)}
                  onToggle={() => toggleDay(day.day)}
                  onOpenDecision={(id) => onFilters({ decision: id })}
                />
              ))}
            </div>
          )}
        </>
      )}

      <DecisionDrawer
        key={filters.decision ?? "closed"}
        ws={ws}
        decisionId={filters.decision ?? null}
        onClose={() => onFilters({ decision: undefined })}
      />
    </section>
  )
}
