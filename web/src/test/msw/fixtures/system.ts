import type {
  AccountDetail,
  IngestRun,
  IngestSources,
  SettingsCatalog,
  SettingsDoc,
} from "@/api/pending-types"

import { DAY, HOUR } from "./rng"

export function accountFixtures(now: number): AccountDetail[] {
  return [
    {
      id: "claude:acc-1",
      provider: "claude",
      email: "dev@example.com",
      tier: "max_5x",
      status: "ready",
      cooldownUntil: null,
      cooldownReason: null,
      requests: 128,
      tokensIn: 412_000,
      tokensOut: 96_000,
    },
    {
      id: "claude:acc-2",
      provider: "claude",
      email: "work@example.com",
      tier: "pro",
      status: "cooldown",
      cooldownUntil: now + 42_000,
      cooldownReason: "rate limited",
      requests: 77,
      tokensIn: 198_000,
      tokensOut: 41_000,
    },
    {
      id: "codex:acc-1",
      provider: "codex",
      email: "dev@example.com",
      tier: "plus",
      status: "ready",
      cooldownUntil: null,
      cooldownReason: null,
      requests: 23,
      tokensIn: 60_000,
      tokensOut: 12_000,
    },
    {
      id: "agy:acc-1",
      provider: "agy",
      email: "dev@example.com",
      tier: null,
      status: "auth_expired",
      cooldownUntil: null,
      cooldownReason: null,
      requests: 4,
      tokensIn: 3_000,
      tokensOut: 800,
    },
  ]
}

export const ROLES = [
  "prober",
  "teacher",
  "practice",
  "extractor",
  "auditor",
  "pdf",
  "research",
  "visual",
  "demo",
] as const

export function settingsFixture(): SettingsDoc {
  const profiles: SettingsDoc["profiles"] = {}
  for (const role of ROLES) {
    profiles[role] = {
      provider: "claude",
      model: role === "teacher" ? "claude-opus-5-5" : "claude-sonnet-5-5",
      effort: role === "teacher" ? "high" : "medium",
      fallback: "codex",
    }
  }
  return { profiles, pins: {}, concurrencyCap: 2, themeDefault: "system" }
}

export const CATALOG: SettingsCatalog = {
  providers: [
    {
      id: "claude",
      label: "Claude",
      models: [
        "claude-fable-5-1",
        "claude-opus-5-5",
        "claude-sonnet-5-5",
        "claude-haiku-5-5",
      ],
    },
    { id: "codex", label: "Codex", models: ["gpt-5.5", "gpt-5.5-mini"] },
    {
      id: "agy",
      label: "Antigravity",
      models: ["gemini-3-pro", "gemini-3-flash"],
    },
  ],
  effortLevels: ["low", "medium", "high", "xhigh"],
}

export function ingestFixtures(now: number): {
  sources: IngestSources
  runs: IngestRun[]
} {
  return {
    sources: {
      allow: ["/Users/dev/code/tokio-notes", "/Users/dev/code/learnpuppy"],
      claude: true,
      codex: true,
      agy: true,
      git: true,
      sinceDays: 30,
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
      overlaps: [],
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
