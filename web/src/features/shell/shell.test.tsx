import { act, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { beforeEach, describe, expect, it } from "vitest"

import { BANNER_DELAY_MS } from "./ConnectionBanner"
import { useConnection } from "@/stores/connection"
import { useStatus } from "@/stores/status"
import { useUi } from "@/stores/ui"
import { renderApp, resetUi } from "@/test/render"

import { buildCrumbs } from "./Topbar"

beforeEach(resetUi)

const RUST = "ws_rustasync"
const DIST = "ws_distsys"

async function openShell(path = `/w/${RUST}/graph`, name = "Rust async") {
  const view = renderApp(path)
  await screen.findByTestId("sidebar")
  await screen.findByRole("button", { name: `Workspace: ${name}` })
  return view
}

describe("shell", () => {
  it("renders sidebar, topbar and section nav with badges from the API", async () => {
    await openShell()
    for (const key of ["graph", "learn", "review", "digest", "ingest"]) {
      expect(screen.getByTestId(`nav-${key}`)).toBeInTheDocument()
    }
    await waitFor(() =>
      expect(screen.getByTestId("nav-review-badge")).toHaveTextContent("7")
    )
    expect(screen.getByTestId("nav-graph")).toHaveAttribute(
      "aria-current",
      "page"
    )
    expect(screen.getByTestId("nav-accounts")).toBeInTheDocument()
    expect(screen.getByTestId("nav-settings")).toBeInTheDocument()
    const topbar = screen.getByTestId("topbar")
    expect(within(topbar).getByTestId("search-trigger")).toBeInTheDocument()
    expect(within(topbar).getByTestId("jobs-trigger")).toBeInTheDocument()
    expect(
      await within(topbar).findByTestId("accounts-pill")
    ).toHaveTextContent("2/4 ready")
    expect(within(topbar).getByTestId("theme-toggle")).toBeInTheDocument()
  })

  it("shows the accounts pill amber with a cooldown countdown", async () => {
    await openShell()
    const pill = await screen.findByTestId("accounts-pill")
    expect(pill).toHaveAttribute("data-state", "partial")
    expect(await screen.findByTestId("accounts-countdown")).toHaveTextContent(
      /\d+s/
    )
  })

  it("collapses with the [ key, labels disappear, and the choice persists", async () => {
    const user = userEvent.setup()
    await openShell()
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
    const { router } = await openShell(
      `/w/${DIST}/graph`,
      "Distributed systems"
    )
    await user.click(screen.getByTestId("nav-review"))
    await waitFor(() =>
      expect(router.state.location.pathname).toBe(`/w/${DIST}/review`)
    )
    await user.click(screen.getByTestId("nav-accounts"))
    await waitFor(() =>
      expect(router.state.location.pathname).toBe("/accounts")
    )
    expect(useUi.getState().lastWorkspace).toBe(DIST)
    // global routes keep the last workspace in the sidebar
    expect(screen.getByTestId("workspace-chip")).toHaveAccessibleName(
      "Workspace: Distributed systems"
    )
  })

  it("switches workspace from the chip popover, keeping the current section", async () => {
    const user = userEvent.setup()
    const { router } = await openShell(`/w/${RUST}/review`)
    await user.click(screen.getByTestId("workspace-chip"))
    await user.type(await screen.findByTestId("workspace-search"), "distri")
    expect(
      screen.queryByTestId(`workspace-option-${RUST}`)
    ).not.toBeInTheDocument()
    await user.click(screen.getByTestId(`workspace-option-${DIST}`))
    await waitFor(() =>
      expect(router.state.location.pathname).toBe(`/w/${DIST}/review`)
    )
  })

  it("lists live workspaces with due badges and hides archived ones", async () => {
    const user = userEvent.setup()
    await openShell()
    await user.click(screen.getByTestId("workspace-chip"))
    const popover = await screen.findByTestId("workspace-popover")
    expect(
      await within(popover).findByTestId(`workspace-option-${RUST}`)
    ).toHaveTextContent("7")
    expect(
      within(popover).queryByTestId("workspace-option-ws_oldnotes")
    ).not.toBeInTheDocument()
  })

  it("shows a not-found state for an unknown workspace id", async () => {
    renderApp("/w/nope/graph")
    expect(await screen.findByText("Workspace not found")).toBeInTheDocument()
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
    await waitFor(() =>
      expect(screen.getByTestId("route-announcer")).toHaveTextContent(
        "Digest · Rust async · LearnPuppy"
      )
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
    await user.click(await screen.findByTestId("palette-cmd-go-ingest"))
    await waitFor(() =>
      expect(router.state.location.pathname).toBe(`/w/${RUST}/ingest`)
    )
  })
})

describe("global states", () => {
  it("shows the full-screen auth state when the session is rejected", async () => {
    await openShell()
    act(() => useStatus.getState().setAuthRequired(true))
    expect(await screen.findByTestId("auth-required")).toHaveTextContent(
      "learnpuppy serve"
    )
  })

  it("shows the engine banner while no account is ready", async () => {
    await openShell()
    act(() => useStatus.getState().setEngineUnavailable(true))
    // Mock accounts include a ready one, so the banner clears itself.
    await waitFor(() =>
      expect(screen.queryByTestId("engine-banner")).not.toBeInTheDocument()
    )
  })

  it("shows the reconnect banner only after the stream was down for 2 s", async () => {
    await openShell()
    expect(screen.queryByTestId("connection-banner")).not.toBeInTheDocument()
    act(() =>
      useConnection
        .getState()
        .set({ status: "reconnecting", downSince: Date.now(), attempt: 1 })
    )
    expect(screen.queryByTestId("connection-banner")).not.toBeInTheDocument()
    expect(
      await screen.findByTestId(
        "connection-banner",
        {},
        { timeout: BANNER_DELAY_MS + 1500 }
      )
    ).toHaveTextContent("Reconnecting… your work is safe.")
    act(() =>
      useConnection
        .getState()
        .set({ status: "open", downSince: null, recoveredAt: Date.now() })
    )
    expect(await screen.findByText("Back online.")).toBeInTheDocument()
  }, 10_000)
})

describe("buildCrumbs", () => {
  const ws = { id: "learnpuppy", name: "LearnPuppy" }
  it("builds Workspace / Section / Item", () => {
    expect(
      buildCrumbs("/w/learnpuppy/learn/abc", ws).map((c) => c.label)
    ).toEqual(["LearnPuppy", "Learn", "abc"])
  })
  it("falls back to the id while the workspace list loads", () => {
    expect(
      buildCrumbs("/w/learnpuppy/review", undefined).map((c) => c.label)
    ).toEqual(["learnpuppy", "Review"])
  })
  it("handles global routes", () => {
    expect(buildCrumbs("/accounts", ws).map((c) => c.label)).toEqual([
      "Accounts",
    ])
    expect(buildCrumbs("/", ws)).toEqual([])
  })
})
