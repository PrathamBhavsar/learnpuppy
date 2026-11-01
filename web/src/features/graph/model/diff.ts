// What changed between two scenes: which nodes are new (birth animation) and which moved (layout tween).
import type { Scene } from "./scene"

/** Above this node count a layout change snaps instead of tweening (docs/web/05 #16). */
export const TWEEN_NODE_LIMIT = 5000

/** Positions that differ by less than this (world units) count as unmoved. */
const MOVE_EPS = 0.5

export type SceneDiff = {
  /** Indices (in `next`) of nodes absent from `prev`. */
  born: number[]
  /** Indices (in `next`) of nodes whose position changed. */
  moved: number[]
  /** Starting positions for every node of `next`: the old spot if it had one, else its new spot. */
  fromX: Float32Array
  fromY: Float32Array
  layoutChanged: boolean
}

export function diffScenes(prev: Scene | null, next: Scene): SceneDiff {
  const fromX = Float32Array.from(next.x)
  const fromY = Float32Array.from(next.y)
  const born: number[] = []
  const moved: number[] = []
  if (!prev) {
    return { born, moved, fromX, fromY, layoutChanged: false }
  }
  for (let i = 0; i < next.count; i++) {
    const j = prev.index.get(next.ids[i] as string)
    if (j === undefined) {
      born.push(i)
      continue
    }
    fromX[i] = prev.x[j] as number
    fromY[i] = prev.y[j] as number
    if (
      Math.abs((prev.x[j] as number) - (next.x[i] as number)) > MOVE_EPS ||
      Math.abs((prev.y[j] as number) - (next.y[i] as number)) > MOVE_EPS
    )
      moved.push(i)
  }
  return {
    born,
    moved,
    fromX,
    fromY,
    layoutChanged:
      next.layoutVersion !== prev.layoutVersion && moved.length > 0,
  }
}

/** Layout tweens run for a new layout version on small graphs; big ones snap. */
export function shouldTween(diff: SceneDiff, count: number): boolean {
  return diff.layoutChanged && count <= TWEEN_NODE_LIMIT
}
