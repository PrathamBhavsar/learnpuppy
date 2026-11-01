import { screen, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it } from "vitest"

import { renderApp, resetUi } from "@/test/render"

const WS = "ws_rustasync"

describe("classroom info", () => {
  it("chat list guide lists the key parts", async () => {
    resetUi()
    renderApp(`/w/${WS}/learn`)
    await userEvent.click(await screen.findByTestId("screen-guide-open"))
    const guide = await screen.findByTestId("screen-guide")
    for (const part of [
      "Lesson phases",
      "Hint ladder",
      "How sure are you?",
      "Jobs",
      "Steer",
    ])
      expect(within(guide).getByText(part)).toBeInTheDocument()
  })

  it("an info tip opens in the open chat", async () => {
    resetUi()
    renderApp(`/w/${WS}/learn/c_teach_quiz`)
    await userEvent.click(
      await screen.findByRole("button", { name: "About Lesson phases" })
    )
    expect(await screen.findByTestId("info-dialog")).toHaveTextContent(
      "Probe: a few quick questions"
    )
  })
})
