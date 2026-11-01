import { screen, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it } from "vitest"

import { renderApp, resetUi } from "@/test/render"

const PATH = "/w/ws_rustasync/ingest"

beforeEach(() => resetUi())

describe("ingest info", () => {
  it("guide lists the key parts", async () => {
    renderApp(PATH)
    await userEvent.click(await screen.findByTestId("screen-guide-open"))
    const guide = await screen.findByTestId("screen-guide")
    for (const part of [
      "Look back",
      "Project folders",
      "Detect projects",
      "Run history",
    ])
      expect(within(guide).getByText(part)).toBeInTheDocument()
  })

  it("the project folders tip opens its dialog", async () => {
    renderApp(PATH)
    await userEvent.click(
      await screen.findByRole("button", { name: "About Project folders" })
    )
    expect(await screen.findByTestId("info-dialog")).toHaveTextContent(
      "only reads sessions that belong to the project folders"
    )
  })
})
