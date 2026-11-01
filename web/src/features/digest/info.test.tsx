import { screen, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it } from "vitest"

import { renderApp, resetUi } from "@/test/render"
import { useUi } from "@/stores/ui"

const PATH = "/w/ws_rustasync/digest"

beforeEach(() => {
  resetUi()
  useUi.setState({ reduceMotion: true })
})

describe("digest info", () => {
  it("guide lists the key parts", async () => {
    renderApp(PATH)
    await userEvent.click(await screen.findByTestId("screen-guide-open"))
    const guide = await screen.findByTestId("screen-guide")
    for (const part of [
      "Date range",
      "Stated or inferred",
      "Alternatives considered",
      "Regenerate cards",
    ])
      expect(within(guide).getByText(part)).toBeInTheDocument()
  })

  it("a controls tip opens its dialog", async () => {
    renderApp(PATH)
    await userEvent.click(
      await screen.findByRole("button", { name: "About Sources" })
    )
    expect(await screen.findByTestId("info-dialog")).toHaveTextContent(
      "Claude, Codex, Antigravity or Git"
    )
  })
})
