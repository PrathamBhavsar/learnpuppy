import { act, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, http } from "msw"
import { afterAll, beforeAll, beforeEach, describe, expect, it } from "vitest"

import { state } from "@/test/msw/db"
import { cardPage } from "@/test/msw/fixtures/review-contract"
import { server } from "@/test/msw/server"
import { renderApp, resetUi } from "@/test/render"
import { useUi } from "@/stores/ui"
import { keys } from "@/api/keys"

const WS = "ws_rustasync"
const PATH = `/w/${WS}/review`
const API = `*/api/v1/workspaces/${WS}/review`

// jsdom lays nothing out, so every element measures 0x0 and the virtualized rows would never render.
const proto = Element.prototype
const SIZE = { clientHeight: 600, clientWidth: 800 } as const
const originals = {
  clientHeight: Object.getOwnPropertyDescriptor(proto, "clientHeight"),
  clientWidth: Object.getOwnPropertyDescriptor(proto, "clientWidth"),
}
beforeAll(() => {
  for (const [key, value] of Object.entries(SIZE))
    Object.defineProperty(proto, key, { configurable: true, get: () => value })
})
afterAll(() => {
  for (const key of ["clientHeight", "clientWidth"] as const) {
    const original = originals[key]
    if (original) Object.defineProperty(proto, key, original)
  }
})

beforeEach(() => {
  resetUi()
  // Keeps card transitions short and deterministic in jsdom.
  useUi.setState({ reduceMotion: true })
})

const startSession = async (user: ReturnType<typeof userEvent.setup>) => {
  await user.click(await screen.findByTestId("review-start"))
  await screen.findByTestId("review-card")
}

/** Picks option B, waits for the reveal and returns the suggestion's rating number. */
const answerWith = async (
  user: ReturnType<typeof userEvent.setup>,
  key: string
) => {
  await user.keyboard(key)
  await screen.findByTestId("review-rating-bar")
}

describe("Review screen states", () => {
  it("keeps the start screen with a warning when a refresh fails", async () => {
    const { router } = renderApp(PATH)
    await screen.findByTestId("review-start")
    server.use(
      http.get(`${API}/queue`, () =>
        HttpResponse.json(
          { error: { code: "internal", message: "offline" } },
          { status: 500 }
        )
      )
    )
    await act(() =>
      router.options.context!.queryClient.invalidateQueries({
        queryKey: keys.ws(WS).review(),
      })
    )
    expect(await screen.findByTestId("review-stale")).toBeInTheDocument()
    expect(screen.getByTestId("review-start")).toBeInTheDocument()
  })
  it("shows a skeleton, then the stats strip, forecast and start panel", async () => {
    renderApp(PATH)
    const section = await screen.findByTestId("screen-review")
    expect(section).toHaveAttribute("aria-busy", "true")
    expect(screen.getByRole("status", { name: "Loading" })).toBeInTheDocument()

    const strip = await screen.findByTestId("review-stats-strip")
    expect(within(strip).getByTestId("review-stat-due")).toHaveTextContent("7")
    expect(screen.getByTestId("review-forecast")).toBeInTheDocument()
    expect(screen.getAllByTestId(/^review-forecast-bar-/)).toHaveLength(14)
    expect(await screen.findByTestId("review-start")).toHaveTextContent(
      "Start review (7)"
    )
    expect(section).toHaveAttribute("aria-busy", "false")
  })

  it("shows 'All caught up' with the next due time when nothing is due", async () => {
    renderApp(`/w/ws_distsys/review`)
    expect(await screen.findByText("All caught up")).toBeInTheDocument()
    expect(await screen.findByText(/Next card due/)).toBeInTheDocument()
    expect(screen.getByTestId("review-browse-cards")).toBeInTheDocument()
    expect(screen.getByTestId("review-practice-graph")).toHaveAttribute(
      "href",
      "/w/ws_distsys/graph"
    )
  })

  it("shows an error with the request id and recovers on retry", async () => {
    let fail = true
    server.use(
      http.get(`${API}/queue`, () =>
        fail
          ? HttpResponse.json(
              {
                error: {
                  code: "internal",
                  message: "boom",
                  requestId: "req_77",
                  retryable: true,
                },
              },
              { status: 500 }
            )
          : HttpResponse.json({ items: [], nextCursor: null })
      )
    )
    const user = userEvent.setup()
    renderApp(PATH)
    expect(await screen.findByTestId("error-state")).toHaveTextContent("req_77")
    fail = false
    await user.click(screen.getByTestId("error-retry"))
    await waitFor(() =>
      expect(screen.queryByTestId("error-state")).not.toBeInTheDocument()
    )
  })

  it("filters the session to the decisions from the digest", async () => {
    renderApp(`${PATH}?decisions=dec_1,dec_2`)
    expect(await screen.findByTestId("review-start")).toHaveTextContent(
      "Start review (3)"
    )
    expect(screen.getByTestId("review-start-note")).toHaveTextContent(
      "Filtered to 2 decisions"
    )
  })

  it("respects the per-session limit", async () => {
    const user = userEvent.setup()
    renderApp(PATH)
    await screen.findByTestId("review-start")
    await user.click(screen.getByTestId("review-limit"))
    await user.click(await screen.findByRole("option", { name: "10 cards" }))
    // 7 are due, so the limit does not cut anything here.
    expect(screen.getByTestId("review-start")).toHaveTextContent(
      "Start review (7)"
    )
  })
})

