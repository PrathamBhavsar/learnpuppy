import { screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, http } from "msw"
import { afterAll, beforeAll, beforeEach, describe, expect, it } from "vitest"

import { renderApp, resetUi } from "@/test/render"
import { state } from "@/test/msw/db"
import { buildLargeGraph } from "@/test/msw/fixtures/graph-large"
import { server } from "@/test/msw/server"
import { useGraphView } from "@/stores/graphView"

import { EMPTY_SELECTION } from "./model/selection"

import { PERFORMANCE_NODE_LIMIT } from "./use-graph-model"

const WS = "ws_rustasync"
const PATH = `/w/${WS}/graph`
const GRAPH_URL = `*/api/v1/workspaces/${WS}/graph`

// jsdom lays nothing out, so every element measures 0x0 and the virtualized rows would never render.
const proto = Element.prototype
const SIZE = { clientHeight: 600, clientWidth: 800 } as const
const originals = {
  clientHeight: Object.getOwnPropertyDescriptor(proto, "clientHeight"),
  clientWidth: Object.getOwnPropertyDescriptor(proto, "clientWidth"),
}
beforeAll(() => {
  for (const [key, value] of Object.entries(SIZE))
    Object.defineProperty(proto, key, { configurable: true, get: () => value })
})
afterAll(() => {
  for (const key of ["clientHeight", "clientWidth"] as const) {
    const original = originals[key]
    if (original) Object.defineProperty(proto, key, original)
  }
})

beforeEach(() => {
  resetUi()
  useGraphView.setState({
    wsId: null,
    selection: EMPTY_SELECTION,
    minDegree: 0,
    hulls: true,
    labels: "auto",
    minimap: true,
    followNew: false,
    table: false,
    tableHeight: 300,
    performance: "auto",
  })
})

const topNode = () => {
  const graph = state.db.graphs.get(WS)
  const nodes = [...(graph?.snapshot.nodes ?? [])]
  nodes.sort((a, b) => b.degree - a.degree || a.id.localeCompare(b.id))
  return nodes[0]!
}

async function openGraph(path = PATH) {
  const view = renderApp(path)
  await screen.findByTestId("screen-graph")
  await screen.findByTestId("graph-toolbar", undefined, { timeout: 4000 })
  return view
}

