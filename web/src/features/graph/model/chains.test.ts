import { describe, expect, it } from "vitest"

import type { ChainTableRow } from "@/api/pending-types"

import {
  type ChainSort,
  chainsToCsv,
  csvField,
  flattenChains,
  groupRows,
  sortGroups,
} from "./chains"

const row = (chainId: string, groupId: string, from: string, to: string, kind: ChainTableRow["kind"] = "supports"): ChainTableRow => ({
  groupId,
  groupLabel: `Session ${groupId}`,
  chainId,
  from,
  to,
  kind,
})
const rows = [
  row("c1", "s2", "n1", "n2", "implies"),
  row("c2", "s1", "n3", "n4"),
  row("c3", "s2", "n5", "n6", "contrasts"),
  row("c4", "s2", "n1", "n7"),
]
const text = (id: string) => `text of ${id}`

describe("grouping and sorting", () => {
  it("groups rows by session in first-seen order", () => {
    const groups = groupRows(rows)
    expect(groups.map((g) => g.id)).toEqual(["s2", "s1"])
    expect(groups[0]?.rows).toHaveLength(3)
  })

  const sorted = (sort: ChainSort) => sortGroups(groupRows(rows), sort, text)

  it("sorts groups by chain count", () => {
    expect(sorted({ key: "count", dir: "desc" }).map((g) => g.id)).toEqual(["s2", "s1"])
    expect(sorted({ key: "count", dir: "asc" }).map((g) => g.id)).toEqual(["s1", "s2"])
  })

  it("sorts groups by label", () => {
    expect(sorted({ key: "group", dir: "asc" }).map((g) => g.id)).toEqual(["s1", "s2"])
    expect(sorted({ key: "group", dir: "desc" }).map((g) => g.id)).toEqual(["s2", "s1"])
  })

  it("sorts chains within a group by kind", () => {
    const g = sorted({ key: "kind", dir: "asc" }).find((x) => x.id === "s2")
    expect(g?.rows.map((r) => r.kind)).toEqual(["contrasts", "implies", "supports"])
  })

  it("sorts chains within a group by endpoint text", () => {
    const g = sorted({ key: "to", dir: "desc" }).find((x) => x.id === "s2")
    expect(g?.rows.map((r) => r.to)).toEqual(["n7", "n6", "n2"])
  })
})

describe("flattenChains", () => {
  const groups = groupRows(rows)

  it("shows only group rows when nothing is expanded", () => {
    const flat = flattenChains(groups, new Set())
    expect(flat.map((r) => r.level)).toEqual([1, 1])
  })

  it("reveals chains of an expanded group, then both propositions of an expanded chain", () => {
    const flat = flattenChains(groups, new Set(["g:s2", "c:c1"]))
    expect(flat.map((r) => r.level)).toEqual([1, 2, 3, 3, 2, 2, 1])
    const level3 = flat.filter((r) => r.level === 3)
    expect(level3.map((r) => (r.level === 3 ? r.role : ""))).toEqual(["from", "to"])
  })

  it("gives every row a unique key", () => {
    const flat = flattenChains(groups, new Set(["g:s2", "g:s1", "c:c1", "c:c2"]))
    expect(new Set(flat.map((r) => r.key)).size).toBe(flat.length)
  })
})

describe("csv", () => {
  it("quotes fields with separators, quotes and newlines", () => {
    expect(csvField("plain")).toBe("plain")
    expect(csvField("a,b")).toBe('"a,b"')
    expect(csvField('say "hi"')).toBe('"say ""hi"""')
    expect(csvField("two\nlines")).toBe('"two\nlines"')
  })

  it("neutralises spreadsheet formula injection", () => {
    expect(csvField("=SUM(A1)")).toBe("'=SUM(A1)")
    expect(csvField("@cmd")).toBe("'@cmd")
    expect(csvField("-5")).toBe("'-5")
  })

  it("exports a header and one line per chain", () => {
    const csv = chainsToCsv(sortGroups(groupRows(rows), { key: "group", dir: "asc" }, text), text)
    const lines = csv.trimEnd().split("\n")
    expect(lines[0]).toBe("group,chain,kind,from_id,from_text,to_id,to_text")
    expect(lines).toHaveLength(1 + rows.length)
    expect(lines[1]).toBe("Session s1,c2,supports,n3,text of n3,n4,text of n4")
  })
})
