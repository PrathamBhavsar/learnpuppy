import { act, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it } from "vitest"

import { renderApp, resetUi } from "@/test/render"
import { useUi } from "@/stores/ui"

import { buildCrumbs } from "./Topbar"
import { WORKSPACES } from "@/lib/static-workspaces"

beforeEach(resetUi)

async function openShell(path = "/w/learnpuppy/graph") {
  const view = renderApp(path)
  await screen.findByTestId("sidebar")
  return view
}

describe("shell", () => {
  it("renders sidebar, topbar and the section nav with badges", async () => {
    await openShell()
    for (const key of ["graph", "learn", "review", "digest", "ingest"]) {
      expect(screen.getByTestId(`nav-${key}`)).toBeInTheDocument()
    }
    expect(screen.getByTestId("nav-review-badge")).toHaveTextContent("3")
    expect(screen.getByTestId("nav-graph")).toHaveAttribute(
      "aria-current",
      "page"
    )
    expect(screen.getByTestId("nav-accounts")).toBeInTheDocument()
    expect(screen.getByTestId("nav-settings")).toBeInTheDocument()
    const topbar = screen.getByTestId("topbar")
    expect(within(topbar).getByTestId("search-trigger")).toBeInTheDocument()
    expect(within(topbar).getByTestId("jobs-trigger")).toBeInTheDocument()
    expect(within(topbar).getByTestId("accounts-pill")).toHaveTextContent(
      "2/3 ready"
    )
    expect(within(topbar).getByTestId("theme-toggle")).toBeInTheDocument()
  })

  it("collapses with the [ key, labels disappear, and the choice persists", async () => {
    const user = userEvent.setup()
    await openShell()
    expect(screen.getByTestId("sidebar")).toHaveAttribute(
      "data-collapsed",
      "false"
    )
    expect(screen.getByTestId("nav-learn-badge")).toBeInTheDocument()

    await user.keyboard("[[")
    expect(screen.getByTestId("sidebar")).toHaveAttribute(
      "data-collapsed",
      "true"
    )
    expect(screen.queryByTestId("nav-learn-badge")).not.toBeInTheDocument()
    expect(screen.getByTestId("nav-learn")).toHaveAccessibleName("Learn")
    expect(localStorage.getItem("lp-sidebar-collapsed")).toBe("1")

    await user.keyboard("[[")
    expect(screen.getByTestId("sidebar")).toHaveAttribute(
      "data-collapsed",
      "false"
    )
  })

  it("collapses with the footer button and ignores [ while typing in the palette", async () => {
    const user = userEvent.setup()
    await openShell()
    await user.click(screen.getByTestId("sidebar-toggle"))
    expect(useUi.getState().sidebarCollapsed).toBe(true)
    expect(screen.getByTestId("sidebar-toggle")).toHaveAccessibleName(
      "Expand sidebar"
    )

    await user.click(screen.getByTestId("sidebar-toggle"))
    await user.click(screen.getByTestId("search-trigger"))
    await user.type(await screen.findByTestId("palette-input"), "[[")
    expect(useUi.getState().sidebarCollapsed).toBe(false)
  })

  it("starts collapsed when the stored preference says so", async () => {
    localStorage.setItem("lp-sidebar-collapsed", "1")
    useUi.setState({ sidebarCollapsed: true })
    await openShell()
    expect(screen.getByTestId("sidebar")).toHaveAttribute(
      "data-collapsed",
      "true"
    )
  })

  it("navigates between sections and remembers the workspace", async () => {
    const user = userEvent.setup()
    const { router } = await openShell("/w/papers/graph")
    await user.click(screen.getByTestId("nav-review"))
    await waitFor(() =>
      expect(router.state.location.pathname).toBe("/w/papers/review")
    )
    await user.click(screen.getByTestId("nav-accounts"))
    await waitFor(() =>
      expect(router.state.location.pathname).toBe("/accounts")
    )
    expect(useUi.getState().lastWorkspace).toBe("papers")
    // global routes keep the last workspace in the sidebar
    expect(screen.getByTestId("workspace-chip")).toHaveAccessibleName(
      "Workspace: ML papers"
    )
  })

  it("switches workspace from the chip popover", async () => {
    const user = userEvent.setup()
    const { router } = await openShell()
    await user.click(screen.getByTestId("workspace-chip"))
    await user.type(await screen.findByTestId("workspace-search"), "rust")
    expect(
      screen.queryByTestId("workspace-option-papers")
    ).not.toBeInTheDocument()
    await user.click(screen.getByTestId("workspace-option-rust-systems"))
    await waitFor(() =>
      expect(router.state.location.pathname).toBe("/w/rust-systems/graph")
    )
  })

  it("moves focus to the page heading and announces the title after navigation", async () => {
    const user = userEvent.setup()
    await openShell()
    await user.click(screen.getByTestId("nav-digest"))
    await waitFor(() =>
      expect(
        screen.getByRole("heading", { level: 1, name: "Digest" })
      ).toHaveFocus()
    )
    expect(screen.getByTestId("route-announcer")).toHaveTextContent(
      "Digest · LearnPuppy"
    )
  })

  it("opens the command palette with Ctrl/Cmd+K and navigates from it", async () => {
    const user = userEvent.setup()
    const { router } = await openShell()
    await act(async () => {
      window.dispatchEvent(
        new KeyboardEvent("keydown", { key: "k", metaKey: true, ctrlKey: true })
      )
    })
    await user.click(await screen.findByTestId("palette-go-ingest"))
    await waitFor(() =>
      expect(router.state.location.pathname).toBe("/w/learnpuppy/ingest")
    )
  })
})

describe("buildCrumbs", () => {
  const ws = WORKSPACES[0]!
  it("builds Workspace / Section / Item", () => {
    expect(
      buildCrumbs("/w/learnpuppy/learn/abc", ws).map((c) => c.label)
    ).toEqual(["LearnPuppy", "Learn", "abc"])
  })
  it("handles global routes", () => {
    expect(buildCrumbs("/accounts", ws).map((c) => c.label)).toEqual([
      "Accounts",
    ])
    expect(buildCrumbs("/", ws)).toEqual([])
  })
})
