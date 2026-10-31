/**
 * Review session state machine (docs/web/06 §Review). Pure reducer + a thin hook.
 *
 *   idle      no session
 *   answering a card is on screen, waiting for a choice
 *   answered  the server revealed correctness; the rating bar is showing
 *   complete  the queue drained; the summary shows
 *
 * Grading is optimistic: GRADE advances at once and REVERT puts the failed card back (restored to its
 * answered state when nothing newer has been touched) so a rejected grade is never silently lost.
 */
import { useCallback, useReducer } from "react"

import type { Rating, ReviewCard } from "@/api/pending-types"
import type { AnswerResult } from "./api"

export type SessionPhase = "idle" | "answering" | "answered" | "complete"

export type CardOutcome = {
  cardId: string
  sourceDecisionId: string | undefined
  rating: Rating
  correct: boolean
  nextDue: number
}

export type SessionState = {
  runId: number
  phase: SessionPhase
  /** Cards still to come, not including `current`. */
  queue: ReviewCard[]
  current: ReviewCard | null
  /** Server verdict for `current` (set in `answered`). */
  result: AnswerResult | null
  /** The option the user picked for `current`. */
  choiceKey: string | null
  totalCards: number
  rated: Record<Rating, number>
  outcomes: CardOutcome[]
  /** Rating of the card that just left, so the exit animation heads toward its side. */
  lastRating: Rating | null
  startedAt: number | null
  elapsedMs: number
}

export type FailedGrade = {
  card: ReviewCard
  result: AnswerResult
  choiceKey: string
  rating: Rating
}

type SessionAction =
  | { type: "START"; queue: ReviewCard[]; now?: number }
  | { type: "CHOOSING"; cardId: string; choiceKey: string; runId: number }
  | {
      type: "ANSWER"
      result: AnswerResult
      choiceKey: string
      cardId?: string
      runId?: number
    }
  | { type: "GRADE"; rating: Rating; now?: number }
  | { type: "REVERT"; failed: FailedGrade; runId?: number }
  | { type: "IDLE" }

const NO_RATINGS: Record<Rating, number> = { 1: 0, 2: 0, 3: 0, 4: 0 }

export function initialSession(): SessionState {
  return {
    runId: 0,
    phase: "idle",
    queue: [],
    current: null,
    result: null,
    choiceKey: null,
    totalCards: 0,
    rated: { ...NO_RATINGS },
    outcomes: [],
    lastRating: null,
    startedAt: null,
    elapsedMs: 0,
  }
}

export function gradedCount(state: SessionState): number {
  return state.rated[1] + state.rated[2] + state.rated[3] + state.rated[4]
}

export function sessionReducer(
  state: SessionState,
  action: SessionAction
): SessionState {
  switch (action.type) {
    case "START": {
      const [first, ...rest] = action.queue
      if (!first) return initialSession()
      return {
        ...initialSession(),
        runId: state.runId + 1,
        phase: "answering",
        current: first,
        queue: rest,
        totalCards: action.queue.length,
        startedAt: action.now ?? Date.now(),
      }
    }
    case "CHOOSING":
      return action.runId === state.runId &&
        state.current?.id === action.cardId &&
        state.phase === "answering"
        ? { ...state, choiceKey: action.choiceKey }
        : state
    case "ANSWER": {
      if (
        (action.runId !== undefined && action.runId !== state.runId) ||
        (action.cardId && action.cardId !== state.current?.id)
      )
        return state
      if (state.phase !== "answering") return state
      return {
        ...state,
        phase: "answered",
        result: action.result,
        choiceKey: action.choiceKey,
      }
    }
    case "GRADE": {
      if (state.phase !== "answered" || !state.current || !state.result)
        return state
      const outcome: CardOutcome = {
        cardId: state.current.id,
        sourceDecisionId: state.current.sourceDecisionId,
        rating: action.rating,
        correct: state.result.correct,
        nextDue:
          state.result.previewItems?.find((p) => p.rating === action.rating)
            ?.due ?? state.result.next.due,
      }
      const base = {
        ...state,
        rated: {
          ...state.rated,
          [action.rating]: state.rated[action.rating] + 1,
        },
        outcomes: [...state.outcomes, outcome],
        lastRating: action.rating,
        result: null,
        choiceKey: null,
      }
      const [head, ...tail] = state.queue
      if (!head) {
        const now = action.now ?? Date.now()
        return {
          ...base,
          phase: "complete",
          current: null,
          queue: [],
          elapsedMs: state.startedAt ? now - state.startedAt : 0,
        }
      }
      return { ...base, phase: "answering", current: head, queue: tail }
    }
    case "REVERT": {
      if (action.runId !== undefined && action.runId !== state.runId)
        return state
      const { card, result, choiceKey, rating } = action.failed
      if (!state.outcomes.some((o) => o.cardId === card.id)) return state
      const outcomes = state.outcomes.filter((o) => o.cardId !== card.id)
      const rated = {
        ...state.rated,
        [rating]: Math.max(0, state.rated[rating] - 1),
      }
      const reverted = { ...state, outcomes, rated, lastRating: null }
      // Newer card already answered: do not steal the screen, retry the failed one afterwards.
      if (
        state.phase === "answered" ||
        (state.phase === "answering" && state.choiceKey)
      ) {
        return { ...reverted, queue: [card, ...state.queue] }
      }
      const displaced = state.current ? [state.current] : []
      return {
        ...reverted,
        phase: "answered",
        current: card,
        result,
        choiceKey,
        queue: [...displaced, ...state.queue],
        elapsedMs: 0,
      }
    }
    case "IDLE":
      return { ...initialSession(), runId: state.runId + 1 }
    default:
      return state
  }
}

export type Session = ReturnType<typeof useSession>

export function useSession() {
  const [state, dispatch] = useReducer(
    sessionReducer,
    undefined,
    initialSession
  )
  const start = useCallback(
    (queue: ReviewCard[]) => dispatch({ type: "START", queue }),
    []
  )
  const answer = useCallback(
    (result: AnswerResult, choiceKey: string, cardId?: string) =>
      dispatch({
        type: "ANSWER",
        result,
        choiceKey,
        cardId,
        runId: state.runId,
      }),
    [state.runId]
  )
  const choosing = useCallback(
    (cardId: string, choiceKey: string) =>
      dispatch({ type: "CHOOSING", cardId, choiceKey, runId: state.runId }),
    [state.runId]
  )
  const grade = useCallback(
    (rating: Rating) => dispatch({ type: "GRADE", rating }),
    []
  )
  const revert = useCallback(
    (failed: FailedGrade) =>
      dispatch({ type: "REVERT", failed, runId: state.runId }),
    [state.runId]
  )
  const idle = useCallback(() => dispatch({ type: "IDLE" }), [])
  return { state, start, choosing, answer, grade, revert, idle }
}
