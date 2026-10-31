import { describe, expect, it } from "vitest"

import type { ReviewCard } from "@/api/pending-types"

import { buildQueue, parseDecisionIds } from "./queue"

const card = (id: string, decision?: string): ReviewCard => ({
  id,
  front: id,
  kind: "decision",
  sourceDecisionId: decision,
  due: 0,
  state: "review",
})

const due = [
  card("a1", "dec_a"),
  card("b1", "dec_b"),
  card("a2", "dec_a"),
  card("c1", "dec_c"),
  card("x"),
]
const ids = (cards: ReviewCard[]) => cards.map((c) => c.id)

describe("parseDecisionIds", () => {
  it("returns null for nothing and a set for a comma list", () => {
    expect(parseDecisionIds(undefined)).toBeNull()
    expect(parseDecisionIds(" , ")).toBeNull()
    expect([...parseDecisionIds("dec_a, dec_b")!]).toEqual(["dec_a", "dec_b"])
  })
})

describe("buildQueue", () => {
  it("keeps due order and applies the limit", () => {
    expect(
      ids(buildQueue(due, { decisions: null, shuffle: false, limit: 3 }))
    ).toEqual(["a1", "b1", "a2"])
  })

  it("filters to the chosen decisions and drops cards without a source", () => {
    const decisions = parseDecisionIds("dec_a,dec_c")
    expect(
      ids(buildQueue(due, { decisions, shuffle: false, limit: 10 }))
    ).toEqual(["a1", "a2", "c1"])
  })

  it("shuffles source groups but keeps each decision's cards together", () => {
    const reversed = () => 0 // always swaps with the first element
    const out = ids(
      buildQueue(due, {
        decisions: null,
        shuffle: true,
        limit: 10,
        random: reversed,
      })
    )
    expect(out).toHaveLength(5)
    expect(out.indexOf("a2") - out.indexOf("a1")).toBe(1)
    expect(new Set(out)).toEqual(new Set(ids(due)))
  })

  it("applies the limit after shuffling", () => {
    expect(
      buildQueue(due, {
        decisions: null,
        shuffle: true,
        limit: 2,
        random: () => 0.5,
      })
    ).toHaveLength(2)
  })
})
