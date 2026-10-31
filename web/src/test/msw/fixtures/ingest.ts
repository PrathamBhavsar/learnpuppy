import type { IngestRun, IngestSourcesWire } from "@/api/pending-types"

import { DAY, HOUR } from "./rng"

export function ingestFixtures(now: number): {
  sources: IngestSourcesWire
  runs: IngestRun[]
} {
  return {
    sources: {
      sources: {
        allow: ["/Users/dev/code/tokio-notes", "/Users/dev/code/learnpuppy"],
        claude: true,
        codex: true,
        agy: true,
        git: true,
        sinceDays: 30,
      },
      enabled: true,
      runningId: null,
      candidates: [
        {
          path: "/Users/dev/code/raft-lab",
          sessions: 14,
          lastSeen: now - DAY,
          source: "claude",
        },
        {
          path: "/Users/dev/code/dotfiles",
          sessions: 3,
          lastSeen: now - 6 * DAY,
          source: "codex",
        },
        {
          path: "/Users/dev/code/site",
          sessions: 9,
          lastSeen: now - 2 * DAY,
          source: "agy",
        },
      ],
      warnings: [],
    },
    runs: [
      {
        id: "run_3",
        startedAt: now - 2 * HOUR,
        finishedAt: now - 2 * HOUR + 95_000,
        report: {
          sessions: 12,
          newDecisions: 9,
          newCards: 31,
          skipped: 2,
          errors: [],
        },
      },
      {
        id: "run_2",
        startedAt: now - DAY,
        finishedAt: now - DAY + 120_000,
        report: {
          sessions: 8,
          newDecisions: 5,
          newCards: 17,
          skipped: 0,
          errors: [
            "agy: conversation_summaries.db is locked (snapshot failed)",
            "claude: /Users/dev/code/old-project is not readable (permission denied)",
          ],
        },
      },
      {
        id: "run_1",
        startedAt: now - 5 * DAY,
        finishedAt: now - 5 * DAY + 60_000,
        report: {
          sessions: 3,
          newDecisions: 2,
          newCards: 6,
          skipped: 1,
          errors: [],
        },
      },
    ],
  }
}
