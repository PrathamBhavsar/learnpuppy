import { createFileRoute, useNavigate } from "@tanstack/react-router"
import { useCallback } from "react"

import type { NodeSource } from "@/api/pending-types"
import { DIGEST_SOURCES } from "@/features/digest/digest-controls"
import {
  DigestScreen,
  type DigestFilters,
} from "@/features/digest/digest-screen"
import { isRangePreset } from "@/features/digest/range"
import { TextbookDueLine } from "@/features/textbook/DueLine"

type DigestSearch = {
  range?: DigestFilters["range"]
  from?: string
  to?: string
  src?: string
  q?: string
  decision?: string
}

const str = (v: unknown): string | undefined =>
  typeof v === "string" && v !== "" ? v : undefined

function parseSearch(search: Record<string, unknown>): DigestSearch {
  const range = search["range"]
  return {
    range: isRangePreset(range) && range !== "7d" ? range : undefined,
    from: str(search["from"]),
    to: str(search["to"]),
    src: str(search["src"]),
    q: str(search["q"]),
    decision: str(search["decision"]),
  }
}

function parseSources(src: string | undefined): NodeSource[] {
  return (src?.split(",") ?? []).filter((s): s is NodeSource =>
    (DIGEST_SOURCES as readonly string[]).includes(s)
  )
}

function Screen() {
  const { ws } = Route.useParams()
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })

  const onFilters = useCallback(
    (next: Partial<DigestFilters>) =>
      void navigate({
        search: (prev) => {
          const merged: DigestSearch = { ...prev }
          if ("range" in next)
            merged.range = next.range === "7d" ? undefined : next.range
          if ("from" in next) merged.from = next.from
          if ("to" in next) merged.to = next.to
          if ("sources" in next)
            merged.src = next.sources?.length
              ? next.sources.join(",")
              : undefined
          if ("q" in next) merged.q = next.q || undefined
          if ("decision" in next) merged.decision = next.decision
          return merged
        },
        // Filters rewrite the URL in place; opening a decision is a history entry (Back closes it).
        replace: !("decision" in next),
      }),
    [navigate]
  )

  return (
    <DigestScreen
      ws={ws}
      filters={{
        range: search.range ?? "7d",
        from: search.from,
        to: search.to,
        sources: parseSources(search.src),
        q: search.q ?? "",
        decision: search.decision,
      }}
      onFilters={onFilters}
      summary={
        <TextbookDueLine
          ws={ws}
          onOpen={() =>
            void navigate({ to: "/w/$ws/textbook", params: { ws } })
          }
        />
      }
    />
  )
}

export const Route = createFileRoute("/_shell/w/$ws/digest")({
  component: Screen,
  validateSearch: parseSearch,
})
