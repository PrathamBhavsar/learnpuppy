import { Link } from "@tanstack/react-router"
import { motion } from "motion/react"
import {
  ExternalLinkIcon,
  GitCommitHorizontalIcon,
  RefreshCwIcon,
} from "lucide-react"

import { ApiError, toApiError } from "@/api/errors"
import type { RegenerateResult } from "@/api/pending-types"
import { type InfoContent, InfoTip } from "@/components/app/info/info"
import { ErrorState } from "@/components/app/error-state"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { Alert, AlertDescription, AlertTitle } from "@/components/ui/alert"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"
import { Spinner } from "@/components/ui/spinner"
import { useEndSide } from "@/hooks/use-end-side"
import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { staggerDelay } from "@/lib/motion"
import { toastError, toastSuccess } from "@/lib/toast"
import { useState } from "react"

import { DIGEST_COPY } from "./copy"
import { digestInfo } from "./info"
import { useDecision, useRegenerateCards } from "./api"

type Props = {
  ws: string
  decisionId: string | null
  onClose: () => void
}

const dateFormat = new Intl.DateTimeFormat(undefined, { dateStyle: "medium" })

type Problem = { path: string; message: string }

/** Validation problems from a failed or partially rejected regeneration, in one list. */
function problemsFrom(
  error: unknown,
  result: RegenerateResult | null
): Problem[] {
  const fromError =
    error instanceof ApiError ? error.fields.map((f) => ({ ...f })) : []
  const fromResult = (result?.rejected ?? []).map((r) => ({
    path: r.front,
    message: r.reason,
  }))
  return [...fromError, ...fromResult]
}

