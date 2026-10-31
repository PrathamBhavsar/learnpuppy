import { describe, expect, it } from "vitest"

import { formatElapsed, masteryByTopic } from "./session-summary"
import { formatIntervalDays } from "./time"

describe("masteryByTopic", () => {
  it("groups by source decision; mastery is the share rated Good or Easy", () => {
    const topics = masteryByTopic([
      {
        cardId: "1",
        sourceDecisionId: "dec_1",
        rating: 3,
        correct: true,
        nextDue: 0,
      },
      {
        cardId: "2",
        sourceDecisionId: "dec_1",
        rating: 1,
        correct: false,
        nextDue: 0,
      },
      {
        cardId: "3",
        sourceDecisionId: "dec_2",
        rating: 4,
        correct: true,
        nextDue: 0,
      },
      {
        cardId: "4",
        sourceDecisionId: undefined,
        rating: 2,
        correct: true,
        nextDue: 0,
      },
    ])
    expect(topics).toEqual([
      { id: "dec_1", label: "Decision 1", value: 0.5, cards: 2 },
      { id: "dec_2", label: "Decision 2", value: 1, cards: 1 },
      { id: "", label: "Other cards", value: 0, cards: 1 },
    ])
  })
})

describe("formatters", () => {
  it("formats elapsed time as m:ss", () => {
    expect(formatElapsed(0)).toBe("0:00")
    expect(formatElapsed(65_400)).toBe("1:05")
  })

  it("formats server-previewed intervals compactly", () => {
    expect(formatIntervalDays(0.0001)).toBe("<1m")
    expect(formatIntervalDays(10 / 1440)).toBe("10m")
    expect(formatIntervalDays(0.25)).toBe("6h")
    expect(formatIntervalDays(3)).toBe("3d")
    expect(formatIntervalDays(14)).toBe("2w")
    expect(formatIntervalDays(90)).toBe("3mo")
  })
})
