import { createFileRoute, useNavigate } from "@tanstack/react-router"
import { useCallback } from "react"

import type { CardState } from "@/api/pending-types"
import type { BrowserFilter } from "@/features/review/cards-browser"
import { ReviewScreen } from "@/features/review/review-screen"

type ReviewSearch = {
  tab?: "cards"
  state?: CardState
  q?: string
  due?: true
  decisions?: string
}

const STATES: readonly string[] = ["new", "learning", "review", "relearning"]

function parseSearch(search: Record<string, unknown>): ReviewSearch {
  const { tab, state, q, due, decisions } = search
  return {
    tab: tab === "cards" ? "cards" : undefined,
    state:
      typeof state === "string" && STATES.includes(state)
        ? (state as CardState)
        : undefined,
    q: typeof q === "string" && q ? q : undefined,
    due: due === true || due === "true" || due === "1" ? true : undefined,
    decisions:
      typeof decisions === "string" && decisions ? decisions : undefined,
  }
}

function Screen() {
  const { ws } = Route.useParams()
  const search = Route.useSearch()
  const navigate = useNavigate({ from: Route.fullPath })

  const onTabChange = useCallback(
    (next: "review" | "cards") =>
      void navigate({
        search: (prev) => ({
          ...prev,
          tab: next === "cards" ? "cards" : undefined,
        }),
      }),
    [navigate]
  )
  const onBrowserFilter = useCallback(
    (next: BrowserFilter) =>
      void navigate({
        search: (prev) => ({
          ...prev,
          tab: "cards",
          state: next.state,
          q: next.q,
          due: next.due ? true : undefined,
        }),
        replace: true,
      }),
    [navigate]
  )

  return (
    <ReviewScreen
      ws={ws}
      tab={search.tab ?? "review"}
      onTabChange={onTabChange}
      decisionsFilter={search.decisions}
      browserFilter={{ state: search.state, q: search.q, due: search.due }}
      onBrowserFilter={onBrowserFilter}
    />
  )
}

export const Route = createFileRoute("/_shell/w/$ws/review")({
  component: Screen,
  validateSearch: parseSearch,
})
