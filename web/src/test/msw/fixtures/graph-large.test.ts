import { describe, expect, it } from "vitest"

import { LARGE_GRAPH_SIZES, buildLargeGraph } from "./graph-large"

const NOW = 1_800_000_000_000

describe("buildLargeGraph", () => {
  it.each(Object.entries(LARGE_GRAPH_SIZES))("builds the %s fixture with consistent references", (_name, size) => {
    const { snapshot, details } = buildLargeGraph("ws_perf", size, NOW, 7)
    expect(snapshot.nodes).toHaveLength(size)
    expect(Object.keys(snapshot.positions ?? {})).toHaveLength(size)
    expect(snapshot.chains.length).toBeGreaterThan(size)

    const ids = new Set(snapshot.nodes.map((n) => n.id))
    expect(ids.size).toBe(size)
    for (const chain of snapshot.chains) {
      expect(ids.has(chain.from)).toBe(true)
      expect(ids.has(chain.to)).toBe(true)
      expect(chain.from).not.toBe(chain.to)
    }
    const clustered = snapshot.clusters.reduce((sum, c) => sum + c.nodeIds.length, 0)
    expect(clustered).toBe(size)
    expect(snapshot.nodes.some((n) => n.degree > 0)).toBe(true)
    expect(details.get(snapshot.nodes[0]?.id as string)?.id).toBe(snapshot.nodes[0]?.id)
  })

  it("is deterministic for a seed and different across seeds", () => {
    const a = buildLargeGraph("ws_perf", 500, NOW, 3)
    const b = buildLargeGraph("ws_perf", 500, NOW, 3)
    const c = buildLargeGraph("ws_perf", 500, NOW, 4)
    expect(b.snapshot.nodes).toEqual(a.snapshot.nodes)
    expect(b.snapshot.chains).toEqual(a.snapshot.chains)
    expect(c.snapshot.nodes.map((n) => n.text)).not.toEqual(a.snapshot.nodes.map((n) => n.text))
  })
})
