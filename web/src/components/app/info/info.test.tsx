import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it, vi } from "vitest"

import { TooltipProvider } from "@/components/ui/tooltip"

import { InfoTip, ScreenGuide } from "./info"

const info = {
  title: "Due cards",
  short: "Cards that are ready to review today.",
  body: ["Each card is scheduled by FSRS."],
  tips: ["Press 1-4 to grade."],
}

describe("InfoTip", () => {
  it("opens the full explanation on click without triggering the parent", async () => {
    const parent = vi.fn()
    render(
      <TooltipProvider>
        {/* eslint-disable-next-line jsx-a11y/click-events-have-key-events, jsx-a11y/no-static-element-interactions -- probes click bubbling */}
        <div onClick={parent}>
          <InfoTip info={info} />
        </div>
      </TooltipProvider>
    )
    await userEvent.click(
      screen.getByRole("button", { name: "About Due cards" })
    )
    expect(await screen.findByTestId("info-dialog")).toHaveTextContent(
      "Each card is scheduled by FSRS."
    )
    expect(screen.getByTestId("info-dialog")).toHaveTextContent(
      "Press 1-4 to grade."
    )
    expect(parent).not.toHaveBeenCalled()
  })

  it("opens from the keyboard", async () => {
    render(
      <TooltipProvider>
        <InfoTip info={info} />
      </TooltipProvider>
    )
    await userEvent.tab()
    await userEvent.keyboard("{Enter}")
    expect(await screen.findByTestId("info-dialog")).toBeInTheDocument()
  })
})

describe("ScreenGuide", () => {
  it("lists every part of the screen", async () => {
    render(
      <ScreenGuide title="Review" summary="Study due cards." parts={[info]} />
    )
    await userEvent.click(screen.getByTestId("screen-guide-open"))
    expect(await screen.findByTestId("screen-guide")).toHaveTextContent(
      "Due cards"
    )
    expect(screen.getByTestId("screen-guide")).toHaveTextContent(
      "Press 1-4 to grade."
    )
  })
})
