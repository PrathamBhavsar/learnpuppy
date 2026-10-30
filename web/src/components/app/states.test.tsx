import { act, render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { InboxIcon } from "lucide-react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { Button } from "@/components/ui/button"

import { EmptyState } from "./empty-state"
import { ErrorState } from "./error-state"
import { Kbd } from "./kbd"
import { MasteryRing, masteryLevel } from "./mastery-ring"
import { PageHeader } from "./page-header"
import { SkeletonRows } from "./skeleton-block"

afterEach(() => vi.useRealTimers())

describe("EmptyState", () => {
  it("focuses the call to action after 300 ms when nothing else holds focus", () => {
    vi.useFakeTimers()
    render(
      <EmptyState
        icon={InboxIcon}
        title="Nothing here"
        action={<Button>Start</Button>}
      />
    )
    expect(screen.getByRole("button", { name: "Start" })).not.toHaveFocus()
    act(() => void vi.advanceTimersByTime(300))
    expect(screen.getByRole("button", { name: "Start" })).toHaveFocus()
  })

  it("does not steal focus from an element the user already focused", () => {
    vi.useFakeTimers()
    render(
      <>
        <input aria-label="Search" />
        <EmptyState
          icon={InboxIcon}
          title="Nothing here"
          action={<Button>Start</Button>}
        />
      </>
    )
    screen.getByLabelText("Search").focus()
    act(() => void vi.advanceTimersByTime(400))
    expect(screen.getByLabelText("Search")).toHaveFocus()
  })
})

describe("ErrorState", () => {
  it("shows a spinner while retrying and prevents a double submit", async () => {
    const user = userEvent.setup()
    let finish: () => void = () => {}
    const onRetry = vi.fn(
      () => new Promise<void>((resolve) => (finish = resolve))
    )
    render(
      <ErrorState message="Server down" requestId="req_1" onRetry={onRetry} />
    )
    expect(screen.getByRole("alert")).toHaveTextContent("req_1")

    await user.click(screen.getByTestId("error-retry"))
    expect(screen.getByTestId("error-retry")).toBeDisabled()
    expect(screen.getByRole("status", { name: "Loading" })).toBeInTheDocument()
    expect(onRetry).toHaveBeenCalledTimes(1)

    await act(async () => finish())
    expect(screen.getByTestId("error-retry")).toBeEnabled()
  })
})

describe("small components", () => {
  it("PageHeader exposes a focusable h1 for route-change focus", () => {
    render(
      <PageHeader
        title="Graph"
        description="Nodes"
        actions={<Button>Go</Button>}
      />
    )
    const h1 = screen.getByRole("heading", { level: 1, name: "Graph" })
    expect(h1).toHaveAttribute("tabindex", "-1")
    expect(h1).toHaveAttribute("data-page-title")
  })

  it("MasteryRing maps value to the 0-4 ramp and labels itself", () => {
    expect([0, 0.19, 0.2, 0.55, 0.99, 1, 7, -1].map(masteryLevel)).toEqual([
      0, 0, 1, 2, 4, 4, 4, 0,
    ])
    render(<MasteryRing value={0.5} />)
    expect(screen.getByRole("img", { name: "Mastery 50%" })).toHaveAttribute(
      "data-level",
      "2"
    )
  })

  it("Kbd renders platform key caps for a combo and stays LTR", () => {
    render(<Kbd combo="shift+enter" />)
    const group = screen
      .getByText(/^(Shift|⇧)$/)
      .closest("[data-slot=kbd-group]")
    expect(group).toHaveAttribute("dir", "ltr")
    expect(group).toHaveTextContent(/(Shift|⇧)(Enter|↵)/)
  })

  it("SkeletonRows announces loading once", () => {
    render(<SkeletonRows rows={3} />)
    expect(screen.getByRole("status", { name: "Loading" })).toBeInTheDocument()
  })
})
