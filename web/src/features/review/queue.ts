import type { ReviewCard } from "@/api/pending-types"

/** Fisher-Yates over a copy, with an injectable RNG so tests are deterministic. */
function shuffled<T>(items: readonly T[], random: () => number): T[] {
  const copy = [...items]
  for (let i = copy.length - 1; i > 0; i--) {
    const j = Math.floor(random() * (i + 1))
    ;[copy[i], copy[j]] = [copy[j]!, copy[i]!]
  }
  return copy
}

export function parseDecisionIds(
  param: string | undefined
): Set<string> | null {
  if (!param) return null
  const ids = param
    .split(",")
    .map((id) => id.trim())
    .filter(Boolean)
  return ids.length ? new Set(ids) : null
}

/**
 * Builds the session queue from the due cards: optional decision filter ("Review these N decisions"),
 * optional shuffle of source groups (cards from one decision stay adjacent), then the per-session limit.
 */
export function buildQueue(
  due: readonly ReviewCard[],
  options: {
    decisions: Set<string> | null
    shuffle: boolean
    limit: number
    random?: () => number
  }
): ReviewCard[] {
  const filtered = options.decisions
    ? due.filter(
        (c) => c.sourceDecisionId && options.decisions?.has(c.sourceDecisionId)
      )
    : [...due]
  if (!options.shuffle) return filtered.slice(0, options.limit)
  const random = options.random ?? Math.random
  const bySource = new Map<string, ReviewCard[]>()
  for (const card of filtered) {
    const key = card.sourceDecisionId ?? card.id
    bySource.set(key, [...(bySource.get(key) ?? []), card])
  }
  return shuffled([...bySource.values()], random)
    .flat()
    .slice(0, options.limit)
}
