import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it, vi } from "vitest"

import { SleepCheckin } from "./SleepCheckin"

describe("SleepCheckin (F04)", () => {
  it("keeps Continue disabled until a valid hour count is entered", async () => {
    const onSubmit = vi.fn()
    render(
      <SleepCheckin busy={false} suggestReviewOnly={null} onSubmit={onSubmit} />
    )
    const continueButton = screen.getByRole("button", { name: "Continue" })
    expect(continueButton).toBeDisabled()

    const user = userEvent.setup()
    await user.type(screen.getByLabelText("Hours of sleep"), "30")
    expect(continueButton).toBeDisabled()

    await user.clear(screen.getByLabelText("Hours of sleep"))
    await user.type(screen.getByLabelText("Hours of sleep"), "6.5")
    expect(continueButton).toBeEnabled()

    await user.type(
      screen.getByLabelText("Anything else about last night"),
      "  woke up once  "
    )
    await user.click(continueButton)
    expect(onSubmit).toHaveBeenCalledWith(6.5, "woke up once")
  })

  it("suggests a review-only day on poor sleep, once the server has graded it", () => {
    render(
      <SleepCheckin busy={false} suggestReviewOnly={true} onSubmit={vi.fn()} />
    )
    expect(screen.getByTestId("step-checkin-done")).toHaveTextContent(
      /review-only day/i
    )
  })

  it("otherwise just confirms the check-in was logged", () => {
    render(
      <SleepCheckin
        busy={false}
        suggestReviewOnly={false}
        onSubmit={vi.fn()}
      />
    )
    expect(screen.getByTestId("step-checkin-done")).toHaveTextContent(
      /logged/i
    )
  })
})
