import { Link } from "@tanstack/react-router"
import { AnimatePresence, motion } from "motion/react"
import {
  ArrowRightIcon,
  CheckCheckIcon,
  LayersIcon,
  SkipForwardIcon,
} from "lucide-react"
import { useMemo, useState } from "react"

import { toApiError } from "@/api/errors"
import { useWorkspaces } from "@/api/queries/workspaces"
import { EmptyState } from "@/components/app/empty-state"
import { ErrorState } from "@/components/app/error-state"
import { InfoTip, ScreenGuide } from "@/components/app/info/info"
import { PageHeader } from "@/components/app/page-header"
import { SkeletonRows } from "@/components/app/skeleton-block"
import { Button } from "@/components/ui/button"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { usePageTitle } from "@/hooks/use-page-title"
import { useReducedMotion } from "@/hooks/use-reduced-motion"

import { REVIEW_COPY } from "./copy"
import { reviewAllGuide, reviewInfo } from "./info"
import { useReviewQueue } from "./api"
import { buildQueue } from "./queue"
import { ReviewSession } from "./review-session"
import { SessionSummary } from "./session-summary"
import { gradedCount, useSession } from "./use-session"

type Target = { id: string; name: string; due: number }
type WorkspaceResult = { id: string; name: string; graded: number }

const COPY = {
  title: "Review all",
  description: "Due cards across every workspace, one after another.",
  nothingDue: "Nothing due anywhere",
  nothingDueDesc: "Every workspace is caught up.",
} as const

const SESSION_LIMIT = 20

export function ReviewAllScreen() {
  usePageTitle(COPY.title)
  const workspaces = useWorkspaces(false)
  const targets = useMemo<Target[]>(
    () =>
      (workspaces.data ?? [])
        .filter((w) => !w.archived && w.stats.dueCards > 0)
        .map((w) => ({ id: w.id, name: w.name, due: w.stats.dueCards })),
    [workspaces.data]
  )

  // `run` freezes the walk list so graded cards dropping the due counts mid-run cannot shift positions.
  const [run, setRun] = useState<Target[] | null>(null)
  const [index, setIndex] = useState(0)
  const [results, setResults] = useState<WorkspaceResult[]>([])
  const [finished, setFinished] = useState(false)

  const begin = () => {
    setRun(targets)
    setIndex(0)
    setResults([])
    setFinished(false)
  }
  const reset = () => {
    setRun(null)
    setFinished(false)
  }
  const next = (result: WorkspaceResult | null) => {
    if (!run) return
    if (result) setResults((prev) => [...prev, result])
    if (index + 1 >= run.length) {
      setFinished(true)
      setRun(null)
    } else setIndex(index + 1)
  }

  const header = (
    <PageHeader
      title={COPY.title}
      description={COPY.description}
      actions={<ScreenGuide {...reviewAllGuide} />}
    />
  )
  const loading = workspaces.isPending

  return (
    <section
      data-testid="screen-review-all"
      aria-busy={loading}
      className="flex min-h-full flex-col gap-6 p-6"
    >
      {header}
      {workspaces.isError && workspaces.data ? (
        <Alert data-testid="review-all-stale">
          <AlertDescription>
            Showing saved workspaces. Refresh failed.
            <Button
              data-testid="review-all-retry"
              variant="outline"
              size="sm"
              onClick={() => void workspaces.refetch()}
            >
              Retry
            </Button>
          </AlertDescription>
        </Alert>
      ) : null}
      {workspaces.isError && !workspaces.data ? (
        <ErrorState
          message="Could not load workspaces."
          requestId={toApiError(workspaces.error).requestId}
          onRetry={() => workspaces.refetch()}
          className="flex-1"
        />
      ) : loading ? (
        <SkeletonRows rows={5} />
      ) : run ? (
        <WorkspaceRun
          key={run[index]!.id}
          target={run[index]!}
          position={index + 1}
          total={run.length}
          onDone={next}
          onAbort={() => {
            setFinished(results.length > 0)
            setRun(null)
          }}
        />
      ) : finished ? (
        <AllSummary results={results} onDone={reset} />
      ) : targets.length === 0 ? (
        <EmptyState
          icon={CheckCheckIcon}
          title={COPY.nothingDue}
          description={COPY.nothingDueDesc}
          action={
            <Button asChild variant="outline">
              <Link to="/">Back to workspaces</Link>
            </Button>
          }
          className="flex-1"
        />
      ) : (
        <Overview targets={targets} onStart={begin} />
      )}
    </section>
  )
}

function Overview({
  targets,
  onStart,
}: {
  targets: Target[]
  onStart: () => void
}) {
  const reduced = useReducedMotion()
  const total = targets.reduce((sum, t) => sum + t.due, 0)
  return (
    <motion.div
      initial={reduced ? false : { opacity: 0, y: 8 }}
      animate={{ opacity: 1, y: 0 }}
      className="flex max-w-2xl flex-col gap-5"
    >
      <div className="flex items-center justify-between gap-4 border bg-card p-5">
        <div className="flex flex-col gap-1">
          <span className="text-[13px] text-muted-foreground">
            Due across {targets.length} workspace
            {targets.length === 1 ? "" : "s"}
            <InfoTip
              info={reviewInfo.reviewAll}
              className="ms-1.5 align-middle"
            />
          </span>
          <span
            data-testid="review-all-total"
            className="font-heading text-[28px] font-medium tabular-nums"
          >
            {total}
          </span>
        </div>
        <Button size="lg" onClick={onStart} data-testid="review-all-start">
          Start all
          <ArrowRightIcon data-icon="inline-end" className="rtl:-scale-x-100" />
        </Button>
      </div>
      <ul className="flex flex-col gap-2">
        {targets.map((t) => (
          <li
            key={t.id}
            data-testid={`review-all-ws-${t.id}`}
            className="flex items-center justify-between gap-3 border bg-card p-3"
          >
            <span className="truncate font-heading text-[14px] font-medium">
              {t.name}
            </span>
            <span className="flex items-center gap-2 text-[13px] tabular-nums">
              {t.due} due
              <Button asChild variant="ghost" size="sm">
                <Link to="/w/$ws/review" params={{ ws: t.id }}>
                  Open
                </Link>
              </Button>
            </span>
          </li>
        ))}
      </ul>
    </motion.div>
  )
}

