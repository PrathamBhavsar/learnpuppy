import { describe, expect, it } from "vitest"

import { buildHitIndex, hitTest, nodesInRect } from "./hit"

const x = Float32Array.from([0, 50, 52, 200, 400])
const y = Float32Array.from([0, 0, 0, 100, 400])
const half = () => 5

describe("hitTest", () => {
  const index = buildHitIndex(x, y, 5, null)

  it("finds the node whose square contains the point", () => {
    expect(hitTest(index, x, y, half, 5, 3, -2, 0)).toBe(0)
    expect(hitTest(index, x, y, half, 5, 200, 104, 0)).toBe(3)
  })

  it("misses empty space", () => {
    expect(hitTest(index, x, y, half, 5, 120, 120, 0)).toBe(-1)
  })

  it("uses the tolerance around the edge", () => {
    expect(hitTest(index, x, y, half, 5, 7, 0, 0)).toBe(-1)
    expect(hitTest(index, x, y, half, 5, 7, 0, 3)).toBe(0)
  })

  it("picks the closest centre when squares overlap", () => {
    expect(hitTest(index, x, y, half, 5, 51.4, 0, 0)).toBe(2)
    expect(hitTest(index, x, y, half, 5, 50.6, 0, 0)).toBe(1)
  })

  it("respects per-node sizes", () => {
    const big = (i: number) => (i === 3 ? 40 : 5)
    expect(hitTest(index, x, y, big, 40, 230, 100, 0)).toBe(3)
    expect(hitTest(index, x, y, half, 5, 230, 100, 0)).toBe(-1)
  })

  it("ignores nodes excluded from the index (filtered out)", () => {
    const visible = Uint8Array.from([0, 1, 1, 1, 1])
    const filtered = buildHitIndex(x, y, 5, visible)
    expect(filtered.size).toBe(4)
    expect(hitTest(filtered, x, y, half, 5, 0, 0, 3)).toBe(-1)
  })
})

describe("nodesInRect", () => {
  const index = buildHitIndex(x, y, 5, null)

  it("returns nodes inside the rectangle, whichever way it was dragged", () => {
    expect(nodesInRect(index, x, y, { x0: -10, y0: -10, x1: 60, y1: 10 }).sort()).toEqual([0, 1, 2])
    expect(nodesInRect(index, x, y, { x0: 60, y0: 10, x1: -10, y1: -10 }).sort()).toEqual([0, 1, 2])
  })

  it("returns nothing for an empty region", () => {
    expect(nodesInRect(index, x, y, { x0: 100, y0: 200, x1: 150, y1: 250 })).toEqual([])
  })

  it("handles many coincident points", () => {
    const n = 500
    const cx = new Float32Array(n).fill(10)
    const cy = new Float32Array(n).fill(10)
    const dense = buildHitIndex(cx, cy, n, null)
    expect(nodesInRect(dense, cx, cy, { x0: 0, y0: 0, x1: 20, y1: 20 })).toHaveLength(n)
  })
})
