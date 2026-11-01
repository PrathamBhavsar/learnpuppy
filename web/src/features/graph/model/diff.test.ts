import { describe, expect, it } from "vitest"

import { TWEEN_NODE_LIMIT, diffScenes, shouldTween } from "./diff"
import { sceneOf } from "./test-snapshot"

const base = [
  { id: "a", x: 0, y: 0 },
  { id: "b", x: 100, y: 0 },
]

describe("diffScenes", () => {
  it("has nothing to diff on first load", () => {
    const next = sceneOf(base)
    const d = diffScenes(null, next)
    expect(d.born).toEqual([])
    expect(d.layoutChanged).toBe(false)
  })

  it("finds born nodes and starts them at their own position", () => {
    const prev = sceneOf(base)
    const next = sceneOf([...base, { id: "c", x: 50, y: 50 }])
    const d = diffScenes(prev, next)
    expect(d.born).toEqual([2])
    expect(d.fromX[2]).toBe(50)
  })

  it("reports moved nodes only for a new layout version, starting from old positions", () => {
    const prev = sceneOf(base, [], { layoutVersion: 1 })
    const next = sceneOf(
      [
        { id: "a", x: 30, y: 0 },
        { id: "b", x: 100, y: 0 },
      ],
      [],
      { layoutVersion: 2 }
    )
    const d = diffScenes(prev, next)
    expect(d.moved).toEqual([0])
    expect(d.fromX[0]).toBe(0)
    expect(d.layoutChanged).toBe(true)
  })

  it("ignores movement without a layout version bump", () => {
    const prev = sceneOf(base, [], { layoutVersion: 1 })
    const next = sceneOf(
      [
        { id: "a", x: 30, y: 0 },
        { id: "b", x: 100, y: 0 },
      ],
      [],
      { layoutVersion: 1 }
    )
    expect(diffScenes(prev, next).layoutChanged).toBe(false)
  })

  it("does not call sub-pixel jitter a move", () => {
    const prev = sceneOf(base, [], { layoutVersion: 1 })
    const next = sceneOf(
      [
        { id: "a", x: 0.2, y: 0 },
        { id: "b", x: 100, y: 0 },
      ],
      [],
      { layoutVersion: 2 }
    )
    expect(diffScenes(prev, next).moved).toEqual([])
  })
})

describe("shouldTween", () => {
  const moved = diffScenes(
    sceneOf(base, [], { layoutVersion: 1 }),
    sceneOf(
      [
        { id: "a", x: 30, y: 0 },
        { id: "b", x: 100, y: 0 },
      ],
      [],
      { layoutVersion: 2 }
    )
  )
  it("tweens small graphs and snaps big ones (>5k)", () => {
    expect(shouldTween(moved, 200)).toBe(true)
    expect(shouldTween(moved, TWEEN_NODE_LIMIT)).toBe(true)
    expect(shouldTween(moved, TWEEN_NODE_LIMIT + 1)).toBe(false)
  })
})
