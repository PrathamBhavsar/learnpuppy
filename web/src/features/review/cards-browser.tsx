import { useVirtualizer } from "@tanstack/react-virtual"
import { SearchIcon } from "lucide-react"
import { useRef, useState } from "react"

import { toApiError } from "@/api/errors"
import type { CardState } from "@/api/pending-types"
import { EmptyState } from "@/components/app/empty-state"
import { ErrorState } from "@/components/app/error-state"
import { SkeletonRows } from "@/components/app/skeleton-block"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select"
import { InfoTip } from "@/components/app/info/info"
import { Switch } from "@/components/ui/switch"
import { Alert, AlertDescription } from "@/components/ui/alert"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

import { CardDetailDrawer } from "./card-detail-drawer"
import { useCards, type BrowserCard } from "./api"
import { REVIEW_COPY } from "./copy"
import { reviewInfo } from "./info"
import { formatRelativeDue, useNow } from "./time"
import { useUrlSearch } from "./use-url-search"

export type BrowserFilter = {
  state?: CardState
  q?: string
  due?: boolean
}

const STATE_OPTIONS: { value: string; label: string }[] = [
  { value: "all", label: REVIEW_COPY.stateFilterAll },
  { value: "new", label: REVIEW_COPY.stateNames.new },
  { value: "learning", label: REVIEW_COPY.stateNames.learning },
  { value: "review", label: REVIEW_COPY.stateNames.review },
  { value: "relearning", label: REVIEW_COPY.stateNames.relearning },
]

const ROW_HEIGHT = 44
const GRID = "grid-cols-[1fr_6rem_3.5rem_7rem]"

type Props = {
  ws: string
  filter: BrowserFilter
  onFilter: (next: BrowserFilter) => void
}

