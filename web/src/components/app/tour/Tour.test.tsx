import { act, screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it } from "vitest"

import { renderApp, resetUi } from "@/test/render"

import { TOUR_KEY } from "./tour-store"

const WS = "/w/ws_rustasync/graph"

async function openTour(path: string) {
  renderApp(path, { tour: true })
  await screen.findByTestId("tour-splash", {}, { timeout: 3000 })
  await userEvent.click(screen.getByTestId("tour-start"))
  await screen.findByTestId("tour-card")
}

const title = () => screen.getByTestId("tour-title").textContent

async function walk(): Promise<string[]> {
  const seen: string[] = []
  for (let i = 0; i < 30; i += 1) {
    seen.push(title() ?? "")
    if (screen.getByTestId("tour-next").textContent === "Finish") break
    await userEvent.click(screen.getByTestId("tour-next"))
  }
  return seen
}

beforeEach(() => resetUi())

describe("first-run tour", () => {
  it("shows the splash on a first visit, then the Welcome step", async () => {
    await openTour(WS)
    expect(title()).toBe("Welcome to LearnPuppy")
    expect(screen.getByTestId("tour-back")).toBeDisabled()
  })

  it("Next and Back move between steps", async () => {
    await openTour(WS)
    await userEvent.click(screen.getByTestId("tour-next"))
    expect(title()).toMatch(/Workspaces/)
    await userEvent.click(screen.getByTestId("tour-back"))
    expect(title()).toBe("Welcome to LearnPuppy")
  })

  it("arrow keys move and Escape skips, remembering it", async () => {
    await openTour(WS)
    await userEvent.keyboard("{ArrowRight}")
    expect(title()).toMatch(/Workspaces/)
    await userEvent.keyboard("{ArrowLeft}")
    expect(title()).toBe("Welcome to LearnPuppy")
    await userEvent.keyboard("{Escape}")
    await waitFor(() =>
      expect(screen.queryByTestId("tour-card")).not.toBeInTheDocument()
    )
    expect(localStorage.getItem(TOUR_KEY)).toBe("done")
  })

  it("Skip on the splash is remembered", async () => {
    renderApp(WS, { tour: true })
    await screen.findByTestId("tour-splash", {}, { timeout: 3000 })
    await userEvent.click(screen.getByTestId("tour-skip"))
    await waitFor(() =>
      expect(screen.queryByTestId("tour-splash")).not.toBeInTheDocument()
    )
    expect(localStorage.getItem(TOUR_KEY)).toBe("done")
  })

  it("inside a workspace it covers every sidebar section and ends with Finish", async () => {
    await openTour(WS)
    const all = (await walk()).join("|")
    expect(all).toMatch(/Graph.*Learn.*Review.*Digest.*Ingest/)
    expect(all).toMatch(/Accounts/)
    expect(all).not.toMatch(/What lives inside/)
    await userEvent.click(screen.getByTestId("tour-next"))
    await waitFor(() =>
      expect(screen.queryByTestId("tour-card")).not.toBeInTheDocument()
    )
    expect(localStorage.getItem(TOUR_KEY)).toBe("done")
  })

  it("does not reappear once done", async () => {
    localStorage.setItem(TOUR_KEY, "done")
    renderApp(WS, { tour: true })
    await screen.findByTestId("workspace-chip")
    await act(() => new Promise((r) => setTimeout(r, 800)))
    expect(screen.queryByTestId("tour-splash")).not.toBeInTheDocument()
    expect(screen.queryByTestId("tour-card")).not.toBeInTheDocument()
  })

  it("does not start with ?notour=1", async () => {
    window.history.pushState({}, "", "/?notour=1")
    try {
      renderApp(WS, { tour: true })
      await screen.findByTestId("workspace-chip")
      await act(() => new Promise((r) => setTimeout(r, 800)))
      expect(screen.queryByTestId("tour-splash")).not.toBeInTheDocument()
    } finally {
      window.history.pushState({}, "", "/")
    }
  })

  it("on the home screen it skips the sidebar steps and shows a text-only overview", async () => {
    await openTour("/")
    const all = (await walk()).join("|")
    expect(all).toMatch(/What lives inside a workspace/)
    expect(all).not.toMatch(/Graph: your knowledge map/)
    expect(all).not.toMatch(/Search and jump/)
    expect(all).toMatch(/You are ready/)
  })

  it("can be replayed from Settings", async () => {
    localStorage.setItem(TOUR_KEY, "done")
    renderApp("/settings", { tour: true })
    await userEvent.click(await screen.findByTestId("replay-tour"))
    await screen.findByTestId("tour-card")
    expect(title()).toBe("Welcome to LearnPuppy")
  })

  it("can be replayed from the Help menu", async () => {
    localStorage.setItem(TOUR_KEY, "done")
    renderApp(WS, { tour: true })
    await userEvent.click(await screen.findByTestId("help-menu"))
    await userEvent.click(await screen.findByTestId("help-replay-tour"))
    await screen.findByTestId("tour-card", {}, { timeout: 2000 })
  })
})
