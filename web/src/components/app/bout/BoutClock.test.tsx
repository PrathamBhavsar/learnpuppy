import { render, screen } from "@testing-library/react"
import { describe, expect, it } from "vitest"

import type { Clock } from "@/api/learning-types"

import { BoutClock } from "./BoutClock"

const base: Clock = {
  bout: "b_1",
  status: { status: "active", activeMinutes: 47 },
  minutesToday: 47,
  overrideUsedToday: false,
  gapPauseDue: false,
  gapPauseSeconds: 10,
  boutLimitMinutes: 90,
  dailyLimitMinutes: 270,
}

describe("BoutClock (F23)", () => {
  it("shows the server's active minutes, with nothing computed client-side", () => {
    render(<BoutClock clock={base} />)
    expect(screen.getByTestId("bout-clock")).toHaveTextContent(
      "47 of 90 min this bout"
    )
    expect(screen.getByTestId("bout-clock")).toHaveTextContent(
      "47 of 270 min today"
    )
    expect(screen.getByRole("progressbar")).toBeInTheDocument()
  })

  it("shows a full bar once the bout limit is reached", () => {
    render(
      <BoutClock
        clock={{ ...base, status: { status: "bout_limit_reached" } }}
      />
    )
    expect(screen.getByTestId("bout-clock")).toHaveTextContent(
      "90 of 90 min this bout"
    )
  })
})
