import { describe, expect, it } from "vitest"

import type { DigestDay, DigestDecision } from "@/api/pending-types"

import { filterDays, reviewableDecisionIds } from "./filter"
import { formatDayLabel, isoDay, resolveRange } from "./range"

const NOW = Date.UTC(2026, 9, 31, 15, 0, 0)

describe("resolveRange", () => {
  it("resolves presets in UTC day keys", () => {
    expect(resolveRange("today", {}, NOW)).toEqual({
      from: "2026-10-31",
      to: "2026-10-31",
    })
    expect(resolveRange("7d", {}, NOW)).toEqual({
      from: "2026-10-25",
      to: "2026-10-31",
    })
    expect(resolveRange("30d", {}, NOW)).toEqual({
      from: "2026-10-02",
      to: "2026-10-31",
    })
  })

  it("accepts a custom range, swaps reversed bounds and drops malformed ones", () => {
    expect(
      resolveRange("custom", { from: "2026-10-01", to: "2026-10-10" }, NOW)
    ).toEqual({
      from: "2026-10-01",
      to: "2026-10-10",
    })
    expect(
      resolveRange("custom", { from: "2026-10-10", to: "2026-10-01" }, NOW)
    ).toEqual({
      from: "2026-10-01",
      to: "2026-10-10",
    })
    expect(
      resolveRange("custom", { from: "yesterday", to: "2026-10-01" }, NOW)
    ).toEqual({
      from: undefined,
      to: "2026-10-01",
    })
  })
})

describe("formatDayLabel", () => {
  it("names today and yesterday, formats other days and handles undated", () => {
    expect(formatDayLabel("2026-10-31", NOW)).toBe("Today")
    expect(formatDayLabel("2026-10-30", NOW)).toBe("Yesterday")
    expect(formatDayLabel("2026-10-28", NOW)).toMatch(/Oct/)
    expect(formatDayLabel("undated", NOW)).toBe("Undated")
    expect(isoDay(NOW)).toBe("2026-10-31")
  })
})

const decision = (
  id: string,
  title: string,
  chosen: string,
  cards: string[]
): DigestDecision => ({
  id,
  title,
  chosen,
  basis: "stated",
  alternatives: [],
  cardIds: cards,
})

const days: DigestDay[] = [
  {
    day: "2026-10-31",
    sessions: [
      {
        sessionId: "s1",
        source: "claude",
        project: "learnpuppy",
        summary: "Wired the SSE hub",
        startedAt: NOW,
        decisions: [
          decision("d1", "Ring buffer for replay", "4096 entries", ["c1"]),
          decision("d2", "Resync on lag", "send resync", []),
        ],
      },
    ],
  },
  {
    day: "2026-10-30",
    sessions: [
      {
        sessionId: "s2",
        source: "git",
        project: "design-notes",
        summary: "Type scale",
        startedAt: NOW,
        decisions: [decision("d3", "Minor third scale", "ratio 1.2", ["c3"])],
      },
    ],
  },
]

describe("filterDays", () => {
  it("returns everything for an empty query", () => {
    expect(filterDays(days, "  ")).toHaveLength(2)
  })

  it("keeps all decisions when the project or summary matches", () => {
    const out = filterDays(days, "learnpuppy")
    expect(out).toHaveLength(1)
    expect(out[0]!.sessions[0]!.decisions).toHaveLength(2)
  })

  it("keeps only matching decisions otherwise, case-insensitively, and drops empty days", () => {
    const out = filterDays(days, "RESYNC")
    expect(out).toHaveLength(1)
    expect(out[0]!.sessions[0]!.decisions.map((d) => d.id)).toEqual(["d2"])
    expect(filterDays(days, "nothing like this")).toEqual([])
  })
})

describe("reviewableDecisionIds", () => {
  it("lists visible decisions that have cards", () => {
    expect(reviewableDecisionIds(days)).toEqual(["d1", "d3"])
    expect(reviewableDecisionIds(filterDays(days, "scale"))).toEqual(["d3"])
  })
})
