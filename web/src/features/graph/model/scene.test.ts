import { describe, expect, it } from "vitest"

import { SOURCES, neighbours, weightForDegree } from "./scene"
import { sceneOf, snapshotOf } from "./test-snapshot"
import { buildScene } from "./scene"

describe("buildScene", () => {
  const scene = sceneOf(
    [
      { id: "a", x: 0, y: 0, cluster: "k1", source: "codex" },
      { id: "b", x: 100, y: 0, cluster: "k1" },
      {
        id: "c",
        x: 100,
        y: 100,
        cluster: "k2",
        source: "git",
      },
      { id: "d", x: -50, y: 80 },
    ],
    [
      ["a", "b"],
      ["a", "c"],
      ["c", "d", "conjunction"],
      ["a", "missing"],
      ["b", "b"],
    ]
  )

  it("indexes nodes and keeps ids, texts and sources in step", () => {
    expect(scene.count).toBe(4)
    expect(scene.index.get("c")).toBe(2)
    expect(scene.ids).toEqual(["a", "b", "c", "d"])
    expect(SOURCES[scene.source[0] as number]).toBe("codex")
    // The wire carries no claim/concept kind, so every node draws as a claim.
    expect(scene.kind[2]).toBe(0)
  })

  it("drops chains with a missing endpoint or a self loop", () => {
    expect(scene.edgeCount).toBe(3)
    expect(Array.from(scene.degree)).toEqual([2, 1, 2, 1])
  })

  it("builds symmetric CSR adjacency", () => {
    expect(Array.from(neighbours(scene, 0)).sort()).toEqual([1, 2])
    expect(Array.from(neighbours(scene, 2)).sort()).toEqual([0, 3])
    expect(Array.from(neighbours(scene, 3))).toEqual([2])
  })

  it("orders by degree then id", () => {
    expect(Array.from(scene.order)).toEqual([0, 2, 1, 3])
  })

  it("computes bounds from positions", () => {
    expect(scene.bounds).toEqual({ minX: -50, minY: 0, maxX: 100, maxY: 100 })
  })

  it("assigns cluster indices and hull slots", () => {
    expect(scene.cluster[0]).toBe(0)
    expect(scene.cluster[2]).toBe(1)
    expect(scene.cluster[3]).toBe(-1)
    expect(scene.clusters.map((c) => c.slot)).toEqual([1, 2])
    expect(scene.clusters[0]?.hull.length).toBeGreaterThanOrEqual(3)
  })

  it("weights size by degree within a sane range", () => {
    expect(weightForDegree(0)).toBeCloseTo(0.8)
    expect(weightForDegree(1000)).toBe(2.2)
    expect(scene.weight[0] as number).toBeGreaterThan(scene.weight[1] as number)
  })
})

describe("unplaced nodes", () => {
  it("land near their cluster, deterministically", () => {
    const snap = snapshotOf([
      { id: "a", x: 500, y: 500, cluster: "k" },
      { id: "b", x: 520, y: 480, cluster: "k" },
      { id: "new", cluster: "k" },
    ])
    const one = buildScene(snap)
    const two = buildScene(snap)
    const i = one.index.get("new") as number
    expect(
      Math.hypot((one.x[i] as number) - 510, (one.y[i] as number) - 490)
    ).toBeLessThan(200)
    expect(one.x[i]).toBe(two.x[i])
    expect(one.y[i]).toBe(two.y[i])
  })

  it("handles an empty graph", () => {
    const empty = buildScene(snapshotOf([]))
    expect(empty.count).toBe(0)
    expect(empty.bounds.minX).toBeLessThan(empty.bounds.maxX)
  })
})
