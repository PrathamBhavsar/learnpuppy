import { beforeEach, describe, expect, it, vi } from "vitest"

import { selectOnly } from "@/features/graph/model/selection"

const KEY = "lp-graph-prefs"

async function freshStore() {
  vi.resetModules()
  return (await import("./graphView")).useGraphView
}

beforeEach(() => localStorage.clear())

describe("graph view store", () => {
  it("starts from the defaults when nothing is stored", async () => {
    const s = (await freshStore()).getState()
    expect(s).toMatchObject({
      hulls: true,
      labels: "auto",
      minimap: true,
      followNew: false,
      table: false,
      tableHeight: 300,
      performance: "auto",
    })
  })

  it("persists preferences and restores them on the next load", async () => {
    const first = await freshStore()
    first.getState().setPref("hulls", false)
    first.getState().setPref("labels", "never")
    first.getState().setPref("table", true)
    expect(JSON.parse(localStorage.getItem(KEY) ?? "{}")).toMatchObject({
      hulls: false,
      labels: "never",
      table: true,
    })

    const second = (await freshStore()).getState()
    expect(second).toMatchObject({ hulls: false, labels: "never", table: true })
  })

  it("ignores corrupt or out-of-range stored values field by field", async () => {
    localStorage.setItem(
      KEY,
      JSON.stringify({ hulls: "yes", labels: "bold", tableHeight: 99_999, performance: "sometimes", minimap: false })
    )
    const s = (await freshStore()).getState()
    expect(s.hulls).toBe(true)
    expect(s.labels).toBe("auto")
    expect(s.performance).toBe("auto")
    expect(s.minimap).toBe(false)
    expect(s.tableHeight).toBe(640)
  })

  it("survives unparseable storage", async () => {
    localStorage.setItem(KEY, "{not json")
    expect((await freshStore()).getState().hulls).toBe(true)
  })

  it("clamps a stored table height up to the minimum", async () => {
    localStorage.setItem(KEY, JSON.stringify({ tableHeight: 10 }))
    expect((await freshStore()).getState().tableHeight).toBe(160)
  })

  it("resets selection and filters when the workspace changes, but not on a re-bind", async () => {
    const store = await freshStore()
    store.getState().bind("ws_a")
    store.getState().setSelection(selectOnly("n1"))
    store.getState().setMinDegree(3)
    store.getState().bind("ws_a")
    expect(store.getState().selection.selected).toEqual(["n1"])
    expect(store.getState().minDegree).toBe(3)

    store.getState().bind("ws_b")
    expect(store.getState().selection.selected).toEqual([])
    expect(store.getState().minDegree).toBe(0)
  })

  it("clearSelection is a no-op on an empty selection and empties a populated one", async () => {
    const store = await freshStore()
    const before = store.getState().selection
    store.getState().clearSelection()
    expect(store.getState().selection).toBe(before)
    store.getState().setSelection(selectOnly("n1"))
    store.getState().clearSelection()
    expect(store.getState().selection.selected).toEqual([])
  })

  it("remembers a camera per workspace", async () => {
    const store = await freshStore()
    const cam = { x: 1, y: 2, k: 0.5, w: 800, h: 600, layoutVersion: 3 }
    store.getState().saveCamera("ws_a", cam)
    expect(store.getState().cameras["ws_a"]).toEqual(cam)
  })
})
