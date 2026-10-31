import type { RoleProfile } from "@/api/pending-types"

const PROVIDER_LABELS: Record<string, string> = {
  claude: "Claude",
  codex: "Codex",
  agy: "Antigravity",
}

export function providerLabel(id: string): string {
  return PROVIDER_LABELS[id] ?? id
}

export function roleLabel(role: string): string {
  return role.charAt(0).toUpperCase() + role.slice(1)
}

export type ProfileChange = {
  role: string
  field: "provider" | "model" | "effort" | "fallback" | "override"
  from: string
  to: string
}

const INHERIT = "inherit global"

function show(value: string | boolean | null | undefined): string {
  if (value === null || value === undefined || value === "") return "default"
  if (typeof value === "boolean") return value ? "on" : "off"
  return value
}

/** Field-by-field differences between two role → profile maps; a role missing on one side is "inherit global". */
export function diffProfiles(
  saved: Record<string, RoleProfile>,
  draft: Record<string, RoleProfile>
): ProfileChange[] {
  const changes: ProfileChange[] = []
  const roles = [...new Set([...Object.keys(saved), ...Object.keys(draft)])]
  for (const role of roles) {
    const before = saved[role]
    const after = draft[role]
    if (!before && after) {
      changes.push({ role, field: "override", from: INHERIT, to: "override" })
      continue
    }
    if (before && !after) {
      changes.push({ role, field: "override", from: "override", to: INHERIT })
      continue
    }
    if (!before || !after) continue
    const fields = ["provider", "model", "effort", "fallback"] as const
    for (const field of fields) {
      const from = show(before[field])
      const to = show(after[field])
      if (from !== to) changes.push({ role, field, from, to })
    }
  }
  return changes
}

/** Switching provider keeps the model only when the new provider offers it. */
export function withProvider(
  profile: RoleProfile,
  provider: string,
  models: readonly string[]
): RoleProfile {
  return {
    ...profile,
    provider,
    model: models.includes(profile.model) ? profile.model : (models[0] ?? ""),
  }
}

/** A copy of `record` without `key`. */
export function without<T>(
  record: Record<string, T>,
  key: string
): Record<string, T> {
  return Object.fromEntries(Object.entries(record).filter(([k]) => k !== key))
}
