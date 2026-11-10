import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { act, render, screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { http, HttpResponse } from "msw"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import type { Clock } from "@/api/learning-types"
import { state } from "@/test/msw/db"
import { learningFixture } from "@/test/msw/fixtures/learning"
import { server } from "@/test/msw/server"

import { BoutShell } from "./BoutShell"

const WS = "ws_rustasync"

function seed(patch: Partial<ReturnType<typeof learningFixture>> = {}) {
  state.db.learning.set(WS, { ...learningFixture(), ...patch })
}

function clockOf(patch: Partial<Clock>): Clock {
  return { ...learningFixture().clock, ...patch }
}

function mount(onClosedSpy?: (close: () => void) => void) {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return render(
    <QueryClientProvider client={qc}>
      <BoutShell ws={WS} course="c1">
        {({ onClosed }) => {
          onClosedSpy?.(onClosed)
          return <p data-testid="session">session</p>
        }}
      </BoutShell>
    </QueryClientProvider>
  )
}

describe("BoutShell (F03, F04, F23, F24, F25)", () => {
  beforeEach(() => {
    vi.spyOn(document, "hidden", "get").mockReturnValue(false)
  })
  afterEach(() => {
    vi.restoreAllMocks()
    vi.useRealTimers()
  })

  it("asks how you slept on the first bout of the day, then shows the ready gate", async () => {
    seed({ checkinNeeded: true })
    const user = userEvent.setup()
    mount()
    await user.type(await screen.findByLabelText("Hours of sleep"), "5")
    await user.click(screen.getByRole("button", { name: "Continue" }))
    expect(await screen.findByTestId("step-checkin-done")).toHaveTextContent(
      "review-only"
    )
    await user.click(screen.getByRole("button", { name: "Continue" }))
    expect(await screen.findByTestId("step-ready")).toBeInTheDocument()
    expect(screen.queryByTestId("session")).not.toBeInTheDocument()
  })

  it("goes straight to the ready gate once today's check-in is done", async () => {
    seed({ checkinNeeded: false })
    mount()
    expect(await screen.findByTestId("step-ready")).toBeInTheDocument()
  })

  it("shows the lock instead of the session inside the gap, with only a typed override", async () => {
    seed({
      checkinNeeded: false,
      clock: clockOf({ status: { status: "gap", remainingMinutes: 42 } }),
    })
    mount()
    expect(await screen.findByTestId("screen-locked")).toBeInTheDocument()
    expect(screen.getByTestId("locked-time-left")).toHaveTextContent("42")
    expect(screen.queryByTestId("session")).not.toBeInTheDocument()
    expect(screen.queryByTestId("step-ready")).not.toBeInTheDocument()
  })

  it("renders the session with the clock when a bout is already open, and keeps heartbeating", async () => {
    seed({
      checkinNeeded: false,
      clock: clockOf({
        bout: "b_open",
        status: { status: "active", activeMinutes: 12 },
        minutesToday: 12,
      }),
    })
    let beats = 0
    server.use(
      http.post("*/learning/bout/heartbeat", () => {
        beats += 1
        return HttpResponse.json(
          clockOf({
            bout: "b_open",
            status: { status: "active", activeMinutes: 12 },
            minutesToday: 12,
          })
        )
      })
    )
    mount()
    expect(await screen.findByTestId("session")).toBeInTheDocument()
    expect(screen.getByTestId("bout-clock")).toHaveTextContent(
      "12 of 90 min this bout"
    )
    await waitFor(() => expect(beats).toBeGreaterThanOrEqual(1))
  })

  it("reports leaving the tab to the server", async () => {
    seed({
      checkinNeeded: false,
      clock: clockOf({
        bout: "b_open",
        status: { status: "active", activeMinutes: 1 },
      }),
    })
    const lost: string[] = []
    server.use(
      http.post("*/learning/bout/focus-lost", async ({ request }) => {
        lost.push(((await request.json()) as { bout: string }).bout)
        return new HttpResponse(null, { status: 204 })
      })
    )
    mount()
    await screen.findByTestId("session")
    vi.spyOn(document, "hidden", "get").mockReturnValue(true)
    act(() => {
      document.dispatchEvent(new Event("visibilitychange"))
    })
    await waitFor(() => expect(lost).toEqual(["b_open"]))
  })

  it("shows a blank pause once when one is due, never twice for the same minute", async () => {
    seed({
      checkinNeeded: false,
      clock: clockOf({
        bout: "b_open",
        status: { status: "active", activeMinutes: 5 },
        gapPauseDue: true,
        gapPauseSeconds: 1,
      }),
    })
    server.use(
      // The server keeps the pause due until the minute moves on.
      http.post("*/learning/bout/heartbeat", () =>
        HttpResponse.json(
          clockOf({
            bout: "b_open",
            status: { status: "active", activeMinutes: 5 },
            gapPauseDue: true,
            gapPauseSeconds: 1,
          })
        )
      )
    )
    mount()
    expect(await screen.findByTestId("gap-pause")).toBeInTheDocument()
    await waitFor(
      () => expect(screen.queryByTestId("gap-pause")).not.toBeInTheDocument(),
      { timeout: 4000 }
    )
    await new Promise((r) => setTimeout(r, 300))
    expect(screen.queryByTestId("gap-pause")).not.toBeInTheDocument()
    expect(screen.getByTestId("session")).toBeInTheDocument()
  })

  it("ends the bout when the session closes, and keeps showing the session", async () => {
    seed({
      checkinNeeded: false,
      clock: clockOf({
        bout: "b_open",
        status: { status: "active", activeMinutes: 30 },
      }),
    })
    let ended = 0
    server.use(
      http.post("*/learning/bout/end", () => {
        ended += 1
        return HttpResponse.json(
          clockOf({
            bout: null,
            status: { status: "gap", remainingMinutes: 120 },
          })
        )
      })
    )
    let close: () => void = () => {}
    mount((c) => {
      close = c
    })
    await screen.findByTestId("session")
    act(() => close())
    await waitFor(() => expect(ended).toBe(1))
    expect(await screen.findByTestId("bout-ended")).toBeInTheDocument()
    // The Close report must stay on screen after the bout ends.
    expect(screen.getByTestId("session")).toBeInTheDocument()
    expect(screen.queryByTestId("screen-locked")).not.toBeInTheDocument()
  })
})
