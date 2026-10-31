import { describe, expect, it } from "vitest"

import { type Point, centroid, convexHull, paddedHull } from "./hull"

const area = (poly: readonly Point[]): number => {
  let sum = 0
  poly.forEach((p, i) => {
    const q = poly[(i + 1) % poly.length] as Point
    sum += p[0] * q[1] - q[0] * p[1]
  })
  return sum / 2
}

describe("convexHull", () => {
  it("returns small inputs unchanged", () => {
    expect(convexHull([])).toEqual([])
    expect(convexHull([[1, 2]])).toEqual([[1, 2]])
    expect(convexHull([[0, 0], [1, 1]])).toEqual([[0, 0], [1, 1]])
  })

  it("drops interior and collinear points", () => {
    const hull = convexHull([[0, 0], [4, 0], [4, 4], [0, 4], [2, 2], [2, 0], [1, 3]])
    expect(hull).toHaveLength(4)
    expect(area(hull)).toBe(16)
  })

  it("is counter-clockwise so the signed area is positive", () => {
    expect(area(convexHull([[0, 0], [5, 1], [3, 6], [-2, 4], [1, 2]]))).toBeGreaterThan(0)
  })
})

describe("paddedHull", () => {
  it("is empty for no points and grows a hull outward by the padding", () => {
    expect(paddedHull([], 5)).toEqual([])
    const grown = paddedHull([[0, 0], [10, 0], [10, 10], [0, 10]], 2)
    expect(area(grown)).toBe(14 * 14)
    for (const [x, y] of grown) {
      expect(x === -2 || x === 12).toBe(true)
      expect(y === -2 || y === 12).toBe(true)
    }
  })

  it("keeps every original point inside the grown hull", () => {
    const pts: Point[] = [[0, 0], [7, 3], [2, 9], [5, 5], [-3, 4]]
    const grown = paddedHull(pts, 1.5)
    const xs = grown.map((p) => p[0])
    const ys = grown.map((p) => p[1])
    for (const [x, y] of pts) {
      expect(x).toBeGreaterThan(Math.min(...xs))
      expect(x).toBeLessThan(Math.max(...xs))
      expect(y).toBeGreaterThan(Math.min(...ys))
      expect(y).toBeLessThan(Math.max(...ys))
    }
  })
})

describe("centroid", () => {
  it("averages the points and defaults to the origin", () => {
    expect(centroid([])).toEqual([0, 0])
    expect(centroid([[0, 0], [4, 2], [2, 4]])).toEqual([2, 2])
  })
})
