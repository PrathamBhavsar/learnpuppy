import type { AccountDetail } from "@/api/pending-types"

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
      reauthHint: null,
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
      reauthHint: null,
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
      reauthHint: null,
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
      reauthHint: "agy auth login",
    },
  ]
}

/** The command that renews an expired login, as the server reports it (`reauthHint`). */
export function reauthHintFor(provider: string, id: string): string | null {
  if (provider === "claude")
    return `CLAUDE_CONFIG_DIR=~/.claude-accounts/${id.split(":")[1] ?? "work"} claude login`
  if (provider === "codex") return "codex login"
  if (provider === "agy") return "agy auth login"
  return null
}
