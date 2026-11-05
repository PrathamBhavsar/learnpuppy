import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it, vi } from "vitest"

import type { Clock } from "@/api/learning-types"

import { LockedScreen } from "./LockedScreen"

const clock = (overrides: Partial<Clock> = {}): Clock => ({
  bout: null,
  status: { status: "gap", remainingMinutes: 42 },
  minutesToday: 90,
  overrideUsedToday: false,
  gapPauseDue: false,
  gapPauseSeconds: 10,
  boutLimitMinutes: 90,
  dailyLimitMinutes: 270,
  ...overrides,
})

describe("LockedScreen (F23)", () => {
  it("renders nothing while active", () => {
    const { container } = render(
      <LockedScreen
        clock={clock({ status: { status: "active", activeMinutes: 1 } })}
        onOverride={vi.fn()}
        overridePending={false}
      />
    )
    expect(container).toBeEmptyDOMElement()
  })

  it("shows the server's time left during a gap, read straight from the clock", () => {
    render(
      <LockedScreen clock={clock()} onOverride={vi.fn()} overridePending={false} />
    )
    expect(screen.getByTestId("locked-time-left")).toHaveTextContent(
      "Back in 42 min."
    )
  })

  it("requires a typed reason before the override can be used", async () => {
    const onOverride = vi.fn()
    render(
      <LockedScreen clock={clock()} onOverride={onOverride} overridePending={false} />
    )
    const button = screen.getByRole("button", {
      name: "Override, just this once",
    })
    expect(button).toBeDisabled()
    const user = userEvent.setup()
    await user.type(
      screen.getByLabelText("Reason for overriding the limit"),
      "  deadline tonight  "
    )
    expect(button).toBeEnabled()
    await user.click(button)
    expect(onOverride).toHaveBeenCalledWith("deadline tonight")
  })

  it("hides the override form once today's override is already used", () => {
    render(
      <LockedScreen
        clock={clock({ overrideUsedToday: true })}
        onOverride={vi.fn()}
        overridePending={false}
      />
    )
    expect(screen.getByTestId("override-used")).toBeInTheDocument()
    expect(
      screen.queryByRole("button", { name: "Override, just this once" })
    ).not.toBeInTheDocument()
  })

  it("explains the bout and daily limits distinctly", () => {
    render(
      <LockedScreen
        clock={clock({ status: { status: "bout_limit_reached" } })}
        onOverride={vi.fn()}
        overridePending={false}
      />
    )
    expect(screen.getByTestId("screen-locked")).toHaveTextContent(
      /90-minute limit/
    )

    render(
      <LockedScreen
        clock={clock({ status: { status: "daily_limit_reached" } })}
        onOverride={vi.fn()}
        overridePending={false}
      />
    )
    expect(
      screen.getAllByTestId("screen-locked").at(-1)
    ).toHaveTextContent(/study time is used up/)
  })
})
