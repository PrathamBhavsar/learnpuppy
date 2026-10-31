import { ClockIcon } from "lucide-react"

import { toApiError } from "@/api/errors"
import { ErrorState } from "@/components/app/error-state"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { Badge } from "@/components/ui/badge"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetHeader,
  SheetTitle,
} from "@/components/ui/sheet"
import { useEndSide } from "@/hooks/use-end-side"

import { REVIEW_COPY } from "./copy"
import { useCardDetail } from "./api"
import { formatRelativeDue, useNow } from "./time"

type Props = {
  ws: string
  cardId: string | null
  onClose: () => void
}

const dateTime = new Intl.DateTimeFormat(undefined, {
  dateStyle: "medium",
  timeStyle: "short",
})

export function CardDetailDrawer({ ws, cardId, onClose }: Props) {
  const side = useEndSide()
  const detail = useCardDetail(ws, cardId ?? undefined)
  const card = detail.data
  const now = useNow()

  return (
    <Sheet open={cardId !== null} onOpenChange={(open) => !open && onClose()}>
      <SheetContent
        side={side}
        data-testid="review-card-drawer"
        className="flex w-full flex-col gap-0 overflow-y-auto sm:max-w-md"
      >
        <SheetHeader>
          <SheetTitle>Card</SheetTitle>
          <SheetDescription className="sr-only">
            Card details and review history
          </SheetDescription>
        </SheetHeader>

        {detail.isError ? (
          <ErrorState
            message="Could not load this card."
            requestId={toApiError(detail.error).requestId}
            onRetry={() => detail.refetch()}
          />
        ) : !card ? (
          <div
            role="status"
            aria-label="Loading"
            className="flex flex-col gap-3 px-4"
          >
            <SkeletonBlock className="h-5 w-1/3" />
            <SkeletonBlock className="h-16 w-full" />
            <SkeletonBlock className="h-24 w-full" />
          </div>
        ) : (
          <div className="flex flex-col gap-4 px-4 pb-6">
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="secondary" data-testid="review-card-drawer-state">
                {REVIEW_COPY.stateNames[card.state]}
              </Badge>
              <Badge variant="outline" className="gap-1 tabular-nums">
                <ClockIcon className="size-3" />
                {card.due <= now
                  ? REVIEW_COPY.dueNowLabel
                  : formatRelativeDue(card.due, now)}
              </Badge>
            </div>

            <p
              dir="auto"
              data-testid="review-card-drawer-front"
              className="text-[14px] leading-relaxed"
            >
              {card.front}
            </p>

            {card.options?.length ? (
              <ul className="flex flex-col gap-1.5" aria-label="Choices">
                {card.options.map((opt) => (
                  <li
                    key={opt.key}
                    data-correct={opt.key === card.correctKey ? "" : undefined}
                    className="flex items-start gap-2 text-[13px] data-[correct]:text-success"
                  >
                    <span className="flex size-5 shrink-0 items-center justify-center border text-[11px]">
                      {opt.key}
                    </span>
                    <span dir="auto">{opt.text}</span>
                  </li>
                ))}
              </ul>
            ) : null}

            <p dir="auto" className="text-[13px] text-muted-foreground">
              {card.rationale}
            </p>

            <section className="flex flex-col gap-2">
              <h3 className="text-[12px] text-muted-foreground">
                {REVIEW_COPY.history}
              </h3>
              {card.history.length === 0 ? (
                <p
                  data-testid="review-card-history-empty"
                  className="text-[13px] text-muted-foreground"
                >
                  {REVIEW_COPY.notReviewed}
                </p>
              ) : (
                <ol
                  data-testid="review-card-history"
                  className="flex flex-col divide-y border"
                >
                  {[...card.history].reverse().map((log) => (
                    <li
                      key={`${log.ts}-${log.deviceId}`}
                      className="flex items-center justify-between px-3 py-2 text-[13px]"
                    >
                      <span className="text-muted-foreground tabular-nums">
                        {dateTime.format(log.ts)}
                      </span>
                      <span>{REVIEW_COPY.ratingLabels[log.rating - 1]}</span>
                    </li>
                  ))}
                </ol>
              )}
            </section>

            <Collapsible data-testid="review-card-drawer-details">
              <CollapsibleTrigger
                className="flex w-full items-center justify-between px-2 py-1.5 text-[13px] font-medium outline-none hover:bg-accent focus-visible:ring-1 focus-visible:ring-ring"
                data-testid="review-card-drawer-details-trigger"
              >
                {REVIEW_COPY.detail}
              </CollapsibleTrigger>
              <CollapsibleContent className="grid grid-cols-2 gap-2 pt-2">
                <Stat label={REVIEW_COPY.reps} value={card.reps} />
                <Stat label={REVIEW_COPY.lapses} value={card.lapses} />
                <Stat
                  label={REVIEW_COPY.stability}
                  value={card.stability.toFixed(1)}
                />
                <Stat
                  label={REVIEW_COPY.difficulty}
                  value={card.difficulty.toFixed(1)}
                />
              </CollapsibleContent>
            </Collapsible>
          </div>
        )}
      </SheetContent>
    </Sheet>
  )
}

function Stat({ label, value }: { label: string; value: number | string }) {
  return (
    <div className="flex flex-col gap-0.5 border bg-card p-2">
      <span className="text-[11px] text-muted-foreground">{label}</span>
      <span className="font-heading text-[15px] font-medium tabular-nums">
        {value}
      </span>
    </div>
  )
}