export function CardsBrowser({ ws, filter, onFilter }: Props) {
  const [draft, setDraft] = useUrlSearch(filter.q ?? "", (q) =>
    onFilter({ ...filter, q: q || undefined })
  )
  const { state, due } = filter

  const cards = useCards(ws, { state, q: filter.q ?? "" })
  const [openId, setOpenId] = useState<string | null>(null)
  const scrollRef = useRef<HTMLDivElement>(null)

  const now = useNow()
  const list = (cards.data ?? []).filter((c) => !due || c.due <= now)

  // eslint-disable-next-line react-hooks/incompatible-library -- the compiler is not enabled; the virtualizer is used as documented
  const virtual = useVirtualizer({
    count: list.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 6,
    initialRect: { width: 800, height: 480 },
  })

  return (
    <div data-testid="review-cards-browser" className="flex flex-col gap-3">
      <div className="flex flex-col gap-2 sm:flex-row sm:items-center">
        <div className="relative flex-1">
          <SearchIcon
            aria-hidden
            className="pointer-events-none absolute start-2.5 top-1/2 size-4 -translate-y-1/2 text-muted-foreground"
          />
          <Input
            type="search"
            value={draft}
            onChange={(e) => setDraft(e.target.value)}
            placeholder={REVIEW_COPY.browserSearch}
            data-testid="review-cards-search"
            data-screen-search
            className="ps-8"
            aria-label={REVIEW_COPY.browserSearch}
          />
        </div>
        <Select
          value={state ?? "all"}
          onValueChange={(v) =>
            onFilter({
              ...filter,
              state: v === "all" ? undefined : (v as CardState),
            })
          }
        >
          <SelectTrigger
            data-testid="review-cards-state"
            className="w-40"
            aria-label="Filter by state"
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {STATE_OPTIONS.map((s) => (
              <SelectItem
                key={s.value}
                value={s.value}
                data-testid={`review-cards-state-${s.value}`}
              >
                {s.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
        <div className="flex items-center gap-2">
          <Switch
            id="review-cards-due"
            data-testid="review-cards-due"
            checked={Boolean(due)}
            onCheckedChange={(v) =>
              onFilter({ ...filter, due: v || undefined })
            }
          />
          <Label htmlFor="review-cards-due" className="text-[13px]">
            {REVIEW_COPY.dueOnly}
          </Label>
          <InfoTip info={reviewInfo.dueOnly} />
        </div>
      </div>

      {cards.isError && cards.data ? (
        <Alert data-testid="review-cards-stale">
          <AlertDescription>
            Showing saved cards. Refresh failed.
            <Button
              variant="outline"
              size="sm"
              data-testid="review-cards-retry"
              onClick={() => void cards.refetch()}
            >
              Retry
            </Button>
          </AlertDescription>
        </Alert>
      ) : null}
      {cards.isError && !cards.data ? (
        <ErrorState
          message="Could not load cards."
          requestId={toApiError(cards.error).requestId}
          onRetry={() => cards.refetch()}
        />
      ) : cards.isPending ? (
        <div aria-busy="true" data-testid="review-cards-loading">
          <SkeletonRows rows={8} />
        </div>
      ) : list.length === 0 ? (
        <EmptyState
          icon={SearchIcon}
          title={REVIEW_COPY.noCards}
          description={
            cards.data?.length === 0 && !filter.q && !state
              ? REVIEW_COPY.noCardsAtAll
              : REVIEW_COPY.noCardsFiltered
          }
          focusAction={false}
        />
      ) : (
        <div
          role="table"
          aria-label="Cards"
          aria-rowcount={list.length + 1}
          data-testid="review-cards-table"
          className="border"
        >
          <div
            role="row"
            className={cn(
              "grid gap-2 border-b bg-card px-3 py-2 text-[11px] font-medium text-muted-foreground",
              GRID
            )}
          >
            <span role="columnheader">{REVIEW_COPY.front}</span>
            <span role="columnheader" className="flex items-center gap-1">
              {REVIEW_COPY.state}
              <InfoTip info={reviewInfo.cardState} />
            </span>
            <span role="columnheader" className="text-end">
              {REVIEW_COPY.reps}
            </span>
            <span role="columnheader" className="text-end">
              {REVIEW_COPY.due}
            </span>
          </div>
          <div
            ref={scrollRef}
            data-testid="review-cards-scroll"
            className="max-h-[60vh] overflow-auto"
          >
            <div
              role="rowgroup"
              style={{ height: virtual.getTotalSize() }}
              className="relative w-full"
            >
              {virtual.getVirtualItems().map((item) => {
                const card = list[item.index]
                if (!card) return null
                return (
                  <CardRow
                    key={card.id}
                    card={card}
                    index={item.index}
                    now={now}
                    active={openId === card.id}
                    offset={item.start}
                    onOpen={() => setOpenId(card.id)}
                  />
                )
              })}
            </div>
          </div>
        </div>
      )}

      <CardDetailDrawer
        ws={ws}
        cardId={openId}
        onClose={() => setOpenId(null)}
      />
    </div>
  )
}

function CardRow({
  card,
  index,
  now,
  active,
  offset,
  onOpen,
}: {
  card: BrowserCard
  index: number
  now: number
  active: boolean
  offset: number
  onOpen: () => void
}) {
  return (
    <div
      role="row"
      aria-rowindex={index + 2}
      data-testid="review-card-row"
      data-card-id={card.id}
      tabIndex={0}
      onClick={onOpen}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault()
          onOpen()
        }
      }}
      style={{ height: ROW_HEIGHT, transform: `translateY(${offset}px)` }}
      className={cn(
        "absolute start-0 top-0 grid w-full cursor-pointer items-center gap-2 border-b px-3 text-[13px] outline-none hover:bg-accent/50 focus-visible:ring-1 focus-visible:ring-ring",
        GRID,
        active && "bg-accent"
      )}
    >
      <span role="cell" dir="auto" className="truncate">
        {card.front}
      </span>
      <span role="cell" className="text-[12px] text-muted-foreground">
        {REVIEW_COPY.stateNames[card.state]}
      </span>
      <span role="cell" className="text-end tabular-nums">
        {card.reps}
      </span>
      <span
        role="cell"
        className="text-end text-[12px] text-muted-foreground tabular-nums"
      >
        {card.due <= now
          ? REVIEW_COPY.dueNowLabel
          : formatRelativeDue(card.due, now)}
      </span>
    </div>
  )
}
