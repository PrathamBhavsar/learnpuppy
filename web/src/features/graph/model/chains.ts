// The 3-level chain table as data: groups (source session) → chains → the two propositions a chain
// links. Sorting, expansion and flattening for the virtualized list live here so they are unit tested.
import type { ChainKind, ChainTableRow } from "@/api/pending-types"

export type TextOf = (nodeId: string) => string

export type ChainSort = {
  key: "group" | "count" | "kind" | "from" | "to"
  dir: "asc" | "desc"
}

export type ChainGroup = {
  id: string
  label: string
  rows: ChainTableRow[]
}

export type FlatRow =
  | { level: 1; key: string; group: ChainGroup; expanded: boolean }
  | {
      level: 2
      key: string
      row: ChainTableRow
      groupId: string
      expanded: boolean
    }
  | {
      level: 3
      key: string
      nodeId: string
      role: "from" | "to"
      kind: ChainKind
      chainId: string
    }

export const DEFAULT_SORT: ChainSort = { key: "count", dir: "desc" }

export function groupRows(rows: readonly ChainTableRow[]): ChainGroup[] {
  const groups = new Map<string, ChainGroup>()
  for (const row of rows) {
    let group = groups.get(row.groupId)
    if (!group) {
      group = { id: row.groupId, label: row.groupLabel, rows: [] }
      groups.set(row.groupId, group)
    }
    group.rows.push(row)
  }
  return [...groups.values()]
}

const compare = (a: string, b: string): number =>
  a.localeCompare(b, undefined, { numeric: true, sensitivity: "base" })

export function sortGroups(
  groups: ChainGroup[],
  sort: ChainSort,
  textOf: TextOf
): ChainGroup[] {
  const sign = sort.dir === "asc" ? 1 : -1
  const rowCmp = (a: ChainTableRow, b: ChainTableRow): number => {
    switch (sort.key) {
      case "kind":
        return compare(a.kind, b.kind) || compare(a.chainId, b.chainId)
      case "from":
        return (
          compare(textOf(a.from), textOf(b.from)) ||
          compare(a.chainId, b.chainId)
        )
      case "to":
        return (
          compare(textOf(a.to), textOf(b.to)) || compare(a.chainId, b.chainId)
        )
      default:
        return compare(a.chainId, b.chainId)
    }
  }
  const sortedRows = (g: ChainGroup): ChainGroup => ({
    ...g,
    rows: ["kind", "from", "to"].includes(sort.key)
      ? [...g.rows].sort((a, b) => sign * rowCmp(a, b))
      : g.rows,
  })
  const withRows = groups.map(sortedRows)
  if (sort.key === "group")
    return withRows.sort((a, b) => sign * compare(a.label, b.label))
  if (sort.key === "count")
    return withRows.sort(
      (a, b) =>
        sign * (a.rows.length - b.rows.length) || compare(a.label, b.label)
    )
  return withRows.sort((a, b) => compare(a.label, b.label))
}

/** Rows for the virtualizer: expanded groups reveal their chains, expanded chains reveal both ends. */
export function flattenChains(
  groups: readonly ChainGroup[],
  expanded: ReadonlySet<string>
): FlatRow[] {
  const out: FlatRow[] = []
  for (const group of groups) {
    const gKey = `g:${group.id}`
    const gOpen = expanded.has(gKey)
    out.push({ level: 1, key: gKey, group, expanded: gOpen })
    if (!gOpen) continue
    for (const row of group.rows) {
      const cKey = `c:${row.chainId}`
      const cOpen = expanded.has(cKey)
      out.push({ level: 2, key: cKey, row, groupId: group.id, expanded: cOpen })
      if (!cOpen) continue
      out.push(
        {
          level: 3,
          key: `${cKey}:from`,
          nodeId: row.from,
          role: "from",
          kind: row.kind,
          chainId: row.chainId,
        },
        {
          level: 3,
          key: `${cKey}:to`,
          nodeId: row.to,
          role: "to",
          kind: row.kind,
          chainId: row.chainId,
        }
      )
    }
  }
  return out
}

/** Quote a CSV field when it holds a separator, quote or newline; neutralise formula-leading characters. */
export function csvField(value: string): string {
  const safe = /^[=+\-@\t\r]/.test(value) ? `'${value}` : value
  return /[",\n\r]/.test(safe) ? `"${safe.replace(/"/g, '""')}"` : safe
}

const CSV_HEADER = [
  "group",
  "chain",
  "kind",
  "from_id",
  "from_text",
  "to_id",
  "to_text",
] as const

export function chainsToCsv(
  groups: readonly ChainGroup[],
  textOf: TextOf
): string {
  const lines: string[] = [CSV_HEADER.join(",")]
  for (const group of groups)
    for (const row of group.rows)
      lines.push(
        [
          group.label,
          row.chainId,
          row.kind,
          row.from,
          textOf(row.from),
          row.to,
          textOf(row.to),
        ]
          .map(csvField)
          .join(",")
      )
  return `${lines.join("\n")}\n`
}