describe("graph screen", () => {
  it("loads the snapshot into the toolbar counts and the accessible list", async () => {
    await openGraph()
    expect(screen.getByTestId("graph-counts")).toHaveTextContent(
      "150 nodes · 201 chains"
    )
    const list = screen.getByRole("listbox", { name: /most connected first/i })
    const rows = within(list).getAllByTestId("graph-list-item")
    expect(rows.length).toBeGreaterThan(3)
    expect(rows[0]).toHaveTextContent(topNode().text)
    expect(rows[0]).toHaveAttribute("aria-setsize", "150")
    // jsdom has no 2D context: the canvas degrades to a notice and the list stays usable.
    expect(screen.getByTestId("graph-canvas-unsupported")).toBeInTheDocument()
  })

  it("selecting a list row selects the node, opens its drawer, announces it and shows the selection bar", async () => {
    const user = userEvent.setup()
    await openGraph()
    const first = screen.getAllByTestId("graph-list-item")[0]!
    await user.click(first)
    expect(first).toHaveAttribute("aria-selected", "true")
    expect(
      await screen.findByTestId("graph-selection-count")
    ).toHaveTextContent("1 selected")
    expect(screen.getByTestId("graph-live")).toHaveTextContent(
      `Selected: ${topNode().text}`
    )
    expect(await screen.findByTestId("graph-drawer")).toBeInTheDocument()

    await user.click(screen.getByTestId("graph-selection-clear"))
    await waitFor(() =>
      expect(
        screen.queryByTestId("graph-selection-bar")
      ).not.toBeInTheDocument()
    )
    await waitFor(() =>
      expect(screen.queryByTestId("graph-drawer")).not.toBeInTheDocument()
    )
    expect(screen.getByTestId("graph-live")).toHaveTextContent(
      "Selection cleared"
    )
  })

  it("Enter on the list opens the drawer with provenance, and the close button dismisses it", async () => {
    const user = userEvent.setup()
    const { router } = await openGraph()
    const list = screen.getByTestId("graph-list")
    list.focus()
    await user.keyboard("{Enter}")
    const drawer = await screen.findByTestId("graph-drawer")
    expect(within(drawer).getByTestId("graph-drawer-text")).toHaveTextContent(
      topNode().text
    )
    await waitFor(() =>
      expect(
        within(drawer).getByTestId("graph-drawer-source")
      ).toBeInTheDocument()
    )
    expect(router.state.location.search).toMatchObject({ node: topNode().id })

    await user.click(within(drawer).getByTestId("graph-drawer-close"))
    await waitFor(() =>
      expect(screen.queryByTestId("graph-drawer")).not.toBeInTheDocument()
    )
  })

  it("restores the open node from ?node= on a cold load", async () => {
    const id = topNode().id
    await openGraph(`${PATH}?node=${id}`)
    const drawer = await screen.findByTestId("graph-drawer")
    expect(within(drawer).getByTestId("graph-drawer-text")).toHaveTextContent(
      topNode().text
    )
  })

  it("search filters the list to matches and writes ?q=", async () => {
    const user = userEvent.setup()
    const { router } = await openGraph()
    await user.type(screen.getByTestId("graph-search"), "executor")
    await waitFor(() =>
      expect(router.state.location.search).toMatchObject({ q: "executor" })
    )
    await waitFor(() =>
      expect(screen.getByTestId("graph-search-count")).toHaveTextContent(
        /match/
      )
    )
    const rows = screen.getAllByTestId("graph-list-item")
    const total = Number(rows[0]!.getAttribute("aria-setsize"))
    expect(total).toBeGreaterThan(0)
    expect(total).toBeLessThan(150)
    for (const row of rows)
      expect(row.textContent?.toLowerCase()).toContain("executor")

    await user.click(screen.getByTestId("graph-search-clear"))
    await waitFor(() =>
      expect(router.state.location.search).not.toHaveProperty("q")
    )
    await waitFor(() =>
      expect(screen.getAllByTestId("graph-list-item")[0]).toHaveAttribute(
        "aria-setsize",
        "150"
      )
    )
  })

  it("delete previews the blast radius, requires acknowledgement, then removes the node", async () => {
    const user = userEvent.setup()
    const doomed = topNode().id
    await openGraph()
    await user.click(screen.getAllByTestId("graph-list-item")[0]!)
    await user.click(await screen.findByTestId("graph-selection-delete"))

    const dialog = await screen.findByTestId("graph-delete-dialog")
    expect(
      await within(dialog).findByTestId("graph-delete-summary")
    ).toHaveTextContent(/chains? lose/)
    expect(within(dialog).getByTestId("graph-delete-blast")).toBeInTheDocument()
    const confirm = within(dialog).getByTestId("graph-delete-confirm")
    expect(confirm).toBeDisabled()

    await user.click(within(dialog).getByTestId("graph-delete-ack"))
    expect(confirm).toBeEnabled()
    await user.click(confirm)

    await waitFor(() =>
      expect(
        screen.queryByTestId("graph-delete-dialog")
      ).not.toBeInTheDocument()
    )
    await waitFor(() =>
      expect(screen.getByTestId("graph-counts")).toHaveTextContent("149 nodes")
    )
    expect(
      state.db.graphs.get(WS)?.snapshot.nodes.some((n) => n.id === doomed)
    ).toBe(false)
  })

  it("cancelling the delete keeps the graph untouched", async () => {
    const user = userEvent.setup()
    await openGraph()
    await user.click(screen.getAllByTestId("graph-list-item")[0]!)
    await user.click(await screen.findByTestId("graph-selection-delete"))
    await user.click(await screen.findByTestId("graph-delete-cancel"))
    await waitFor(() =>
      expect(
        screen.queryByTestId("graph-delete-dialog")
      ).not.toBeInTheDocument()
    )
    expect(screen.getByTestId("graph-counts")).toHaveTextContent("150 nodes")
  })

  it("toggles the chain table and lists chains grouped by session", async () => {
    const user = userEvent.setup()
    await openGraph()
    await user.click(screen.getByTestId("graph-toggle-table"))
    const table = await screen.findByTestId("graph-table")
    expect(within(table).getByTestId("graph-table-scope")).toHaveTextContent(
      "All chains"
    )
    expect(
      await within(table).findByTestId("graph-table-tree")
    ).toBeInTheDocument()
    await user.click(screen.getByTestId("graph-table-close"))
    await waitFor(() =>
      expect(screen.queryByTestId("graph-table")).not.toBeInTheDocument()
    )
  })

  it("a source filter hides other sources from the list and clears again", async () => {
    const user = userEvent.setup()
    const { router } = await openGraph()
    await user.click(screen.getByTestId("graph-filter-source-git"))
    await waitFor(() =>
      expect(router.state.location.search).toHaveProperty("src")
    )
    const rows = screen.getAllByTestId("graph-list-item")
    expect(Number(rows[0]!.getAttribute("aria-setsize"))).toBeLessThan(150)
    expect(screen.getByTestId("graph-counts")).toHaveTextContent(/of 150 nodes/)

    await user.click(screen.getByTestId("graph-clear-filters"))
    await waitFor(() =>
      expect(screen.getByTestId("graph-counts")).toHaveTextContent(
        "150 nodes · 201 chains"
      )
    )
  })
})

