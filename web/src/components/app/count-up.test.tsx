import { act, render, screen, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it } from "vitest"

import { renderWithProviders, resetUi } from "@/test/render"
import { useUi } from "@/stores/ui"

import { CountUp } from "./count-up"

beforeEach(resetUi)

describe("CountUp", () => {
  it("renders the initial value with tabular numerals and no live announcements", () => {
    renderWithProviders(<CountUp value={1234} />)
    const node = screen.getByText("1,234")
    expect(node).toHaveClass("tabular-nums")
    expect(node).toHaveAttribute("aria-live", "off")
  })

  it("tweens to the new value instead of jumping", async () => {
    const { rerender } = render(<CountUp value={0} />)
    rerender(<CountUp value={500} />)
    expect(screen.getByText("0")).toBeInTheDocument()
    await waitFor(() => expect(screen.getByText("500")).toBeInTheDocument(), {
      timeout: 2000,
    })
  })

  it("jumps straight to the value under the reduce-motion override", async () => {
    const { rerender } = render(<CountUp value={0} />)
    act(() => useUi.getState().setReduceMotion(true))
    rerender(<CountUp value={500} />)
    await waitFor(() => expect(screen.getByText("500")).toBeInTheDocument(), {
      timeout: 100,
    })
  })

  it("honours a custom formatter and decimals", () => {
    render(<CountUp value={0.5} decimals={1} format={(v) => `${v * 100}%`} />)
    expect(screen.getByText("50%")).toBeInTheDocument()
  })
})
