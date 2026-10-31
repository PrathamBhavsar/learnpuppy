import type { Rating } from "@/api/pending-types"

import type { SessionPhase } from "./use-session"

export type SessionKeyAction =
  | { type: "choose"; index: number }
  | { type: "rate"; rating: Rating }
  | { type: "accept" }
  | { type: "exit" }

const CHOICE_KEYS: Record<string, number> = {
  "1": 0,
  "2": 1,
  "3": 2,
  "4": 3,
  a: 0,
  b: 1,
  c: 2,
  d: 3,
}

type KeyInput = {
  key: string
  ctrlKey?: boolean
  metaKey?: boolean
  altKey?: boolean
}

/**
 * Maps a key press to a session action (docs/web/06): 1-4 / a-d choose while answering; 1-4 rate and Space
 * accepts the server suggestion once answered; Esc exits. Pure so the keyboard contract is unit-tested.
 */
export function resolveSessionKey(
  phase: SessionPhase,
  optionCount: number,
  event: KeyInput
): SessionKeyAction | null {
  if (event.ctrlKey || event.metaKey || event.altKey) return null
  const key = event.key.toLowerCase()
  if (key === "escape") {
    return phase === "answering" || phase === "answered"
      ? { type: "exit" }
      : null
  }
  if (phase === "answering") {
    const index = CHOICE_KEYS[key]
    return index !== undefined && index < optionCount
      ? { type: "choose", index }
      : null
  }
  if (phase === "answered") {
    if (key === " ") return { type: "accept" }
    const index = CHOICE_KEYS[key]
    if (index !== undefined && key >= "1" && key <= "4")
      return { type: "rate", rating: (index + 1) as Rating }
  }
  return null
}