/** One workspace in the walk: interstitial, then its focus session, then its summary. */
function WorkspaceRun({
  target,
  position,
  total,
  onDone,
  onAbort,
}: {
  target: Target
  position: number
  total: number
  onDone: (result: WorkspaceResult | null) => void
  onAbort: () => void
}) {
  const reduced = useReducedMotion()
  const queue = useReviewQueue(target.id)
  const session = useSession()
  const [entered, setEntered] = useState(false)
  const isLast = position === total

  const begin = () => {
    const cards = buildQueue(queue.data?.items ?? [], {
      decisions: null,
      shuffle: false,
      limit: SESSION_LIMIT,
    })
    if (!cards.length) return onDone(null)
    session.start(cards)
    setEntered(true)
  }

  if (entered && session.state.phase !== "idle") {
    return (
      <ReviewSession
        ws={target.id}
        session={session}
        onExit={onAbort}
        heading={
          <p
            data-testid="review-all-heading"
            className="pb-3 text-[13px] font-medium"
          >
            {target.name}{" "}
            <span className="text-[12px] font-normal text-muted-foreground tabular-nums">
              ({position}/{total})
            </span>
          </p>
        }
        summary={
          <SessionSummary
            state={session.state}
            onExit={() =>
              onDone({
                id: target.id,
                name: target.name,
                graded: gradedCount(session.state),
              })
            }
            exitLabel={isLast ? "Finish" : "Next workspace"}
          />
        }
      />
    )
  }

  return (
    <AnimatePresence mode="wait">
      <motion.div
        key={target.id}
        data-testid="review-all-interstitial"
        data-workspace-id={target.id}
        initial={reduced ? false : { opacity: 0, y: 12 }}
        animate={{ opacity: 1, y: 0 }}
        exit={{ opacity: 0, y: -12 }}
        transition={{ duration: 0.22 }}
        className="mx-auto flex w-full max-w-md flex-col gap-5 border bg-card p-8 text-center"
      >
        <div className="flex flex-col gap-1">
          <span className="text-[12px] text-muted-foreground tabular-nums">
            Workspace {position} of {total}
          </span>
          <h2 className="font-heading text-[24px] font-medium">
            {target.name}
          </h2>
          <p className="text-[13px] text-muted-foreground tabular-nums">
            {target.due} card{target.due === 1 ? "" : "s"} due
          </p>
        </div>
        {queue.isError ? (
          <ErrorState
            message="Could not load this workspace's cards."
            requestId={toApiError(queue.error).requestId}
            onRetry={() => queue.refetch()}
            className="py-4"
          />
        ) : null}
        <div className="flex items-center justify-center gap-2">
          <Button
            onClick={begin}
            disabled={queue.isPending || queue.isError}
            data-testid="review-all-continue"
          >
            <LayersIcon data-icon="inline-start" />
            Review these
          </Button>
          <Button
            variant="outline"
            onClick={() => onDone(null)}
            data-testid="review-all-skip"
          >
            <SkipForwardIcon
              data-icon="inline-start"
              className="rtl:-scale-x-100"
            />
            {isLast ? "Skip and finish" : "Skip"}
          </Button>
        </div>
        <Button
          variant="ghost"
          size="sm"
          onClick={onAbort}
          data-testid="review-all-stop"
        >
          {REVIEW_COPY.exit}
        </Button>
      </motion.div>
    </AnimatePresence>
  )
}

function AllSummary({
  results,
  onDone,
}: {
  results: WorkspaceResult[]
  onDone: () => void
}) {
  const reduced = useReducedMotion()
  const total = results.reduce((sum, r) => sum + r.graded, 0)
  return (
    <motion.div
      data-testid="review-all-summary"
      initial={reduced ? false : { opacity: 0, scale: 0.96 }}
      animate={{ opacity: 1, scale: 1 }}
      transition={{ duration: 0.6, ease: [0.22, 1, 0.36, 1] }}
      className="mx-auto flex w-full max-w-2xl flex-col gap-5"
    >
      <h2 className="font-heading text-[24px] font-medium">All done</h2>
      <p className="text-[13px] text-muted-foreground tabular-nums">
        Reviewed {total} card{total === 1 ? "" : "s"} across {results.length}{" "}
        workspace{results.length === 1 ? "" : "s"}.
      </p>
      <ul className="flex flex-col gap-2">
        {results.map((r) => (
          <li
            key={r.id}
            className="flex items-center justify-between border bg-card p-3 text-[13px]"
          >
            <span className="truncate font-medium">{r.name}</span>
            <span className="text-muted-foreground tabular-nums">
              {r.graded} graded
            </span>
          </li>
        ))}
      </ul>
      <Button onClick={onDone} variant="outline" data-testid="review-all-done">
        Done
      </Button>
    </motion.div>
  )
}
