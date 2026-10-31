// Keyboard navigation across the canvas: arrows jump to the nearest neighbour in that direction and
// Tab walks the degree-sorted order. Pure functions over the Scene so they are tested without a DOM.
import type { Scene } from "./scene"

export type Direction = "left" | "right" | "up" | "down"

const VECTORS: Record<Direction, readonly [number, number]> = {
  left: [-1, 0],
  right: [1, 0],
  up: [0, -1],
  down: [0, 1],
}

/** Candidates must lie within this angle of the requested direction (45° each side, widening to 80°). */
const CONES = [Math.PI / 4, (80 * Math.PI) / 180] as const

/**
 * The visible node nearest to `from` in `dir`. Distance is weighted by how far off-axis the candidate
 * is, so ArrowRight prefers the node straight right over a closer one drifting diagonally.
 * Returns -1 when nothing lies that way.
 */
export function nearestInDirection(
  scene: Scene,
  visible: Uint8Array | null,
  from: number,
  dir: Direction
): number {
  const [dx, dy] = VECTORS[dir]
  const fx = scene.x[from] as number
  const fy = scene.y[from] as number
  for (const cone of CONES) {
    const minCos = Math.cos(cone)
    let best = -1
    let bestScore = Infinity
    for (let i = 0; i < scene.count; i++) {
      if (i === from || (visible && !visible[i])) continue
      const vx = (scene.x[i] as number) - fx
      const vy = (scene.y[i] as number) - fy
      const dist = Math.hypot(vx, vy)
      if (dist === 0) continue
      const cos = (vx * dx + vy * dy) / dist
      if (cos < minCos) continue
      const score = dist * (2 - cos)
      if (score < bestScore) {
        bestScore = score
        best = i
      }
    }
    if (best >= 0) return best
  }
  return -1
}

/** Visible nodes in Tab order (degree desc, then id). */
export function tabOrder(scene: Scene, visible: Uint8Array | null): Uint32Array {
  if (!visible) return scene.order
  return scene.order.filter((i) => visible[i] === 1)
}

/**
 * Next index in `order` after `current` (or before, with `reverse`). Returns null at the ends so the
 * caller can let Tab leave the canvas instead of trapping focus; -1 `current` enters at the start/end.
 */
export function stepTab(
  order: Uint32Array,
  current: number,
  reverse: boolean
): number | null {
  if (order.length === 0) return null
  const at = current < 0 ? -1 : order.indexOf(current)
  if (reverse) {
    if (at === -1) return order[order.length - 1] as number
    return at === 0 ? null : (order[at - 1] as number)
  }
  if (at === -1) return order[0] as number
  return at === order.length - 1 ? null : (order[at + 1] as number)
}

/** The visible node closest to a world point (used to start keyboard navigation near the view center). */
export function nearestToPoint(
  scene: Scene,
  visible: Uint8Array | null,
  wx: number,
  wy: number
): number {
  let best = -1
  let bestDist = Infinity
  for (let i = 0; i < scene.count; i++) {
    if (visible && !visible[i]) continue
    const d = Math.hypot((scene.x[i] as number) - wx, (scene.y[i] as number) - wy)
    if (d < bestDist) {
      bestDist = d
      best = i
    }
  }
  return best
}
