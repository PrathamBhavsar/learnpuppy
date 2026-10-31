// Spatial index over node positions (d3-quadtree) for hover, click and marquee queries.
import { type Quadtree, quadtree } from "d3-quadtree"

export type HitIndex = {
  tree: Quadtree<number>
  /** Number of indexed (visible) nodes. */
  size: number
}

/** Indexes the nodes `visible` allows (all when null) at their current positions. */
export function buildHitIndex(
  x: Float32Array,
  y: Float32Array,
  count: number,
  visible: Uint8Array | null
): HitIndex {
  const items: number[] = []
  for (let i = 0; i < count; i++) if (!visible || visible[i]) items.push(i)
  const tree = quadtree<number>()
    .x((i) => x[i]!)
    .y((i) => y[i]!)
    .addAll(items)
  return { tree, size: items.length }
}

/**
 * The node whose square contains world point (wx, wy), within `tolWorld` of its edge. `halfWorld(i)`
 * gives a node's half-side in world units (it varies with zoom and degree). The closest centre wins.
 * `maxHalfWorld` bounds the search window so the quadtree can prune.
 */
export function hitTest(
  index: HitIndex,
  x: Float32Array,
  y: Float32Array,
  halfWorld: (i: number) => number,
  maxHalfWorld: number,
  wx: number,
  wy: number,
  tolWorld: number
): number {
  let best = -1
  let bestDist = Infinity
  const reach = maxHalfWorld + tolWorld
  index.tree.visit((node, x0, y0, x1, y1) => {
    if (wx < x0 - reach || wx > x1 + reach || wy < y0 - reach || wy > y1 + reach)
      return true
    if (!node.length) {
      let leaf: typeof node | undefined = node
      while (leaf) {
        const i = (leaf as { data: number }).data
        const dx = Math.abs(wx - x[i]!)
        const dy = Math.abs(wy - y[i]!)
        const lim = halfWorld(i) + tolWorld
        if (dx <= lim && dy <= lim) {
          const d = dx * dx + dy * dy
          if (d < bestDist) {
            bestDist = d
            best = i
          }
        }
        leaf = (leaf as { next?: typeof node }).next
      }
    }
    return false
  })
  return best
}

export type WorldRect = { x0: number; y0: number; x1: number; y1: number }

/** Nodes whose centre lies inside the (normalised) world rectangle. */
export function nodesInRect(
  index: HitIndex,
  x: Float32Array,
  y: Float32Array,
  rect: WorldRect
): number[] {
  const x0 = Math.min(rect.x0, rect.x1)
  const x1 = Math.max(rect.x0, rect.x1)
  const y0 = Math.min(rect.y0, rect.y1)
  const y1 = Math.max(rect.y0, rect.y1)
  const found: number[] = []
  index.tree.visit((node, qx0, qy0, qx1, qy1) => {
    if (qx0 > x1 || qx1 < x0 || qy0 > y1 || qy1 < y0) return true
    if (!node.length) {
      let leaf: typeof node | undefined = node
      while (leaf) {
        const i = (leaf as { data: number }).data
        if (x[i]! >= x0 && x[i]! <= x1 && y[i]! >= y0 && y[i]! <= y1) found.push(i)
        leaf = (leaf as { next?: typeof node }).next
      }
    }
    return false
  })
  return found
}
