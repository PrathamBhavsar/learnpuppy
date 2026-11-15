import { act, render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { useUi } from "@/stores/ui"

import { GapPause } from "./GapPause"

beforeEach(() => {
  vi.useFakeTimers({ shouldAdvanceTime: true })
})
afterEach(() => {
  vi.useRealTimers()
  useUi.setState({ reduceMotion: false })
})

describe("GapPause (F24)", () => {
  it("counts down and calls onDone only once the full duration has passed", async () => {
    const onDone = vi.fn()
    render(<GapPause seconds={3} onDone={onDone} />)
    expect(screen.getByTestId("gap-pause")).toHaveTextContent("Pause — 3s")

    await act(() => vi.advanceTimersByTimeAsync(1000))
    expect(screen.getByTestId("gap-pause")).toHaveTextContent("Pause — 2s")
    expect(onDone).not.toHaveBeenCalled()

    await act(() => vi.advanceTimersByTimeAsync(1000))
    expect(onDone).not.toHaveBeenCalled()
    await act(() => vi.advanceTimersByTimeAsync(1000))
    expect(onDone).toHaveBeenCalledTimes(1)
  })

  it("cannot be skipped by a key press", async () => {
    const onDone = vi.fn()
    render(<GapPause seconds={5} onDone={onDone} />)
    await userEvent.setup({ delay: null }).keyboard("{Escape}{Enter} ")
    expect(onDone).not.toHaveBeenCalled()
    expect(screen.getByTestId("gap-pause")).toBeInTheDocument()
  })

  it("has no skip control in the DOM at all", () => {
    render(<GapPause seconds={5} onDone={vi.fn()} />)
    expect(screen.queryByRole("button")).not.toBeInTheDocument()
  })

  it("drops the animated pulse under reduced motion but keeps the countdown", async () => {
    useUi.setState({ reduceMotion: true })
    render(<GapPause seconds={2} onDone={vi.fn()} />)
    expect(screen.queryByTestId("gap-pause-pulse")).not.toBeInTheDocument()
    expect(screen.getByTestId("gap-pause")).toHaveTextContent("Pause — 2s")
  })
})
