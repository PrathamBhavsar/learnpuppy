import { QueryClient, QueryClientProvider } from "@tanstack/react-query"
import { act, render, renderHook, screen, waitFor } from "@testing-library/react"
import type { ReactNode } from "react"
import { describe, expect, it } from "vitest"

import { state } from "@/test/msw/db"

import { useClock, useLearningActions } from "./api"
import { LockedScreen } from "./LockedScreen"

const WS = "ws_rustasync"

function wrapper() {
  const qc = new QueryClient({ defaultOptions: { queries: { retry: false } } })
  return {
    qc,
    Wrapper: ({ children }: { children: ReactNode }) => (
      <QueryClientProvider client={qc}>{children}</QueryClientProvider>
    ),
  }
}

/** Mounts the clock-driven lock screen against a fresh `QueryClient`, as a real page load would. */
function ClockGate() {
  const clock = useClock(WS, { pollMs: false })
  if (!clock.data) return null
  return <LockedScreen clock={clock.data} onOverride={() => {}} overridePending={false} />
}

describe("learning clock (F23) is refresh-proof", () => {
  it("a brand-new mount with a brand-new query client still shows the server's gap", async () => {
    state.db.learning.set(WS, {
      clock: {
        bout: null,
        status: { status: "gap", remainingMinutes: 42 },
        minutesToday: 90,
        overrideUsedToday: false,
        gapPauseDue: false,
        gapPauseSeconds: 10,
        boutLimitMinutes: 90,
        dailyLimitMinutes: 270,
      },
      checkinNeeded: false,
    })

    const first = wrapper()
    const { unmount } = render(
      <first.Wrapper>
        <ClockGate />
      </first.Wrapper>
    )
    await waitFor(() =>
      expect(screen.getByTestId("locked-time-left")).toHaveTextContent(
        "Back in 42 min."
      )
    )
    unmount()
    first.qc.clear()

    // Simulates a page refresh: nothing survives except what the mock server holds.
    const second = wrapper()
    render(
      <second.Wrapper>
        <ClockGate />
      </second.Wrapper>
    )
    await waitFor(() =>
      expect(screen.getByTestId("locked-time-left")).toHaveTextContent(
        "Back in 42 min."
      )
    )
  })
})

describe("useLearningActions (F23)", () => {
  it("refuses to start a bout the ready gate was not completed for", async () => {
    const { Wrapper } = wrapper()
    const { result } = renderHook(() => useLearningActions(WS), {
      wrapper: Wrapper,
    })
    act(() =>
      result.current.startBout.mutate({
        course: "c1",
        ready: { phoneAway: false, tabsClosed: true, focusSeconds: 30 },
      })
    )
    await waitFor(() => expect(result.current.startBout.isError).toBe(true))
    expect(result.current.startBout.error).toMatchObject({
      details: { gate: "not_ready" },
    })
  })

  it("runs start, heartbeat and end, and the clock moves only on the server's say", async () => {
    const { qc, Wrapper } = wrapper()
    const { result } = renderHook(() => useLearningActions(WS), {
      wrapper: Wrapper,
    })

    act(() =>
      result.current.startBout.mutate({
        course: "c1",
        ready: { phoneAway: true, tabsClosed: true, focusSeconds: 30 },
      })
    )
    await waitFor(() =>
      expect(result.current.startBout.data).toMatchObject({
        status: { status: "active", activeMinutes: 0 },
      })
    )

    act(() => result.current.heartbeat.mutate("b_mock"))
    await waitFor(() =>
      expect(result.current.heartbeat.data).toMatchObject({
        status: { status: "active", activeMinutes: 1 },
        minutesToday: 1,
      })
    )

    act(() => result.current.endBout.mutate("b_mock"))
    await waitFor(() =>
      expect(result.current.endBout.data?.status).toMatchObject({
        status: "gap",
      })
    )
    expect(qc.getQueryData(["ws", WS, "learning", "clock"])).toMatchObject({
      status: { status: "gap" },
    })
  })

  it("an override lifts the gap and is recorded on the clock", async () => {
    state.db.learning.set(WS, {
      clock: {
        bout: null,
        status: { status: "gap", remainingMinutes: 100 },
        minutesToday: 90,
        overrideUsedToday: false,
        gapPauseDue: false,
        gapPauseSeconds: 10,
        boutLimitMinutes: 90,
        dailyLimitMinutes: 270,
      },
      checkinNeeded: false,
    })
    const { Wrapper } = wrapper()
    const { result } = renderHook(
      () => ({ clock: useClock(WS, { pollMs: false }), ...useLearningActions(WS) }),
      { wrapper: Wrapper }
    )
    await waitFor(() =>
      expect(result.current.clock.data?.status.status).toBe("gap")
    )
    act(() => result.current.override.mutate({ reason: "deadline" }))
    await waitFor(() =>
      expect(result.current.clock.data?.overrideUsedToday).toBe(true)
    )
  })
})
