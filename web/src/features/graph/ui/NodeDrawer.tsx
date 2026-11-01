import { Link } from "@tanstack/react-router"
import {
  ArrowRightIcon,
  ExternalLinkIcon,
  GraduationCapIcon,
  Trash2Icon,
  XIcon,
} from "lucide-react"
import { motion } from "motion/react"
import { useEffect, useRef } from "react"

import type { GraphChain } from "@/api/pending-types"
import { useGraphNode } from "@/api/queries/graph"
import { toApiError } from "@/api/errors"
import { MasteryRing } from "@/components/app/mastery-ring"
import { SkeletonBlock } from "@/components/app/skeleton-block"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { dur, ease } from "@/lib/motion"
import { cn } from "@/lib/utils"

import { copy, dateFormat } from "../copy"
import { NODE_KINDS, SOURCES, type Scene } from "../model/scene"

import { SOURCE_BG } from "./source-style"

export const DRAWER_WIDTH = 400

type Props = {
  ws: string
  scene: Scene
  /** Scene index of the node shown. */
  idx: number
  /** Which physical edge the drawer slides from ("end" of the page direction). */
  edge: "left" | "right"
  /** Move keyboard focus into the drawer (opened with Enter). */
  focusOnOpen: boolean
  onClose: () => void
  onHop: (nodeId: string) => void
  onPractice: (nodeId: string) => void
  onDelete: (nodeId: string) => void
  practicePending: boolean
}

function ChainList({
  title,
  chains,
  side,
  scene,
  onHop,
}: {
  title: string
  chains: GraphChain[]
  side: "from" | "to"
  scene: Scene
  onHop: (id: string) => void
}) {
  return (
    <section aria-label={title} className="flex flex-col gap-1.5">
      <h3 className="text-[11px] font-medium tracking-wide text-muted-foreground uppercase">
        {title}
      </h3>
      {chains.length === 0 ? (
        <p className="text-[12px] text-muted-foreground">
          {copy.drawer.noChains}
        </p>
      ) : (
        <ul className="flex flex-col">
          {chains.map((chain) => {
            const otherId = chain[side]
            const text = scene.texts[scene.index.get(otherId) ?? -1] ?? otherId
            return (
              <li key={chain.id}>
                <button
                  type="button"
                  onClick={() => onHop(otherId)}
                  data-testid={`graph-drawer-chain-${chain.id}`}
                  className="flex w-full items-start gap-2 px-1.5 py-1.5 text-start text-[12px] hover:bg-muted focus-visible:bg-muted focus-visible:outline-none"
                >
                  <ArrowRightIcon
                    aria-hidden
                    className={cn(
                      "mt-0.5 size-3 shrink-0 text-muted-foreground rtl:-scale-x-100",
                      side === "from" && "rotate-180 rtl:-rotate-180"
                    )}
                  />
                  <span dir="auto" className="line-clamp-2 min-w-0 flex-1">
                    {text}
                  </span>
                  <Badge variant="outline" className="shrink-0">
                    {copy.drawer.chainKinds[chain.kind] ?? chain.kind}
                  </Badge>
                </button>
              </li>
            )
          })}
        </ul>
      )}
    </section>
  )
}

