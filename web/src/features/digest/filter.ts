import type {
  DigestDay,
  DigestDecision,
  DigestSession,
} from "@/api/pending-types"

function matches(text: string, needle: string): boolean {
  return text.toLowerCase().includes(needle)
}

function sessionMatches(session: DigestSession, needle: string): boolean {
  return matches(session.project, needle) || matches(session.summary, needle)
}

function decisionMatches(decision: DigestDecision, needle: string): boolean {
  return matches(decision.title, needle) || matches(decision.chosen, needle)
}

/**
 * Client-side text filter over the loaded digest. A session whose project or summary matches keeps all
 * its decisions; otherwise only the matching decisions stay. Empty days and sessions are dropped.
 */
export function filterDays(
  days: readonly DigestDay[],
  query: string
): DigestDay[] {
  const needle = query.trim().toLowerCase()
  if (!needle) return [...days]
  return days
    .map((day) => ({
      ...day,
      sessions: day.sessions.flatMap((session) => {
        if (sessionMatches(session, needle)) return [session]
        const decisions = session.decisions.filter((d) =>
          decisionMatches(d, needle)
        )
        return decisions.length ? [{ ...session, decisions }] : []
      }),
    }))
    .filter((day) => day.sessions.length > 0)
}

function decisionsOf(days: readonly DigestDay[]): DigestDecision[] {
  return days.flatMap((d) => d.sessions.flatMap((s) => s.decisions))
}

/** Ids of the visible decisions that have at least one card, for "Review these N decisions". */
export function reviewableDecisionIds(days: readonly DigestDay[]): string[] {
  return decisionsOf(days)
    .filter((d) => d.cardIds.length > 0)
    .map((d) => d.id)
}
