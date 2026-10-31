import type { AccountDetail, SettingsDoc } from "@/api/pending-types"

const REDACTED = "[redacted]"

/**
 * Patterns that look like credentials. Diagnostics are built from an allow-list of fields, and this
 * pass is a second guard over whatever text (ours or the server's) ends up on the clipboard.
 */
const SECRET_PATTERNS: RegExp[] = [
  /\b(?:sk|pk|rk)-[A-Za-z0-9][A-Za-z0-9_-]{6,}/g,
  /\bgh[pousr]_[A-Za-z0-9]{16,}/g,
  /\bxox[abpr]-[A-Za-z0-9-]{10,}/g,
  /\beyJ[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]{8,}\.[A-Za-z0-9_-]*/g,
  /\bBearer\s+[A-Za-z0-9._~+/=-]{8,}/gi,
  /\b(?:access[_-]?token|refresh[_-]?token|oauth[_-]?token|api[_-]?key|secret|password|token)\b\s*[:=]\s*["']?[^\s"',;]+/gi,
  /\b[A-Za-z0-9_-]{40,}\b/g,
]

export function redactSecrets(text: string): string {
  return SECRET_PATTERNS.reduce(
    (out, pattern) =>
      out.replace(pattern, (match) => {
        const key = /^[A-Za-z_-]+\s*[:=]/.exec(match)
        return key ? `${key[0]} ${REDACTED}` : REDACTED
      }),
    text
  )
}

export type DiagnosticsInput = {
  version: string
  engine: string
  dataDir: string
  accounts: readonly AccountDetail[]
  settings: SettingsDoc | undefined
  userAgent: string
}

/** Plain-text report: versions, engine, per-provider account health and role routing. No emails, no tokens. */
export function buildDiagnostics(input: DiagnosticsInput): string {
  const lines = [
    "LearnPuppy diagnostics",
    `version: ${input.version}`,
    `engine: ${input.engine}`,
    `data dir: ${input.dataDir}`,
    `browser: ${input.userAgent}`,
    "",
    "accounts:",
  ]
  const providers = [...new Set(input.accounts.map((a) => a.provider))]
  if (providers.length === 0) lines.push("  none found")
  for (const provider of providers) {
    const mine = input.accounts.filter((a) => a.provider === provider)
    const byStatus = new Map<string, number>()
    for (const a of mine)
      byStatus.set(a.status, (byStatus.get(a.status) ?? 0) + 1)
    lines.push(
      `  ${provider}: ${mine.length} (${[...byStatus].map(([s, n]) => `${n} ${s}`).join(", ")})`
    )
  }
  if (input.settings) {
    lines.push(
      "",
      `concurrency cap: ${input.settings.concurrencyCap}`,
      "roles:"
    )
    for (const [role, p] of Object.entries(input.settings.profiles))
      lines.push(
        `  ${role}: ${p.provider}/${p.model}${p.effort ? ` (${p.effort})` : ""}`
      )
  }
  return redactSecrets(lines.join("\n"))
}