export function DecisionDrawer({ ws, decisionId, onClose }: Props) {
  const side = useEndSide()
  const reduced = useReducedMotion()
  const detail = useDecision(ws, decisionId)
  const regenerate = useRegenerateCards(ws)
  const [lastResult, setLastResult] = useState<RegenerateResult | null>(null)
  const [error, setError] = useState<unknown>(null)
  const decision = detail.data
  const problems = problemsFrom(error, lastResult)

  const runRegenerate = () => {
    if (!decisionId || regenerate.isPending) return
    setError(null)
    setLastResult(null)
    regenerate.mutate(decisionId, {
      onSuccess: (result) => {
        setLastResult(result)
        if (!result.rejected?.length)
          toastSuccess(
            DIGEST_COPY.regenerated(result.saved ?? result.cardIds.length)
          )
      },
      onError: (err) => {
        setError(err)
        // Field-level validation lands in the drawer; anything else is a toast.
        if (!(err instanceof ApiError && err.fields.length))
          toastError(err, "Could not regenerate cards")
      },
    })
  }

  const frontOf = (id: string) =>
    decision?.cards.find((c) => c.id === id)?.front

  return (
    <Sheet
      open={decisionId !== null}
      onOpenChange={(open) => {
        if (!open) {
          setError(null)
          setLastResult(null)
          onClose()
        }
      }}
    >
      <SheetContent
        side={side}
        data-testid="digest-drawer"
        className="flex w-full flex-col gap-0 overflow-y-auto sm:max-w-lg"
      >
        <SheetHeader>
          <SheetTitle dir="auto" data-testid="digest-drawer-title">
            {decision?.title ?? "Decision"}
          </SheetTitle>
          <SheetDescription className="sr-only">
            Decision details, alternatives, commits and generated cards
          </SheetDescription>
        </SheetHeader>

        {detail.isError ? (
          <ErrorState
            message={DIGEST_COPY.drawerError}
            requestId={toApiError(detail.error).requestId}
            onRetry={() => detail.refetch()}
          />
        ) : !decision ? (
          <div
            role="status"
            aria-label="Loading"
            className="flex flex-col gap-3 px-4"
          >
            <SkeletonBlock className="h-6 w-1/3" />
            <SkeletonBlock className="h-16 w-full" />
            <SkeletonBlock className="h-24 w-full" />
          </div>
        ) : (
          <div className="flex flex-col gap-5 px-4 pb-6">
            <div className="flex flex-col gap-2">
              <div className="flex items-center gap-1.5">
                <Badge
                  variant={
                    decision.basis === "stated" ? "default" : "secondary"
                  }
                  data-testid="digest-drawer-basis"
                >
                  {DIGEST_COPY.basis[decision.basis].label}
                </Badge>
                <InfoTip info={digestInfo.basis} />
                <InfoTip info={digestInfo.decision} className="ms-auto" />
              </div>
              <p className="text-[12px] text-muted-foreground">
                {DIGEST_COPY.basis[decision.basis].explanation}
              </p>
            </div>

            <Field label={DIGEST_COPY.chosen}>
              <p
                dir="auto"
                className="text-[14px]"
                data-testid="digest-drawer-chosen"
              >
                {decision.chosen}
              </p>
            </Field>
            <Field label={DIGEST_COPY.why}>
              <p dir="auto" className="text-[13px] text-muted-foreground">
                {decision.why}
              </p>
            </Field>
            <Field label={DIGEST_COPY.risk}>
              <p dir="auto" className="text-[13px] text-muted-foreground">
                {decision.risk}
              </p>
            </Field>

            <Field
              label={DIGEST_COPY.alternatives}
              info={digestInfo.alternatives}
            >
              {decision.alternatives.length === 0 ? (
                <p className="text-[13px] text-muted-foreground">
                  None recorded.
                </p>
              ) : (
                <ul
                  data-testid="digest-drawer-alternatives"
                  className="flex flex-col gap-2"
                >
                  {decision.alternatives.map((alt, i) => (
                    <motion.li
                      key={alt.option}
                      initial={reduced ? false : { opacity: 0, y: 6 }}
                      animate={{ opacity: 1, y: 0 }}
                      transition={{
                        duration: 0.14,
                        delay: staggerDelay(i, decision.alternatives.length),
                      }}
                      className="flex flex-col gap-0.5 border bg-card p-2.5"
                    >
                      <span dir="auto" className="text-[13px]">
                        {alt.option}
                      </span>
                      <span
                        dir="auto"
                        className="text-[12px] text-muted-foreground"
                      >
                        {DIGEST_COPY.rejectedBecause} {alt.rejectedBecause}
                      </span>
                    </motion.li>
                  ))}
                </ul>
              )}
            </Field>

            {decision.files.length > 0 ? (
              <Field label={DIGEST_COPY.files}>
                <ul dir="ltr" className="flex flex-col gap-0.5 text-[12px]">
                  {decision.files.map((file) => (
                    <li key={file} className="truncate text-muted-foreground">
                      {file}
                    </li>
                  ))}
                </ul>
              </Field>
            ) : null}

            <Field label={DIGEST_COPY.commits} info={digestInfo.commits}>
              {decision.commitsError ? (
                <p role="status" className="text-[13px] text-warning">
                  {decision.commitsError}
                </p>
              ) : null}
              {decision.commits.length === 0 ? (
                <p className="text-[13px] text-muted-foreground">
                  {DIGEST_COPY.noCommits}
                </p>
              ) : (
                <ul
                  data-testid="digest-drawer-commits"
                  className="flex flex-col divide-y border"
                >
                  {decision.commits.map((commit) => (
                    <li
                      key={commit.sha}
                      className="flex items-center gap-2 px-2.5 py-2 text-[13px]"
                    >
                      <GitCommitHorizontalIcon
                        aria-hidden
                        className="size-4 shrink-0 text-source-git"
                      />
                      <code
                        dir="ltr"
                        className="text-[12px] text-muted-foreground"
                      >
                        {commit.sha.slice(0, 7)}
                      </code>
                      <span dir="auto" className="min-w-0 flex-1 truncate">
                        {commit.subject}
                      </span>
                      {commit.at !== undefined ? (
                        <span className="shrink-0 text-[11px] text-muted-foreground tabular-nums">
                          {dateFormat.format(commit.at)}
                        </span>
                      ) : null}
                    </li>
                  ))}
                </ul>
              )}
            </Field>

            <Field label={DIGEST_COPY.generatedCards} info={digestInfo.cards}>
              {decision.cardIds.length === 0 ? (
                <p className="text-[13px] text-muted-foreground">
                  {DIGEST_COPY.noCards}
                </p>
              ) : (
                <ul
                  data-testid="digest-drawer-cards"
                  className="flex flex-col gap-1.5"
                >
                  {decision.cardIds.map((id) => {
                    const front = frontOf(id)
                    return (
                      <li
                        key={id}
                        className="flex items-center gap-2 text-[13px]"
                      >
                        <span dir="auto" className="min-w-0 flex-1 truncate">
                          {front ?? id}
                        </span>
                        <Button asChild variant="ghost" size="sm">
                          <Link
                            to="/w/$ws/review"
                            params={{ ws }}
                            search={{ tab: "cards", q: front ?? id }}
                            data-testid={`digest-drawer-card-${id}`}
                          >
                            <ExternalLinkIcon
                              data-icon="inline-start"
                              className="rtl:-scale-x-100"
                            />
                            {DIGEST_COPY.openInBrowser}
                          </Link>
                        </Button>
                      </li>
                    )
                  })}
                </ul>
              )}
              <div className="flex items-center gap-1.5 self-start">
                <Button
                  variant="outline"
                  onClick={runRegenerate}
                  disabled={regenerate.isPending}
                  data-testid="digest-regenerate"
                >
                  {regenerate.isPending ? (
                    <Spinner data-icon="inline-start" />
                  ) : (
                    <RefreshCwIcon data-icon="inline-start" />
                  )}
                  {regenerate.isPending
                    ? DIGEST_COPY.regenerating
                    : DIGEST_COPY.regenerate}
                </Button>
                <InfoTip info={digestInfo.regenerate} />
              </div>
              {problems.length > 0 ? (
                <Alert
                  variant="destructive"
                  data-testid="digest-regenerate-errors"
                >
                  <AlertTitle>{DIGEST_COPY.validationTitle}</AlertTitle>
                  <AlertDescription>
                    <ul className="flex flex-col gap-1">
                      {problems.map((p) => (
                        <li key={`${p.path}-${p.message}`}>
                          <span dir="auto" className="font-medium">
                            {p.path}
                          </span>
                          : {p.message}
                        </li>
                      ))}
                    </ul>
                  </AlertDescription>
                </Alert>
              ) : null}
            </Field>
          </div>
        )}
      </SheetContent>
    </Sheet>
  )
}

function Field({
  label,
  info,
  children,
}: {
  label: string
  info?: InfoContent
  children: React.ReactNode
}) {
  return (
    <section className="flex flex-col gap-1.5">
      <h3 className="flex items-center gap-1 text-[12px] text-muted-foreground">
        {label}
        {info ? <InfoTip info={info} /> : null}
      </h3>
      {children}
    </section>
  )
}
