import type { AccountDetail, AccountStatus } from "@/api/pending-types"

export const STATUS_LABEL: Record<AccountStatus, string> = {
  ready: "Ready",
  cooldown: "Cooling down",
  auth_expired: "Login expired",
  disabled: "Disabled",
}

const PROVIDER_ORDER = ["claude", "codex", "agy"] as const

const PROVIDER_TITLE: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  agy: "Antigravity",
}

/** Shown when the server sent no `reauthHint` for an expired login. */
const FALLBACK_HINT: Record<string, string> = {
  claude: "claude login",
  codex: "codex login",
  agy: "agy auth login",
}

export function providerTitle(provider: string): string {
  return PROVIDER_TITLE[provider] ?? provider
}

export function reauthCommand(account: AccountDetail): string | null {
  if (account.reauthHint) return account.reauthHint
  return account.status === "auth_expired"
    ? (FALLBACK_HINT[account.provider] ?? null)
    : null
}

export type AccountGroup = {
  provider: string
  title: string
  accounts: AccountDetail[]
}

/** Claude, Codex, Antigravity, then anything else, each with its accounts in server order. */
export function groupAccounts(
  accounts: readonly AccountDetail[]
): AccountGroup[] {
  const providers = [
    ...PROVIDER_ORDER.filter((p) => accounts.some((a) => a.provider === p)),
    ...new Set(
      accounts
        .map((a) => a.provider)
        .filter((p) => !(PROVIDER_ORDER as readonly string[]).includes(p))
    ),
  ]
  return providers.map((provider) => ({
    provider,
    title: providerTitle(provider),
    accounts: accounts.filter((a) => a.provider === provider),
  }))
}

export function formatTokens(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`
  if (n >= 1_000) return `${(n / 1_000).toFixed(n >= 100_000 ? 0 : 1)}k`
  return String(n)
}

export function formatCountdown(ms: number): string {
  const total = Math.max(0, Math.ceil(ms / 1000))
  const m = Math.floor(total / 60)
  const s = total % 60
  return m > 0 ? `${m}m ${String(s).padStart(2, "0")}s` : `${s}s`
}

/** Fraction of the cooldown still to run, 0..1. */
export function cooldownFraction(remainingMs: number, totalMs: number): number {
  if (totalMs <= 0) return 0
  return Math.min(1, Math.max(0, remainingMs / totalMs))
}

export function readyCount(accounts: readonly AccountDetail[]): number {
  return accounts.filter((a) => a.status === "ready").length
}