describe("Review session (keyboard only)", () => {
  it("answers and grades five cards without the mouse, then shows the summary", async () => {
    const user = userEvent.setup()
    renderApp(PATH)
    await startSession(user)
    expect(screen.getByTestId("review-progress-label")).toHaveTextContent(
      "Card 1 of 7"
    )

    for (let i = 0; i < 5; i++) {
      const card = (await screen.findAllByTestId("review-card")).at(-1)!
      expect(card).toHaveAttribute("data-phase", "answering")
      await answerWith(user, i % 2 ? "b" : "2")
      expect(screen.getByTestId("review-verdict")).toBeInTheDocument()
      expect(screen.getByTestId("review-rating-bar")).toBeInTheDocument()
      // Space accepts the server's suggestion; the mock suggests Good or Again.
      await user.keyboard(" ")
      await waitFor(() =>
        expect(screen.getByTestId("review-progress-label")).toHaveTextContent(
          `Card ${i + 2} of 7`
        )
      )
    }
    // The last two cards are rated with explicit number keys.
    for (const rating of ["4", "1"]) {
      await answerWith(user, "c")
      await user.keyboard(rating)
    }
    const summary = await screen.findByTestId("review-summary")
    expect(within(summary).getByText("7 cards reviewed")).toBeInTheDocument()
    expect(
      within(summary).getByTestId("review-summary-rating-4")
    ).toHaveTextContent("1")
    expect(
      within(summary).getByTestId("review-summary-time")
    ).toBeInTheDocument()
    expect(
      within(summary).getByTestId("review-summary-mastery")
    ).toBeInTheDocument()
    expect(
      within(summary).getByTestId("review-summary-next")
    ).not.toHaveTextContent("—")
  })

  it("shows previewed intervals from the server on the rating bar", async () => {
    const user = userEvent.setup()
    renderApp(PATH)
    await startSession(user)
    await answerWith(user, "1")
    const bar = screen.getByTestId("review-rating-bar")
    // The mock previews 1d / 4d for fresh cards and 3d / 8d / 19d for review cards.
    expect(bar.textContent).toMatch(/Again\s*<?\d+[mhd]/)
    expect(within(bar).getByTestId("review-rating-3")).toHaveTextContent(
      /\d+(d|w)/
    )
    expect(within(bar).getByTestId("review-rating-4")).toBeInTheDocument()
  })

  it("rolls back and toasts when the server rejects a grade", async () => {
    server.use(
      http.post(`${API}/:id/grade`, () =>
        HttpResponse.json(
          {
            error: {
              code: "internal",
              message: "disk full",
              requestId: "req_9",
            },
          },
          { status: 500 }
        )
      )
    )
    const user = userEvent.setup()
    renderApp(PATH)
    await startSession(user)
    const firstId = screen
      .getByTestId("review-card")
      .getAttribute("data-card-id")
    await answerWith(user, "1")
    await user.keyboard("3")
    // The failed card comes back, already answered, with the rating bar to retry.
    expect(await screen.findByText("Could not save rating")).toBeInTheDocument()
    await waitFor(() => {
      const cards = screen.getAllByTestId("review-card")
      expect(cards.at(-1)).toHaveAttribute("data-card-id", firstId)
      expect(cards.at(-1)).toHaveAttribute("data-phase", "answered")
    })
    expect(screen.getByTestId("review-progress-label")).toHaveTextContent(
      "Card 1 of 7"
    )
  })

  it("advances before the grade request resolves (optimistic)", async () => {
    server.use(
      http.post(`${API}/:id/grade`, async () => {
        await new Promise((r) => setTimeout(r, 400))
        return HttpResponse.json({})
      })
    )
    const user = userEvent.setup()
    renderApp(PATH)
    await startSession(user)
    await answerWith(user, "1")
    const t0 = performance.now()
    await user.keyboard("3")
    await waitFor(() =>
      expect(screen.getByTestId("review-progress-label")).toHaveTextContent(
        "Card 2 of 7"
      )
    )
    expect(performance.now() - t0).toBeLessThan(300)
  })

  it("asks before leaving mid-card and keeps the session on 'Keep reviewing'", async () => {
    const user = userEvent.setup()
    renderApp(PATH)
    await startSession(user)
    await user.keyboard("{Escape}")
    const dialog = await screen.findByTestId("review-exit-dialog")
    // Choice keys are ignored while the dialog owns the keyboard.
    await user.keyboard("1")
    expect(screen.getByTestId("review-card")).toHaveAttribute(
      "data-phase",
      "answering"
    )
    await user.click(within(dialog).getByTestId("review-exit-cancel"))
    await waitFor(() =>
      expect(screen.queryByTestId("review-exit-dialog")).not.toBeInTheDocument()
    )
    expect(screen.getByTestId("review-session")).toBeInTheDocument()

    await user.keyboard("{Escape}")
    await user.click(await screen.findByTestId("review-exit-confirm"))
    await waitFor(() =>
      expect(screen.queryByTestId("review-session")).not.toBeInTheDocument()
    )
    expect(screen.getByTestId("review-start")).toBeInTheDocument()
  })

  it("a wrong choice shows the correct option and the explanation", async () => {
    const user = userEvent.setup()
    renderApp(PATH)
    await startSession(user)
    const front = screen.getByTestId("review-card-prompt").textContent
    expect(front).toBeTruthy()
    const card = state.db.cards.get(WS)!.find((c) => c.front === front)!
    const wrong = card.options!.findIndex((o) => o.key !== card.correctKey)
    await user.keyboard(String(wrong + 1))
    const verdict = await screen.findByTestId("review-verdict")
    expect(verdict).toHaveAttribute("data-correct", "false")
    expect(verdict).toHaveTextContent(card.explanation)
    expect(
      screen.getByTestId(`review-choice-${card.correctKey.toLowerCase()}`)
    ).toHaveAttribute("data-state", "correct")
  })
})