export function NodeDrawer({
  ws,
  scene,
  idx,
  edge,
  focusOnOpen,
  onClose,
  onHop,
  onPractice,
  onDelete,
  practicePending,
}: Props) {
  const id = scene.ids[idx] as string
  const detail = useGraphNode(ws, id)
  const reduced = useReducedMotion()
  const headingRef = useRef<HTMLHeadingElement>(null)
  const source = SOURCES[scene.source[idx] as number] ?? "chat"
  const kind = NODE_KINDS[scene.kind[idx] as number] ?? "claim"
  const cards = detail.data?.cards ?? []
  const inReview = cards.filter((c) => c.state === "review").length

  useEffect(() => {
    if (focusOnOpen) headingRef.current?.focus({ preventScroll: true })
  }, [focusOnOpen, id])

  const fromX = edge === "right" ? DRAWER_WIDTH : -DRAWER_WIDTH
  return (
    <motion.aside
      aria-label={copy.drawer.label}
      data-testid="graph-drawer"
      initial={reduced ? { opacity: 0 } : { x: fromX, opacity: 0.4 }}
      animate={{ x: 0, opacity: 1 }}
      exit={reduced ? { opacity: 0 } : { x: fromX, opacity: 0 }}
      transition={{ duration: reduced ? 0.12 : dur.base, ease: ease.out }}
      style={{ width: DRAWER_WIDTH, [edge]: 0 }}
      className="absolute inset-y-0 z-30 flex max-w-[92%] flex-col border-s border-border bg-background shadow-sm"
    >
      <header className="flex items-center gap-2 border-b border-border px-4 py-2.5">
        <Badge variant="outline" data-testid="graph-drawer-kind">
          {kind}
        </Badge>
        <Badge
          variant="secondary"
          className="gap-1.5"
          data-testid="graph-drawer-source"
        >
          <span aria-hidden className={cn("size-2", SOURCE_BG[source])} />
          {copy.legend.sourceNames[source]}
        </Badge>
        <Button
          variant="ghost"
          size="icon-sm"
          className="ms-auto"
          aria-label={copy.drawer.close}
          data-testid="graph-drawer-close"
          onClick={onClose}
        >
          <XIcon />
        </Button>
      </header>
      <div className="lp-pane flex min-h-0 flex-1 flex-col gap-5 overflow-y-auto px-4 py-4">
        <h2
          ref={headingRef}
          tabIndex={-1}
          dir="auto"
          data-testid="graph-drawer-text"
          className="text-[14px] leading-[1.5] font-medium outline-none"
        >
          {scene.texts[idx]}
        </h2>

        <dl className="grid grid-cols-[auto_1fr] gap-x-4 gap-y-1.5 text-[12px]">
          <dt className="text-muted-foreground">{copy.drawer.born}</dt>
          <dd className="tabular-nums">
            {dateFormat.format(scene.createdAt[idx] as number)}
          </dd>
          <dt className="text-muted-foreground">{copy.drawer.session}</dt>
          <dd className="min-w-0 truncate" title={scene.sessionIds[idx]}>
            {scene.sessionIds[idx]}
          </dd>
        </dl>

        {detail.isPending ? (
          <div
            role="status"
            aria-label="Loading details"
            className="flex flex-col gap-3"
          >
            <SkeletonBlock className="h-16 w-full" />
            <SkeletonBlock className="h-4 w-2/3" />
            <SkeletonBlock className="h-4 w-full" />
          </div>
        ) : detail.isError ? (
          <div
            role="alert"
            className="flex flex-col items-start gap-2 text-[12px]"
          >
            <p className="text-destructive">{copy.drawer.loadFailed}</p>
            <p className="text-muted-foreground">
              {toApiError(detail.error).message}
            </p>
            <Button
              variant="outline"
              size="sm"
              onClick={() => void detail.refetch()}
            >
              Retry
            </Button>
          </div>
        ) : (
          <>
            <section
              aria-label={copy.drawer.provenance}
              className="flex flex-col gap-1.5"
            >
              <h3 className="text-[11px] font-medium tracking-wide text-muted-foreground uppercase">
                {copy.drawer.provenance}
              </h3>
              <blockquote
                dir="auto"
                className="border-s-2 border-border ps-3 text-[12px] leading-[1.5] text-muted-foreground"
              >
                {detail.data.provenanceDetail.snippet}
              </blockquote>
              {detail.data.provenanceDetail.chatId ? (
                <Button
                  asChild
                  variant="link"
                  size="xs"
                  className="self-start px-0"
                >
                  <Link
                    to="/w/$ws/learn/$chat"
                    params={{ ws, chat: detail.data.provenanceDetail.chatId }}
                    data-testid="graph-drawer-open-chat"
                  >
                    {copy.drawer.openChat}
                    <ExternalLinkIcon
                      data-icon="inline-end"
                      className="rtl:-scale-x-100"
                    />
                  </Link>
                </Button>
              ) : null}
            </section>

            <ChainList
              title={copy.drawer.incoming}
              chains={detail.data.incoming}
              side="from"
              scene={scene}
              onHop={onHop}
            />
            <ChainList
              title={copy.drawer.outgoing}
              chains={detail.data.outgoing}
              side="to"
              scene={scene}
              onHop={onHop}
            />

            <section
              aria-label={copy.drawer.cards}
              className="flex flex-col gap-1.5"
            >
              <h3 className="text-[11px] font-medium tracking-wide text-muted-foreground uppercase">
                {copy.drawer.cards}
              </h3>
              {cards.length === 0 ? (
                <p className="text-[12px] text-muted-foreground">
                  {copy.drawer.noCards}
                </p>
              ) : (
                <>
                  <div className="flex items-center gap-2 text-[12px]">
                    <MasteryRing
                      value={inReview / cards.length}
                      size={24}
                      label={`${copy.drawer.mastery}: ${inReview} of ${cards.length} cards in review`}
                    />
                    <span className="text-muted-foreground tabular-nums">
                      {inReview} / {cards.length}
                    </span>
                  </div>
                  <ul className="flex flex-col">
                    {cards.map((card) => (
                      <li
                        key={card.id}
                        className="flex items-start gap-2 px-1.5 py-1.5 text-[12px]"
                      >
                        <span
                          dir="auto"
                          className="line-clamp-2 min-w-0 flex-1"
                        >
                          {card.front}
                        </span>
                        <Badge variant="outline">
                          {copy.drawer.cardStates[card.state] ?? card.state}
                        </Badge>
                      </li>
                    ))}
                  </ul>
                </>
              )}
            </section>
          </>
        )}
      </div>
      <footer className="flex items-center gap-2 border-t border-border px-4 py-3">
        <Button
          size="sm"
          onClick={() => onPractice(id)}
          disabled={practicePending}
          data-testid="graph-drawer-practice"
        >
          <GraduationCapIcon data-icon="inline-start" />
          {copy.drawer.practice}
        </Button>
        <Button
          variant="destructive"
          size="sm"
          onClick={() => onDelete(id)}
          data-testid="graph-drawer-delete"
        >
          <Trash2Icon data-icon="inline-start" />
          {copy.drawer.delete}
        </Button>
      </footer>
    </motion.aside>
  )
}
