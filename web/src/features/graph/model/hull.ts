// Convex hulls for cluster silhouettes, padded outward so nodes sit inside with breathing room.

export type Point = readonly [number, number]

const cross = (o: Point, a: Point, b: Point): number =>
  (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0])

/** Andrew's monotone chain. Returns the hull counter-clockwise (in a y-up frame) without repeats. */
export function convexHull(points: readonly Point[]): Point[] {
  if (points.length < 3) return [...points]
  const sorted = [...points].sort((a, b) => a[0] - b[0] || a[1] - b[1])
  const lower: Point[] = []
  for (const p of sorted) {
    while (lower.length >= 2 && cross(lower[lower.length - 2] as Point, lower[lower.length - 1] as Point, p) <= 0)
      lower.pop()
    lower.push(p)
  }
  const upper: Point[] = []
  for (let i = sorted.length - 1; i >= 0; i--) {
    const p = sorted[i] as Point
    while (upper.length >= 2 && cross(upper[upper.length - 2] as Point, upper[upper.length - 1] as Point, p) <= 0)
      upper.pop()
    upper.push(p)
  }
  lower.pop()
  upper.pop()
  return [...lower, ...upper]
}

/** Hull of `points` grown by `pad`: only the raw hull's vertices are expanded, which keeps big clusters cheap. */
export function paddedHull(points: readonly Point[], pad: number): Point[] {
  if (points.length === 0) return []
  const grown: Point[] = []
  for (const [x, y] of convexHull(points)) {
    grown.push([x - pad, y - pad], [x + pad, y - pad], [x + pad, y + pad], [x - pad, y + pad])
  }
  return convexHull(grown)
}

export function centroid(points: readonly Point[]): Point {
  if (points.length === 0) return [0, 0]
  let sx = 0
  let sy = 0
  for (const [x, y] of points) {
    sx += x
    sy += y
  }
  return [sx / points.length, sy / points.length]
}
