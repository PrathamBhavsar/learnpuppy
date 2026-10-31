import { describe, expect, it } from "vitest"

import {
  NO_FILTERS,
  countVisible,
  filtersActive,
  matchMask,
  parseSources,
  serializeSources,
  visibilityMask,
} from "./filters"
import { sceneOf } from "./test-snapshot"

const scene = sceneOf(
  [
    { id: "a", x: 0, y: 0, source: "claude", cluster: "k1" },
    { id: "b", x: 1, y: 0, source: "codex", cluster: "k1" },
    { id: "c", x: 2, y: 0, source: "claude", cluster: "k2" },
    { id: "d", x: 3, y: 0, source: "git" },
  ],
  [
    ["a", "b"],
    ["a", "c"],
    ["a", "d"],
  ]
)

describe("visibilityMask", () => {
  it("is null when no filter is set, so callers skip masking", () => {
    expect(filtersActive(NO_FILTERS)).toBe(false)
    expect(visibilityMask(scene, NO_FILTERS)).toBeNull()
  })

  it("filters by source", () => {
    const m = visibilityMask(scene, { ...NO_FILTERS, sources: ["claude"] })
    expect(Array.from(m ?? [])).toEqual([1, 0, 1, 0])
  })

  it("filters by cluster", () => {
    const m = visibilityMask(scene, { ...NO_FILTERS, cluster: "k1" })
    expect(Array.from(m ?? [])).toEqual([1, 1, 0, 0])
  })

  it("filters by minimum degree", () => {
    const m = visibilityMask(scene, { ...NO_FILTERS, minDegree: 2 })
    expect(Array.from(m ?? [])).toEqual([1, 0, 0, 0])
  })

  it("combines filters with AND", () => {
    const m = visibilityMask(scene, { sources: ["claude"], cluster: "k1", minDegree: 0 })
    expect(Array.from(m ?? [])).toEqual([1, 0, 0, 0])
  })

  it("treats an unknown cluster as matching nothing", () => {
    const m = visibilityMask(scene, { ...NO_FILTERS, cluster: "gone" })
    expect(Array.from(m ?? [])).toEqual([0, 0, 0, 0])
  })
})

describe("countVisible", () => {
  it("counts all when unfiltered", () => {
    expect(countVisible(scene, null)).toEqual({ nodes: 4, chains: 3 })
  })

  it("counts only chains with both endpoints visible", () => {
    const m = visibilityMask(scene, { ...NO_FILTERS, sources: ["claude"] })
    expect(countVisible(scene, m)).toEqual({ nodes: 2, chains: 1 })
  })
})

describe("matchMask", () => {
  it("marks matched ids and ignores unknown ones", () => {
    expect(Array.from(matchMask(scene, ["b", "nope", "d"]) ?? [])).toEqual([0, 1, 0, 1])
  })
  it("is null without a search and all-zero for a search with no hits", () => {
    expect(matchMask(scene, null)).toBeNull()
    expect(Array.from(matchMask(scene, []) ?? [])).toEqual([0, 0, 0, 0])
  })
})

describe("source params", () => {
  it("parses known sources in canonical order and drops junk", () => {
    expect(parseSources("git,claude,bogus")).toEqual(["claude", "git"])
    expect(parseSources(undefined)).toEqual([])
    expect(parseSources("")).toEqual([])
  })
  it("serializes to the URL form, undefined when empty", () => {
    expect(serializeSources(["git", "claude"])).toBe("claude,git")
    expect(serializeSources([])).toBeUndefined()
  })
})
