import { describe, expect, it } from "vitest"

import { nearestInDirection, nearestToPoint, stepTab, tabOrder } from "./nav"
import { sceneOf } from "./test-snapshot"

// Layout (y grows downward):
//        up(1)
//  left(2) center(0) right(3)
//        down(4)      far-right(5)   diagonal(6)
const scene = sceneOf(
  [
    { id: "center", x: 0, y: 0 },
    { id: "up", x: 5, y: -100 },
    { id: "left", x: -100, y: 4 },
    { id: "right", x: 100, y: -3 },
    { id: "down", x: -2, y: 100 },
    { id: "far", x: 400, y: 0 },
    { id: "diag", x: 70, y: 70 },
  ],
  [
    ["center", "up"],
    ["center", "left"],
    ["center", "right"],
  ]
)

describe("nearestInDirection", () => {
  it("moves to the neighbour in each direction", () => {
    const at = (id: string) => scene.index.get(id) as number
    expect(nearestInDirection(scene, null, at("center"), "up")).toBe(at("up"))
    expect(nearestInDirection(scene, null, at("center"), "down")).toBe(
      at("down")
    )
    expect(nearestInDirection(scene, null, at("center"), "left")).toBe(
      at("left")
    )
    expect(nearestInDirection(scene, null, at("center"), "right")).toBe(
      at("right")
    )
  })

  it("prefers the straight-ahead node over a nearer diagonal one", () => {
    const at = (id: string) => scene.index.get(id) as number
    // `diag` (70,70) is ~99 away at 45° down-right; `right` is 100 straight ahead.
    expect(nearestInDirection(scene, null, at("center"), "right")).toBe(
      at("right")
    )
  })

  it("skips hidden nodes", () => {
    const at = (id: string) => scene.index.get(id) as number
    const visible = new Uint8Array(scene.count).fill(1)
    visible[at("right")] = 0
    expect(nearestInDirection(scene, visible, at("center"), "right")).toBe(
      at("far")
    )
  })

  it("returns -1 when nothing lies that way", () => {
    const at = (id: string) => scene.index.get(id) as number
    expect(nearestInDirection(scene, null, at("far"), "right")).toBe(-1)
  })

  it("widens the cone when the strict one is empty", () => {
    const lone = sceneOf([
      { id: "a", x: 0, y: 0 },
      { id: "b", x: 100, y: 150 },
    ])
    // b sits ~56° below the x axis: outside 45°, inside 80°.
    expect(nearestInDirection(lone, null, 0, "right")).toBe(1)
  })
})

describe("tab order", () => {
  it("is degree descending and respects visibility", () => {
    const order = Array.from(tabOrder(scene, null))
    expect(order[0]).toBe(scene.index.get("center"))
    const visible = new Uint8Array(scene.count).fill(1)
    visible[scene.index.get("center") as number] = 0
    expect(Array.from(tabOrder(scene, visible))).not.toContain(
      scene.index.get("center")
    )
  })

  it("steps forward and backward, entering from -1", () => {
    const order = Uint32Array.from([4, 2, 9])
    expect(stepTab(order, -1, false)).toBe(4)
    expect(stepTab(order, -1, true)).toBe(9)
    expect(stepTab(order, 4, false)).toBe(2)
    expect(stepTab(order, 2, true)).toBe(4)
  })

  it("lets focus leave at the ends instead of trapping it", () => {
    const order = Uint32Array.from([4, 2, 9])
    expect(stepTab(order, 9, false)).toBeNull()
    expect(stepTab(order, 4, true)).toBeNull()
    expect(stepTab(new Uint32Array(0), -1, false)).toBeNull()
  })
})

describe("nearestToPoint", () => {
  it("finds the visible node closest to a world point", () => {
    expect(nearestToPoint(scene, null, 95, -5)).toBe(scene.index.get("right"))
    const visible = new Uint8Array(scene.count).fill(1)
    visible[scene.index.get("right") as number] = 0
    expect(nearestToPoint(scene, visible, 95, -5)).not.toBe(
      scene.index.get("right")
    )
  })
})
