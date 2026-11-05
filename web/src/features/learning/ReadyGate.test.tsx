import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { useUi } from "@/stores/ui"

import { FOCUS_SECONDS, ReadyGate } from "./ReadyGate"

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true })
})
afterEach(() => {
  vi.useRealTimers()
  useUi.setState({ reduceMotion: false })
})

async function checkBoth() {
  const user = userEvent.setup({ delay: null })
  await user.click(screen.getByLabelText("Phone is away"))
  await user.click(screen.getByLabelText("Other tabs are closed"))
  return user
}

describe("ReadyGate (F03)", () => {
  it("keeps Begin disabled until both checks are done and the focus timer finishes", async () => {
    const onReady = vi.fn()
    render(<ReadyGate busy={false} onReady={onReady} />)
    expect(screen.getByRole("button", { name: "Begin" })).toBeDisabled()

    await checkBoth()
    expect(screen.getByRole("button", { name: "Begin" })).toBeDisabled()
    expect(screen.getByTestId("focus-timer")).toHaveTextContent(
      `${FOCUS_SECONDS}s`
    )

    await vi.advanceTimersByTimeAsync(FOCUS_SECONDS * 1000)
    expect(screen.getByRole("button", { name: "Begin" })).toBeEnabled()

    await userEvent.setup({ delay: null }).click(
      screen.getByRole("button", { name: "Begin" })
    )
    expect(onReady).toHaveBeenCalledWith({
      phoneAway: true,
      tabsClosed: true,
      focusSeconds: FOCUS_SECONDS,
    })
  })

  it("can be driven entirely by keyboard", async () => {
    const onReady = vi.fn()
    render(<ReadyGate busy={false} onReady={onReady} />)
    const user = userEvent.setup({ delay: null })
    await user.tab()
    expect(screen.getByLabelText("Phone is away")).toHaveFocus()
    await user.keyboard(" ")
    await user.tab()
    expect(screen.getByLabelText("Other tabs are closed")).toHaveFocus()
    await user.keyboard(" ")
    expect(screen.getByLabelText("Phone is away")).toBeChecked()
    expect(screen.getByLabelText("Other tabs are closed")).toBeChecked()

    await vi.advanceTimersByTimeAsync(FOCUS_SECONDS * 1000)
    expect(screen.getByRole("button", { name: "Begin" })).toBeEnabled()
  })

  it("the breathing routine is optional, never required, and always shows its warning", async () => {
    const onReady = vi.fn()
    render(<ReadyGate busy={false} onReady={onReady} />)
    const user = userEvent.setup({ delay: null })
    expect(screen.queryByTestId("breathing-warning")).not.toBeInTheDocument()

    await checkBoth()
    await vi.advanceTimersByTimeAsync(FOCUS_SECONDS * 1000)
    // Ready without ever touching breathing.
    expect(screen.getByRole("button", { name: "Begin" })).toBeEnabled()

    await user.click(
      screen.getByRole("button", { name: "Optional breathing routine" })
    )
    expect(screen.getByTestId("breathing-warning")).toHaveTextContent(
      /never do this in water or while driving/i
    )
    expect(screen.getByTestId("breathing-warning")).toHaveTextContent(
      /always optional/i
    )
    // Still ready — breathing changed nothing about the gate.
    expect(screen.getByRole("button", { name: "Begin" })).toBeEnabled()
  })

  it("respects reduced motion by dropping the animated circle", async () => {
    useUi.setState({ reduceMotion: true })
    render(<ReadyGate busy={false} onReady={vi.fn()} />)
    await userEvent
      .setup({ delay: null })
      .click(screen.getByRole("button", { name: "Optional breathing routine" }))
    expect(screen.queryByTestId("breathing-circle")).not.toBeInTheDocument()
    expect(screen.getByTestId("breathing-warning")).toBeInTheDocument()
  })
})