describe("graph screen states", () => {
  it("shows the empty state with both calls to action when there are no nodes", async () => {
    server.use(
      http.get(GRAPH_URL, () =>
        HttpResponse.json({
          version: 1,
          nodes: [],
          chains: [],
          clusters: [],
          positions: {},
          layoutVersion: 1,
        })
      )
    )
    renderApp(PATH)
    expect(await screen.findByTestId("graph-empty-learn")).toBeInTheDocument()
    expect(screen.getByTestId("graph-empty-ingest")).toBeInTheDocument()
    expect(screen.queryByTestId("graph-toolbar")).not.toBeInTheDocument()
  })

  it("shows a retryable error when the snapshot fails to load", async () => {
    let calls = 0
    server.use(
      http.get(GRAPH_URL, () => {
        calls += 1
        return HttpResponse.json(
          { error: { code: "internal", message: "boom", requestId: "req_9" } },
          { status: 500 }
        )
      })
    )
    const user = userEvent.setup()
    renderApp(PATH)
    expect(
      await screen.findByText("Could not load the graph", undefined, {
        timeout: 4000,
      })
    ).toBeInTheDocument()
    const before = calls
    await user.click(screen.getByRole("button", { name: /retry|try again/i }))
    await waitFor(() => expect(calls).toBeGreaterThan(before))
  })

  it("switches to performance mode above the node limit, and the chip can turn it off", async () => {
    const user = userEvent.setup()
    state.db.graphs.set(
      WS,
      buildLargeGraph(WS, PERFORMANCE_NODE_LIMIT + 1, state.db.now, 5)
    )
    renderApp(PATH)
    const chip = await screen.findByTestId(
      "graph-performance-chip",
      undefined,
      { timeout: 8000 }
    )
    expect(chip).toHaveTextContent(/performance mode/i)
    expect(useGraphView.getState().performance).toBe("auto")

    await user.click(screen.getByTestId("graph-performance-toggle"))
    expect(useGraphView.getState().performance).toBe("off")
  }, 20_000)

  it("does not show the performance chip at the limit", async () => {
    state.db.graphs.set(
      WS,
      buildLargeGraph(WS, PERFORMANCE_NODE_LIMIT, state.db.now, 5)
    )
    renderApp(PATH)
    await screen.findByTestId("graph-toolbar", undefined, { timeout: 8000 })
    expect(
      screen.queryByTestId("graph-performance-chip")
    ).not.toBeInTheDocument()
  }, 20_000)
})
