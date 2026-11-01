import { Link } from "@tanstack/react-router"
import {
  ArrowRightIcon,
  CheckCheckIcon,
  LayersIcon,
  NetworkIcon,
} from "lucide-react"
import { motion } from "motion/react"
import { useCallback, useMemo, useState } from "react"

import { toApiError } from "@/api/errors"
import { EmptyState } from "@/components/app/empty-state"
import { ErrorState } from "@/components/app/error-state"
import { InfoTip, ScreenGuide } from "@/components/app/info/info"
import { PageHeader } from "@/components/app/page-header"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { Button } from "@/components/ui/button"
import { Tabs, TabsList, TabsTrigger, TabsContent } from "@/components/ui/tabs"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { usePageTitle } from "@/hooks/use-page-title"
import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { useWorkspaceName } from "@/hooks/use-workspace"

import { type BrowserFilter, CardsBrowser } from "./cards-browser"
import { useCards, useReviewQueue, useReviewStats } from "./api"
import { REVIEW_COPY } from "./copy"
import { Forecast } from "./forecast"
import { reviewGuide, reviewInfo } from "./info"
import { buildQueue, parseDecisionIds } from "./queue"
import { ReviewControls, type ReviewControlsValue } from "./review-controls"
import { ReviewSession } from "./review-session"
import { SessionSummary } from "./session-summary"
import { StatsStrip } from "./stats-strip"
import { formatRelativeDue, useNow } from "./time"
import { useSession } from "./use-session"

type ReviewScreenProps = {
  ws: string
  tab: "review" | "cards"
  onTabChange: (tab: "review" | "cards") => void
  /** Comma-separated decision ids ("Review these N decisions" from the digest). */
  decisionsFilter?: string
  browserFilter: BrowserFilter
  onBrowserFilter: (next: BrowserFilter) => void
}

