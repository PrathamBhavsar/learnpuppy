import type { AccountDetail, AccountStatus } from "@/api/pending-types"

export const STATUS_LABEL: Record<AccountStatus, string> = {
  ready: "Ready",
  cooldown: "Limited",
  auth_expired: "Sign-in needed",
  disabled: "Disabled",
  offline: "Offline",
  no_model: "No model",
  missing: "Not installed",
}

const PROVIDER_ORDER = [
  "claude",
  "codex",
  "agy",
  "kiro",
  "opencode",
] as const

const PROVIDER_TITLE: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  agy: "Antigravity",
  kiro: "Kiro",
  opencode: "OpenCode",
}

/** Providers with a free way to start (shown first in the "Connect an AI" empty state). */
export const FREE_PROVIDERS = ["kiro", "opencode"] as const

/** The command to run yourself to connect each provider. The app never runs it. */
export const CONNECT_COMMAND: Record<string, string> = {
  claude: "claude login",
  codex: "codex login",
  agy: "agy auth login",
  kiro: "kiro-cli login",
  opencode: "opencode auth login",
}

/** One line on what each provider is, for the "not connected" cards. */
export const PROVIDER_BLURB: Record<string, string> = {
  claude: "Anthropic's Claude, through your Claude subscription.",
  codex: "OpenAI's Codex, through your ChatGPT plan.",
  agy: "Google's Antigravity.",
  kiro: "Kiro has a free tier with several models.",
  opencode: "OpenCode has free models (such as big-pickle) and needs no payment.",
}

/** Shown when the server sent no `reauthHint` for an expired login. */
const FALLBACK_HINT = CONNECT_COMMAND

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

/** Providers with no account at all, in display order. */
export function missingProviders(accounts: readonly AccountDetail[]): string[] {
  return PROVIDER_ORDER.filter((p) => !accounts.some((a) => a.provider === p))
}

/** The command that connects `account` (the server's own `howToConnect`, else the known one). */
export function connectCommand(account: AccountDetail): string | null {
  return (
    account.howToConnect ||
    reauthCommand(account) ||
    CONNECT_COMMAND[account.provider] ||
    null
  )
}

export type AiState =
  | { kind: "ok"; ready: number; total: number }
  | { kind: "partial"; ready: number; total: number }
  | { kind: "offline"; ready: number; total: number }
  /** Nothing is connected or usable (no account, or all signed out / uninstalled). */
  | { kind: "none"; ready: number; total: number }
  /** Everything usable is at its limit; `nextResetAt` is when the first comes back. */
  | { kind: "limited"; ready: number; total: number; nextResetAt: number | null }

/** The worst state across accounts: offline, then none connected, then all limited. */
export function aiState(accounts: readonly AccountDetail[]): AiState {
  const total = accounts.length
  const ready = readyCount(accounts)
  if (accounts.some((a) => a.status === "offline"))
    return { kind: "offline", ready, total }
  const live = accounts.filter(
    (a) => a.status === "ready" || a.status === "cooldown"
  )
  if (live.length === 0) return { kind: "none", ready, total }
  if (ready === 0) {
    const times = live
      .map((a) => a.resetsAt ?? a.cooldownUntil)
      .filter((t): t is number => typeof t === "number")
    return {
      kind: "limited",
      ready,
      total,
      nextResetAt: times.length ? Math.min(...times) : null,
    }
  }
  return { kind: ready < total ? "partial" : "ok", ready, total }
}
