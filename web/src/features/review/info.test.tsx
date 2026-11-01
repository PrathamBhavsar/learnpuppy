import { screen, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it } from "vitest"

import { renderApp, resetUi } from "@/test/render"
import { useUi } from "@/stores/ui"

beforeEach(() => {
  resetUi()
  useUi.setState({ reduceMotion: true })
})

describe("review info", () => {
  it("guide lists the key parts", async () => {
    renderApp("/w/ws_rustasync/review")
    await userEvent.click(await screen.findByTestId("screen-guide-open"))
    const guide = await screen.findByTestId("screen-guide")
    for (const part of [
      "Spaced repetition and FSRS",
      "Streak",
      "How well did you recall it?",
      "14-day forecast",
    ])
      expect(within(guide).getByText(part)).toBeInTheDocument()
  })

  it("a stats tip explains the streak", async () => {
    renderApp("/w/ws_rustasync/review")
    await userEvent.click(
      await screen.findByRole("button", { name: "About Streak" })
    )
    expect(await screen.findByTestId("info-dialog")).toHaveTextContent(
      "Days in a row"
    )
  })

  it("rating tip does not rate the card behind it", async () => {
    renderApp("/w/ws_rustasync/review")
    await userEvent.click(await screen.findByTestId("review-start"))
    await screen.findByTestId("review-card")
    await userEvent.keyboard("b")
    await screen.findByTestId("review-rating-bar")
    await userEvent.click(
      screen.getByRole("button", { name: "About How well did you recall it?" })
    )
    await screen.findByTestId("info-dialog")
    await userEvent.keyboard("1")
    expect(screen.getByTestId("review-rating-bar")).toBeInTheDocument()
  })

  it("review all has its own guide", async () => {
    renderApp("/review-all")
    await userEvent.click(await screen.findByTestId("screen-guide-open"))
    expect(await screen.findByTestId("screen-guide")).toHaveTextContent(
      "up to 20 cards from each"
    )
  })
})