export function ReviewScreen({
  ws,
  tab,
  onTabChange,
  decisionsFilter,
  browserFilter,
  onBrowserFilter,
}: ReviewScreenProps) {
  const workspaceName = useWorkspaceName(ws)
  usePageTitle(REVIEW_COPY.title, workspaceName)
  const reduced = useReducedMotion()
  const now = useNow()

  const stats = useReviewStats(ws)
  const queue = useReviewQueue(ws)
  const session = useSession()
  const [controls, setControls] = useState<ReviewControlsValue>({
    limit: 20,
    shuffle: false,
  })

  const decisions = useMemo(
    () => parseDecisionIds(decisionsFilter),
    [decisionsFilter]
  )
  const dueCards = queue.data?.items
  const sessionCards = useMemo(
    () =>
      buildQueue(dueCards ?? [], {
        decisions,
        shuffle: controls.shuffle,
        limit: controls.limit,
      }),
    [dueCards, decisions, controls.shuffle, controls.limit]
  )

  const startSession = useCallback(() => {
    if (sessionCards.length) session.start(sessionCards)
  }, [session, sessionCards])

  const dueNow = stats.data?.dueNow ?? 0
  const loading = stats.isPending || queue.isPending
  const failed =
    (stats.isError && !stats.data) || (queue.isError && !queue.data)
  const error = queue.error ?? stats.error
  const active = session.state.phase !== "idle"
  const caughtUp = dueNow === 0 || sessionCards.length === 0

  return (
    <Tabs
      value={tab}
      onValueChange={(v) => onTabChange(v === "cards" ? "cards" : "review")}
      asChild
    >
      <section
        data-testid="screen-review"
        aria-busy={loading && tab === "review"}
        className="flex min-h-full flex-col gap-6 p-6"
      >
        <PageHeader
          title={REVIEW_COPY.title}
          description={REVIEW_COPY.description}
          actions={
            <>
              <ScreenGuide {...reviewGuide} />
              <TabsList data-testid="review-tabs">
                <TabsTrigger
                  value="review"
                  data-testid="review-tab-review"
                  className="text-foreground"
                >
                  Review
                </TabsTrigger>
                <TabsTrigger
                  value="cards"
                  data-testid="review-tab-cards"
                  className="text-foreground"
                >
                  {REVIEW_COPY.browserTitle}
                </TabsTrigger>
              </TabsList>
            </>
          }
        />

        <TabsContent value="cards">
          <CardsBrowser
            ws={ws}
            filter={browserFilter}
            onFilter={onBrowserFilter}
          />
        </TabsContent>
        <TabsContent value="review">
          {active ? (
            <ReviewSession
              ws={ws}
              session={session}
              onExit={session.idle}
              summary={
                <SessionSummary
                  state={session.state}
                  onAgain={session.idle}
                  onExit={session.idle}
                />
              }
            />
          ) : failed ? (
            <ErrorState
              message="Could not load your review queue."
              requestId={error ? toApiError(error).requestId : undefined}
              onRetry={() => Promise.all([queue.refetch(), stats.refetch()])}
              className="flex-1"
            />
          ) : loading ? (
            <div
              role="status"
              aria-label="Loading"
              className="flex flex-col gap-4"
            >
              <SkeletonBlock className="h-[68px] w-full" />
              <SkeletonBlock className="h-[120px] w-full" />
              <SkeletonBlock className="h-[132px] w-full" />
            </div>
          ) : (
            <motion.div
              initial={reduced ? false : { opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
              className="flex flex-col gap-5"
            >
              {stats.isError || queue.isError ? (
                <Alert data-testid="review-stale">
                  <AlertDescription>
                    Showing saved review data. Refresh failed.
                    <Button
                      data-testid="review-retry"
                      size="sm"
                      variant="outline"
                      onClick={() =>
                        void Promise.all([queue.refetch(), stats.refetch()])
                      }
                    >
                      Retry
                    </Button>
                  </AlertDescription>
                </Alert>
              ) : null}
              {stats.data ? <StatsStrip stats={stats.data} /> : null}
              {stats.data ? (
                <Forecast
                  data={stats.data.forecast}
                  now={stats.data.forecastStart ?? now}
                />
              ) : null}
              {caughtUp ? (
                <CaughtUp
                  ws={ws}
                  filtered={Boolean(decisions)}
                  onBrowse={() => onTabChange("cards")}
                />
              ) : (
                <div
                  data-testid="review-start-panel"
                  className="flex flex-col gap-5 border bg-card p-6"
                >
                  <div className="flex items-center justify-between gap-4">
                    <div className="flex flex-col gap-1">
                      <h2 className="flex items-center gap-2 font-heading text-[20px] font-medium">
                        {REVIEW_COPY.startReview(sessionCards.length)}
                        <InfoTip info={reviewInfo.review} />
                      </h2>
                      <p
                        data-testid="review-start-note"
                        className="flex items-center gap-1.5 text-[13px] text-muted-foreground"
                      >
                        {decisions
                          ? REVIEW_COPY.filteredNote(decisions.size)
                          : REVIEW_COPY.dueNote}
                        <InfoTip info={reviewInfo.spaced} />
                      </p>
                    </div>
                    <Button
                      size="lg"
                      onClick={startSession}
                      data-testid="review-start"
                    >
                      {REVIEW_COPY.startReview(sessionCards.length)}
                      <ArrowRightIcon
                        data-icon="inline-end"
                        className="rtl:-scale-x-100"
                      />
                    </Button>
                  </div>
                  <ReviewControls value={controls} onChange={setControls} />
                </div>
              )}
            </motion.div>
          )}
        </TabsContent>
      </section>
    </Tabs>
  )
}

/** "All caught up" with the next due time, read from the card list. */
function CaughtUp({
  ws,
  filtered,
  onBrowse,
}: {
  ws: string
  filtered: boolean
  onBrowse: () => void
}) {
  const cards = useCards(ws)
  const now = useNow()
  const next = useMemo(() => {
    const future = (cards.data ?? []).map((c) => c.due).filter((d) => d > now)
    return future.length ? Math.min(...future) : null
  }, [cards.data, now])
  const description = filtered
    ? REVIEW_COPY.noneForDecisions
    : next === null
      ? REVIEW_COPY.allCaughtUpDesc
      : `${REVIEW_COPY.allCaughtUpDesc} ${REVIEW_COPY.nextDue(formatRelativeDue(next))}.`
  return (
    <EmptyState
      icon={CheckCheckIcon}
      title={REVIEW_COPY.allCaughtUp}
      description={description}
      action={
        <div className="flex items-center gap-2">
          <Button
            variant="outline"
            onClick={onBrowse}
            data-testid="review-browse-cards"
          >
            <LayersIcon data-icon="inline-start" />
            {REVIEW_COPY.browseCards}
          </Button>
          <Button asChild variant="ghost">
            <Link
              to="/w/$ws/graph"
              params={{ ws }}
              data-testid="review-practice-graph"
            >
              <NetworkIcon data-icon="inline-start" />
              {REVIEW_COPY.practiceFromGraph}
            </Link>
          </Button>
        </div>
      }
    />
  )
}
