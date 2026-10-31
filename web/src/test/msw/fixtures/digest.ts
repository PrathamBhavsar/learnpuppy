import type {
  DecisionDetail,
  DigestDay,
  DigestSession,
} from "@/api/pending-types"

import { DAY, HOUR } from "./rng"

const session = (
  id: string,
  source: DigestSession["source"],
  project: string,
  summary: string,
  startedAt: number,
  decisions: DigestSession["decisions"]
): DigestSession => ({
  sessionId: id,
  source,
  project,
  summary,
  startedAt,
  decisions,
})

export function digestFixtures(now: number): {
  days: DigestDay[]
  details: Map<string, DecisionDetail>
} {
  const day = (offset: number) =>
    new Date(now - offset * DAY).toISOString().slice(0, 10)
  const d = (
    n: number,
    title: string,
    chosen: string,
    basis: "stated" | "inferred",
    alternatives: [string, string][]
  ) => ({
    id: `dec_${n}`,
    title,
    basis,
    chosen,
    alternatives: alternatives.map(([option, rejectedBecause]) => ({
      option,
      rejectedBecause,
    })),
    cardIds: [`card_rustasync_${String(n).padStart(3, "0")}`],
  })
  const days: DigestDay[] = [
    {
      day: day(0),
      sessions: [
        session(
          "sess_a",
          "claude",
          "learnpuppy",
          "Wired the SSE hub and ring buffer replay.",
          now - 3 * HOUR,
          [
            d(
              1,
              "Use a ring buffer for event replay",
              "A 4096-entry ring per workspace stream",
              "stated",
              [
                [
                  "Persist events to disk",
                  "Adds IO to the hot path for no gain",
                ],
                ["Unbounded in-memory log", "Memory grows without bound"],
              ]
            ),
            d(
              2,
              "Resync when the client is too far behind",
              "Send a resync event and have the client invalidate everything",
              "stated",
              [["Replay from disk", "Not worth the complexity"]]
            ),
          ]
        ),
        session(
          "sess_b",
          "codex",
          "learnpuppy",
          "Workspace registry atomic writes.",
          now - 5 * HOUR,
          [
            d(
              3,
              "Write workspace.json atomically",
              "Write to a temp file then rename",
              "inferred",
              [["Write in place", "A crash would leave a corrupt file"]]
            ),
          ]
        ),
      ],
    },
    {
      day: day(1),
      sessions: [
        session(
          "sess_c",
          "agy",
          "raft-lab",
          "Leader election timeouts.",
          now - DAY - 4 * HOUR,
          [
            d(
              4,
              "Randomize election timeouts",
              "150-300 ms random timeout per node",
              "stated",
              [["Fixed timeouts", "Split votes repeat forever"]]
            ),
            d(
              5,
              "Heartbeat interval",
              "One tenth of the minimum election timeout",
              "inferred",
              [
                [
                  "Heartbeat equals timeout",
                  "Followers would time out between beats",
                ],
              ]
            ),
          ]
        ),
      ],
    },
    {
      day: day(3),
      sessions: [
        session(
          "sess_d",
          "git",
          "design-notes",
          "Type scale and spacing.",
          now - 3 * DAY,
          [
            d(
              6,
              "Use a 1.2 minor third type scale",
              "Base 13 px with a 1.2 ratio",
              "stated",
              [["1.333 perfect fourth", "Too steep for dense UI"]]
            ),
          ]
        ),
      ],
    },
  ]
  const details = new Map<string, DecisionDetail>()
  for (const dayGroup of days) {
    for (const s of dayGroup.sessions) {
      for (const dec of s.decisions) {
        details.set(dec.id, {
          ...dec,
          why: `${dec.chosen} keeps the behaviour predictable and easy to test.`,
          risk: "Low: reversible behind a single constant.",
          files: ["crates/lp-service/src/events.rs"],
          commits: [
            {
              sha: "4643f9d",
              subject: "merge: B11 server core into staging",
              at: s.startedAt + HOUR,
            },
          ],
        })
      }
    }
  }
  return { days, details }
}
