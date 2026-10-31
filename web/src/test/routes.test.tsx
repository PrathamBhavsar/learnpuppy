import { screen, waitFor } from "@testing-library/react"
import { beforeEach, describe, expect, it } from "vitest"

import { renderApp, resetUi } from "@/test/render"

beforeEach(resetUi)

const SCREENS: [path: string, testId: string, heading: string][] = [
  ["/w/ws_rustasync/review", "screen-review", "Review"],
  ["/w/ws_rustasync/digest", "screen-digest", "Digest"],
  ["/w/ws_rustasync/ingest", "screen-ingest", "Ingest"],
  ["/w/ws_rustasync/settings", "screen-settings", "Workspace settings"],
  ["/accounts", "screen-accounts", "Accounts"],
  ["/settings", "screen-settings", "Settings"],
  ["/trash", "screen-trash", "Trash"],
  ["/review-all", "screen-review-all", "Review all"],
]

describe("route tree", () => {
  it.each(SCREENS)(
    "%s shows a skeleton, then its empty state",
    async (path, testId, heading) => {
      renderApp(path)
      const screenEl = await screen.findByTestId(testId)
      expect(
        screen.getByRole("heading", { level: 1, name: heading })
      ).toBeInTheDocument()
      expect(screenEl).toHaveAttribute("aria-busy", "true")
      expect(
        screen.getByRole("status", { name: "Loading" })
      ).toBeInTheDocument()
      await waitFor(
        () => expect(screen.getByTestId("empty-state")).toBeInTheDocument(),
        { timeout: 2000 }
      )
      expect(screenEl).toHaveAttribute("aria-busy", "false")
    }
  )

  it("learn shows the two-pane layout with an empty classroom, and a chat inside it", async () => {
    const first = renderApp("/w/ws_rustasync/learn")
    await screen.findByTestId("screen-learn")
    expect(
      await screen.findByText("Start learning something")
    ).toBeInTheDocument()
    first.unmount()

    renderApp("/w/ws_rustasync/learn/chat-42")
    expect(await screen.findByText(/chat chat-42/)).toBeInTheDocument()
    expect(screen.getByTestId("screen-learn")).toBeInTheDocument()
  })

  it("home lists the workspaces", async () => {
    renderApp("/")
    expect(await screen.findByTestId("home")).toBeInTheDocument()
    expect(
      await screen.findByTestId("workspace-card-ws_rustasync")
    ).toHaveAttribute("href", "/w/ws_rustasync/graph")
  })

  it("unknown paths show the not-found state with a way back", async () => {
    renderApp("/nope/at/all")
    expect(await screen.findByText("Page not found")).toBeInTheDocument()
    expect(
      screen.getByRole("link", { name: "Back to workspaces" })
    ).toHaveAttribute("href", "/")
  })

  it("sets the document title as Section · Workspace · LearnPuppy", async () => {
    renderApp("/w/ws_rustasync/digest")
    await screen.findByTestId("screen-digest")
    await waitFor(() =>
      expect(document.title).toBe("Digest · Rust async · LearnPuppy")
    )
  })
})
