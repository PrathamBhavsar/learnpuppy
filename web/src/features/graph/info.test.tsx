import { render, screen, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it } from "vitest"

import { ScreenGuide } from "@/components/app/info/info"
import { TooltipProvider } from "@/components/ui/tooltip"

import { graphGuide } from "./info"
import { Legend } from "./ui/Legend"

describe("graph info", () => {
  it("guide lists the key parts", async () => {
    render(
      <TooltipProvider>
        <ScreenGuide {...graphGuide} />
      </TooltipProvider>
    )
    await userEvent.click(screen.getByTestId("screen-guide-open"))
    const guide = await screen.findByTestId("screen-guide")
    for (const part of [
      "Source filter",
      "Provenance",
      "Delete safely",
      "Chain table",
    ])
      expect(within(guide).getByText(part)).toBeInTheDocument()
  })

  it("legend tip opens its dialog", async () => {
    render(
      <TooltipProvider>
        <Legend />
      </TooltipProvider>
    )
    await userEvent.click(screen.getByRole("button", { name: "About Legend" }))
    expect(await screen.findByTestId("info-dialog")).toHaveTextContent(
      "Colour tells you the source"
    )
  })
})
