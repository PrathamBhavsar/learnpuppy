import { act, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, http } from "msw"
import { beforeEach, describe, expect, it } from "vitest"

import { state } from "@/test/msw/db"
import { server } from "@/test/msw/server"
import { renderApp, resetUi } from "@/test/render"
import { useUi } from "@/stores/ui"
import { keys } from "@/api/keys"

beforeEach(() => {
  resetUi()
  useUi.setState({ reduceMotion: true })
})

describe("Review all", () => {
  it("keeps loaded workspaces when a refresh fails", async () => {
    const { router } = renderApp("/review-all")
    await screen.findByTestId("review-all-start")
    server.use(
      http.get("*/api/v1/workspaces", () =>
        HttpResponse.json(
          { error: { code: "internal", message: "offline" } },
          { status: 500 }
        )
      )
    )
    await act(() =>
      router.options.context!.queryClient.invalidateQueries({
        queryKey: keys.workspacesAll(),
      })
    )
    expect(await screen.findByTestId("review-all-stale")).toBeInTheDocument()
    expect(screen.getByTestId("review-all-start")).toBeInTheDocument()
  })
  it("lists workspaces with due cards and the total", async () => {
    renderApp("/review-all")
    const section = await screen.findByTestId("screen-review-all")
    expect(section).toHaveAttribute("aria-busy", "true")
    expect(await screen.findByTestId("review-all-total")).toHaveTextContent(
      "19"
    )
    expect(screen.getByTestId("review-all-ws-ws_rustasync")).toHaveTextContent(
      "7 due"
    )
    expect(
      screen.getByTestId("review-all-ws-ws_designnotes")
    ).toHaveTextContent("12 due")
    expect(
      screen.queryByTestId("review-all-ws-ws_distsys")
    ).not.toBeInTheDocument()
  })

  it("shows nothing-due when every workspace is caught up", async () => {
    for (const cards of state.db.cards.values())
      for (const c of cards) c.due = state.db.now + 86_400_000
    for (const ws of state.db.workspaces.values()) ws.stats.dueCards = 0
    renderApp("/review-all")
    expect(await screen.findByText("Nothing due anywhere")).toBeInTheDocument()
  })

  it("shows an error with the request id", async () => {
    server.use(
      http.get("*/api/v1/workspaces", () =>
        HttpResponse.json(
          { error: { code: "internal", message: "x", requestId: "req_w" } },
          { status: 500 }
        )
      )
    )
    renderApp("/review-all")
    expect(await screen.findByTestId("error-state")).toHaveTextContent("req_w")
  })

  it("walks workspaces with an interstitial between them", async () => {
    // Keep the walk short: one due card in each of the two workspaces.
    for (const [id, cards] of state.db.cards) {
      if (id === "ws_distsys") continue
      cards.forEach(
        (c, i) =>
          (c.due =
            i === 0 ? state.db.now - 3_600_000 : state.db.now + 86_400_000)
      )
    }
    for (const id of ["ws_rustasync", "ws_designnotes"])
      state.db.workspaces.get(id)!.stats.dueCards = 1
    const user = userEvent.setup()
    renderApp("/review-all")
    await user.click(await screen.findByTestId("review-all-start"))

    const first = await screen.findByTestId("review-all-interstitial")
    expect(first).toHaveTextContent("Workspace 1 of 2")
    const firstId = first.getAttribute("data-workspace-id")
    await user.click(screen.getByTestId("review-all-continue"))
    await screen.findByTestId("review-card")
    expect(screen.getByTestId("review-all-heading")).toHaveTextContent("(1/2)")
    await user.keyboard("1")
    await screen.findByTestId("review-rating-bar")
    await user.keyboard(" ")
    await user.click(await screen.findByTestId("review-summary-exit"))

    const second = await screen.findByTestId("review-all-interstitial")
    expect(second).toHaveTextContent("Workspace 2 of 2")
    expect(second.getAttribute("data-workspace-id")).not.toBe(firstId)
    await user.click(within(second).getByTestId("review-all-skip"))

    const summary = await screen.findByTestId("review-all-summary")
    expect(summary).toHaveTextContent("Reviewed 1 card across 1 workspace")
    await user.click(screen.getByTestId("review-all-done"))
    await waitFor(() =>
      expect(screen.queryByTestId("review-all-summary")).not.toBeInTheDocument()
    )
  })
})