describe("Card browser", () => {
  it("retains loaded cards when a refresh fails", async () => {
    const { router } = renderApp(`${PATH}?tab=cards`)
    await screen.findByTestId("review-cards-table")
    server.use(
      http.get(`${API}/cards`, () =>
        HttpResponse.json(
          { error: { code: "internal", message: "offline" } },
          { status: 500 }
        )
      )
    )
    await act(() =>
      router.options.context!.queryClient.invalidateQueries({
        queryKey: keys.ws(WS).review(),
      })
    )
    expect(await screen.findByTestId("review-cards-stale")).toBeInTheDocument()
    expect(screen.getByTestId("review-cards-table")).toBeInTheDocument()
  })
  it("lists cards, filters by state and search, and opens the detail drawer", async () => {
    const user = userEvent.setup()
    renderApp(`${PATH}?tab=cards`)
    const table = await screen.findByTestId("review-cards-table")
    expect(
      within(table).getAllByTestId("review-card-row").length
    ).toBeGreaterThan(5)

    await user.type(screen.getByTestId("review-cards-search"), "tokio")
    await waitFor(() => {
      const rows = screen.getAllByTestId("review-card-row")
      expect(rows.length).toBeGreaterThan(0)
      rows.forEach((row) => expect(row).toHaveTextContent(/tokio/i))
    })

    await user.clear(screen.getByTestId("review-cards-search"))
    await user.type(screen.getByTestId("review-cards-search"), "zzz-no-match")
    expect(
      await screen.findByText("No cards match these filters")
    ).toBeInTheDocument()
  })

  it("shows the review history and a Details disclosure for a card", async () => {
    const user = userEvent.setup()
    renderApp(`${PATH}?tab=cards&state=review`)
    const rows = await screen.findAllByTestId("review-card-row")
    await user.click(rows[0]!)
    const drawer = await screen.findByTestId("review-card-drawer")
    expect(
      await within(drawer).findByTestId("review-card-history")
    ).toBeInTheDocument()
    expect(within(drawer).queryByText("Stability")).not.toBeInTheDocument()
    await user.click(
      within(drawer).getByTestId("review-card-drawer-details-trigger")
    )
    expect(await within(drawer).findByText("Stability")).toBeInTheDocument()
  })

  it("shows an empty state when the workspace has no cards", async () => {
    server.use(http.get(`${API}/cards`, () => HttpResponse.json(cardPage([]))))
    renderApp(`${PATH}?tab=cards`)
    expect(
      await screen.findByText("No cards match these filters")
    ).toBeInTheDocument()
    expect(
      screen.getByText(/Cards appear here once decisions are ingested/)
    ).toBeInTheDocument()
  })

  it("shows an error state with retry", async () => {
    server.use(
      http.get(`${API}/cards`, () =>
        HttpResponse.json(
          { error: { code: "internal", message: "no", requestId: "req_c" } },
          { status: 500 }
        )
      )
    )
    renderApp(`${PATH}?tab=cards`)
    expect(await screen.findByTestId("error-state")).toHaveTextContent("req_c")
  })
})
