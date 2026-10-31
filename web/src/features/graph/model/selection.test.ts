import { describe, expect, it } from "vitest"

import {
  EMPTY_SELECTION,
  pruneSelection,
  selectMany,
  selectOnly,
  toggleSelected,
} from "./selection"

describe("selection", () => {
  it("selectOnly replaces everything and activates the node", () => {
    expect(selectOnly("a")).toEqual({ selected: ["a"], active: "a" })
  })

  it("toggle adds and activates, then removes", () => {
    let s = selectOnly("a")
    s = toggleSelected(s, "b")
    expect(s).toEqual({ selected: ["a", "b"], active: "b" })
    s = toggleSelected(s, "b")
    expect(s).toEqual({ selected: ["a"], active: "a" })
    s = toggleSelected(s, "a")
    expect(s).toEqual(EMPTY_SELECTION)
  })

  it("removing a non-active node keeps the active one", () => {
    const s = toggleSelected({ selected: ["a", "b", "c"], active: "c" }, "a")
    expect(s).toEqual({ selected: ["b", "c"], active: "c" })
  })

  it("marquee replaces, shift-marquee adds", () => {
    const base = selectOnly("a")
    expect(selectMany(base, ["b", "c"], false)).toEqual({ selected: ["b", "c"], active: null })
    expect(selectMany(base, ["b", "c"], true)).toEqual({ selected: ["a", "b", "c"], active: "a" })
  })

  it("a single swept node becomes active; duplicates collapse", () => {
    expect(selectMany(EMPTY_SELECTION, ["x"], false)).toEqual({ selected: ["x"], active: "x" })
    expect(selectMany(selectOnly("a"), ["a", "b"], true).selected).toEqual(["a", "b"])
  })

  it("an empty sweep without shift clears; with shift it keeps", () => {
    expect(selectMany(selectOnly("a"), [], false)).toEqual(EMPTY_SELECTION)
    expect(selectMany(selectOnly("a"), [], true)).toEqual(selectOnly("a"))
  })

  it("prunes ids that no longer exist and returns the same object when nothing changed", () => {
    const s = { selected: ["a", "b"], active: "b" }
    expect(pruneSelection(s, () => true)).toBe(s)
    expect(pruneSelection(s, (id) => id === "a")).toEqual({ selected: ["a"], active: "a" })
    expect(pruneSelection(s, () => false)).toEqual(EMPTY_SELECTION)
  })
})
