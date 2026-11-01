import { describe, expect, it } from "vitest"

import type { Rating, ReviewCard } from "@/api/pending-types"

import type { AnswerResult } from "./api"
import {
  type SessionState,
  gradedCount,
  initialSession,
  sessionReducer,
} from "./use-session"

const card = (id: string, decision = "dec_1"): ReviewCard => ({
  id,
  front: `front ${id}`,
  options: [{ key: "A", text: "a" }],
  kind: "decision",
  sourceDecisionId: decision,
  due: 0,
  state: "review",
})

const result = (rating: Rating, correct = true): AnswerResult => ({
  correct,
  correctKey: "A",
  explanation: "because",
  rating,
  next: { due: 1000 * rating, intervalDays: rating },
  preview: { again: 0.001, hard: 1, good: 3, easy: 8 },
  previewItems: [],
})

const started = (...ids: string[]): SessionState =>
  sessionReducer(initialSession(), {
    type: "START",
    queue: ids.map((id) => card(id)),
    now: 1000,
  })

const answered = (state: SessionState, rating: Rating = 3): SessionState =>
  sessionReducer(state, {
    type: "ANSWER",
    result: result(rating),
    choiceKey: "A",
  })

describe("session reducer", () => {
  it("starts on the first card and ignores an empty queue", () => {
    const s = started("c1", "c2", "c3")
    expect(s.phase).toBe("answering")
    expect(s.current?.id).toBe("c1")
    expect(s.queue.map((c) => c.id)).toEqual(["c2", "c3"])
    expect(s.totalCards).toBe(3)
    expect(started().phase).toBe("idle")
  })

  it("reveals the verdict on answer, only while answering", () => {
    const s = answered(started("c1"))
    expect(s.phase).toBe("answered")
    expect(s.result?.rating).toBe(3)
    expect(s.choiceKey).toBe("A")
    expect(
      sessionReducer(s, { type: "ANSWER", result: result(1), choiceKey: "B" })
    ).toBe(s)
  })

  it("advances at once on grade and counts the rating", () => {
    const s = sessionReducer(answered(started("c1", "c2")), {
      type: "GRADE",
      rating: 4,
    })
    expect(s.phase).toBe("answering")
    expect(s.current?.id).toBe("c2")
    expect(s.result).toBeNull()
    expect(s.rated[4]).toBe(1)
    expect(s.lastRating).toBe(4)
    expect(gradedCount(s)).toBe(1)
  })

  it("ignores grading before an answer was revealed", () => {
    const s = started("c1", "c2")
    expect(sessionReducer(s, { type: "GRADE", rating: 3 })).toBe(s)
  })

  it("completes after the last card with elapsed time and outcomes", () => {
    const s = sessionReducer(answered(started("c1"), 1), {
      type: "GRADE",
      rating: 1,
      now: 6000,
    })
    expect(s.phase).toBe("complete")
    expect(s.current).toBeNull()
    expect(s.elapsedMs).toBe(5000)
    expect(s.outcomes).toEqual([
      {
        cardId: "c1",
        sourceDecisionId: "dec_1",
        rating: 1,
        correct: true,
        nextDue: 1000,
      },
    ])
  })

  it("reverts a failed grade back onto its answered card", () => {
    const before = answered(started("c1", "c2"))
    const failed = {
      card: before.current!,
      result: before.result!,
      choiceKey: "A",
      rating: 3 as Rating,
    }
    const after = sessionReducer(before, { type: "GRADE", rating: 3 })
    const reverted = sessionReducer(after, { type: "REVERT", failed })
    expect(reverted.phase).toBe("answered")
    expect(reverted.current?.id).toBe("c1")
    expect(reverted.result).toEqual(failed.result)
    expect(reverted.queue.map((c) => c.id)).toEqual(["c2"])
    expect(reverted.rated[3]).toBe(0)
    expect(reverted.outcomes).toEqual([])
  })

  it("reverts after the session completed by restoring the card", () => {
    const before = answered(started("c1"))
    const failed = {
      card: before.current!,
      result: before.result!,
      choiceKey: "A",
      rating: 3 as Rating,
    }
    const done = sessionReducer(before, { type: "GRADE", rating: 3 })
    expect(done.phase).toBe("complete")
    const reverted = sessionReducer(done, { type: "REVERT", failed })
    expect(reverted.phase).toBe("answered")
    expect(reverted.current?.id).toBe("c1")
  })

  it("queues the failed card behind a newer card that is already answered", () => {
    const first = answered(started("c1", "c2", "c3"))
    const failed = {
      card: first.current!,
      result: first.result!,
      choiceKey: "A",
      rating: 3 as Rating,
    }
    const second = answered(sessionReducer(first, { type: "GRADE", rating: 3 }))
    const reverted = sessionReducer(second, { type: "REVERT", failed })
    expect(reverted.current?.id).toBe("c2")
    expect(reverted.phase).toBe("answered")
    expect(reverted.queue.map((c) => c.id)).toEqual(["c1", "c3"])
  })

  it("puts the displaced unanswered card back in the queue on revert", () => {
    const first = answered(started("c1", "c2"))
    const failed = {
      card: first.current!,
      result: first.result!,
      choiceKey: "A",
      rating: 2 as Rating,
    }
    const second = sessionReducer(first, { type: "GRADE", rating: 2 })
    const reverted = sessionReducer(second, { type: "REVERT", failed })
    expect(reverted.current?.id).toBe("c1")
    expect(reverted.queue.map((c) => c.id)).toEqual(["c2"])
  })

  it("returns to idle", () => {
    expect(sessionReducer(started("c1"), { type: "IDLE" })).toMatchObject({
      phase: "idle",
      current: null,
      queue: [],
    })
  })

  it("ignores stale answers and rollback after leaving a session", () => {
    const first = answered(started("c1"))
    const after = sessionReducer(first, { type: "IDLE" })
    expect(
      sessionReducer(after, {
        type: "ANSWER",
        result: result(3),
        choiceKey: "A",
        cardId: "c1",
        runId: first.runId,
      })
    ).toBe(after)
    expect(
      sessionReducer(after, {
        type: "REVERT",
        failed: {
          card: first.current!,
          result: first.result!,
          choiceKey: "A",
          rating: 3,
        },
        runId: first.runId,
      })
    ).toBe(after)
  })

  it("preserves a newer card whose answer request is still running", () => {
    const first = answered(started("c1", "c2"))
    let second = sessionReducer(first, { type: "GRADE", rating: 3 })
    second = sessionReducer(second, {
      type: "CHOOSING",
      cardId: "c2",
      choiceKey: "B",
      runId: second.runId,
    })
    const reverted = sessionReducer(second, {
      type: "REVERT",
      failed: {
        card: first.current!,
        result: first.result!,
        choiceKey: "A",
        rating: 3,
      },
    })
    expect(reverted.current?.id).toBe("c2")
    expect(reverted.queue.map((c) => c.id)).toEqual(["c1"])
  })

  it("uses the server's preview for the chosen rating in the summary", () => {
    const before = answered(started("c1"))
    before.result!.previewItems = [
      {
        rating: 4,
        due: 8000,
        intervalMs: 7000,
        intervalLabel: "server label",
        label: "Easy",
      },
    ]
    const after = sessionReducer(before, { type: "GRADE", rating: 4 })
    expect(after.outcomes[0]?.nextDue).toBe(8000)
  })
})
