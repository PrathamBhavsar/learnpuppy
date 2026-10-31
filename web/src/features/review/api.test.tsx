import { act, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, http } from "msw"
import { beforeEach, expect, it } from "vitest"

import { state } from "@/test/msw/db"
import { cardAnswer, cardPage } from "@/test/msw/fixtures/review-contract"
import { server } from "@/test/msw/server"
import { renderApp, resetUi } from "@/test/render"
import { useUi } from "@/stores/ui"

const WS = "ws_rustasync"
const PATH = `/w/${WS}/review`
const API = `*/api/v1/workspaces/${WS}/review`

beforeEach(() => {
  resetUi()
  useUi.setState({ reduceMotion: true })
})

it("reads every page of the real card-list envelope", async () => {
  const cards = state.db.cards.get(WS)!
  const cursors: (string | null)[] = []
  server.use(
    http.get(`${API}/cards`, ({ request }) => {
      const cursor = new URL(request.url).searchParams.get("cursor")
      cursors.push(cursor)
      return HttpResponse.json(
        cardPage(
          cursor ? cards.slice(1) : cards.slice(0, 1),
          cursor ? null : "next"
        )
      )
    })
  )
  renderApp(`/w/${WS}/review?tab=cards`)
  await screen.findByTestId("review-cards-table")
  expect(cursors).toEqual([null, "next"])
  expect(screen.getByRole("table")).toHaveAttribute("aria-rowcount", "31")
})

it("uses server interval labels and handles an unavailable retention value", async () => {
  const card = state.db.cards.get(WS)![6]!
  const answer = cardAnswer(card, state.db.now)
  answer.preview[3]!.intervalLabel = "server interval"
  server.use(
    http.get(`${API}/stats`, () =>
      HttpResponse.json({
        dueNow: 1,
        newToday: 0,
        reviewedToday: 0,
        streak: 0,
        retention30d: null,
        forecast: Array(14).fill(0),
      })
    ),
    http.post(`${API}/:id/answer`, () => HttpResponse.json(answer))
  )
  const user = userEvent.setup()
  renderApp(PATH)
  expect(await screen.findByTestId("review-stat-retention")).toHaveTextContent(
    "—"
  )
  await user.click(screen.getByTestId("review-start"))
  await user.keyboard("1")
  expect(await screen.findByTestId("review-rating-4")).toHaveTextContent(
    "server interval"
  )
})

it("restores a failed grade after its card has unmounted", async () => {
  let release!: () => void
  const response = new Promise<void>((resolve) => {
    release = resolve
  })
  server.use(
    http.post(`${API}/:id/grade`, async () => {
      await response
      return HttpResponse.json(
        {
          error: {
            code: "internal",
            message: "disk full",
            requestId: "late_grade",
          },
        },
        { status: 500 }
      )
    })
  )
  const user = userEvent.setup()
  renderApp(PATH)
  await user.click(await screen.findByTestId("review-start"))
  const firstId = screen.getByTestId("review-card").getAttribute("data-card-id")
  await user.keyboard("1")
  await screen.findByTestId("review-rating-bar")
  await user.keyboard("3")
  await waitFor(() => {
    const cards = screen.getAllByTestId("review-card")
    expect(cards).toHaveLength(1)
    expect(cards[0]).not.toHaveAttribute("data-card-id", firstId)
  })
  release()
  expect(await screen.findByText("Could not save rating")).toBeInTheDocument()
  await waitFor(() =>
    expect(screen.getAllByTestId("review-card").at(-1)).toHaveAttribute(
      "data-card-id",
      firstId
    )
  )
  expect(screen.getByTestId("review-progress-label")).toHaveTextContent(
    "Card 1 of 7"
  )
})

it("keeps summary controls disabled until the final grade settles and rolls back on failure", async () => {
  const cards = state.db.cards.get(WS)!
  cards.forEach((c, i) => {
    c.due = i ? state.db.now + 86_400_000 : state.db.now - 1000
  })
  let release!: () => void
  const response = new Promise<void>((resolve) => {
    release = resolve
  })
  server.use(
    http.post(`${API}/:id/grade`, async () => {
      await response
      return HttpResponse.json(
        { error: { code: "internal", message: "failed" } },
        { status: 500 }
      )
    })
  )
  const user = userEvent.setup()
  renderApp(PATH)
  await user.click(await screen.findByTestId("review-start"))
  await user.keyboard("1")
  await screen.findByTestId("review-rating-bar")
  await user.keyboard("3")
  const summary = await screen.findByTestId("review-summary")
  expect(within(summary).getByTestId("review-summary-exit")).toBeDisabled()
  release()
  expect(await screen.findByTestId("review-rating-bar")).toBeInTheDocument()
  expect(screen.queryByTestId("review-summary")).not.toBeInTheDocument()
})

it("updates the card-search draft when the URL changes", async () => {
  const { router } = renderApp(`${PATH}?tab=cards&q=tokio`)
  await screen.findByTestId("review-cards-table")
  expect(screen.getByTestId("review-cards-search")).toHaveValue("tokio")
  await act(() =>
    router.navigate({
      to: "/w/$ws/review",
      params: { ws: WS },
      search: { tab: "cards", q: "Mutex" },
    })
  )
  await waitFor(() =>
    expect(screen.getByTestId("review-cards-search")).toHaveValue("Mutex")
  )
})
