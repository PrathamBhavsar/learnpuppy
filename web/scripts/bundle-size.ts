// Bundle budgets (docs/web/04, 07): entry chunk <= 220 kB gzip, any route chunk <= 120 kB (graph/classroom <= 180 kB).
// Usage: bun scripts/bundle-size.ts   (after `vite build`)
import { readFileSync, readdirSync } from "node:fs"
import { join } from "node:path"
import { gzipSync } from "node:zlib"

const DIST = new URL("../dist", import.meta.url).pathname
const KB = 1024
const ENTRY_BUDGET = 220 * KB
const ROUTE_BUDGET = 120 * KB
const HEAVY_ROUTE_BUDGET = 180 * KB
const HEAVY = /graph|classroom/i

export function budgetFor(name: string, entry: string): number {
  if (name === entry) return ENTRY_BUDGET
  return HEAVY.test(name) ? HEAVY_ROUTE_BUDGET : ROUTE_BUDGET
}

/** Chunks reachable only from a lazy vendor root (mermaid pulls in elk, cytoscape, per-diagram chunks): never on first load, exempt from route budgets. */
function lazyClosure(
  files: string[],
  root: RegExp,
  entry: string
): Set<string> {
  const seen = new Set<string>()
  const queue = files.filter((f) => root.test(f))
  while (queue.length) {
    const f = queue.pop() as string
    if (seen.has(f)) continue
    seen.add(f)
    const src = readFileSync(join(DIST, "assets", f), "utf8")
    for (const g of files)
      if (g !== entry && !seen.has(g) && src.includes(g)) queue.push(g)
  }
  return seen
}

if (import.meta.main) {
  const html = readFileSync(join(DIST, "index.html"), "utf8")
  const entry =
    /<script[^>]+type="module"[^>]+src="\/assets\/([^"]+\.js)"/.exec(html)?.[1]
  if (!entry) {
    console.error(
      "bundle-size: could not find the entry script in dist/index.html"
    )
    process.exit(1)
  }
  const files = readdirSync(join(DIST, "assets")).filter((f) =>
    f.endsWith(".js")
  )
  const exempt = lazyClosure(files, /^mermaid\.core-/, entry)
  const rows = files.map((name) => {
    const gz = gzipSync(readFileSync(join(DIST, "assets", name))).length
    const budget = exempt.has(name) ? Infinity : budgetFor(name, entry)
    return { name, gz, budget }
  })
  rows.sort((a, b) => b.gz - a.gz)
  for (const r of rows) {
    const flag = r.gz > r.budget ? "OVER" : "ok  "
    console.log(
      `${flag} ${(r.gz / KB).toFixed(1).padStart(7)} kB gzip / ${Number.isFinite(r.budget) ? (r.budget / KB).toFixed(0) : "lazy"} kB  ${r.name}${r.name === entry ? "  (entry)" : ""}`
    )
  }
  const over = rows.filter((r) => r.gz > r.budget)
  if (over.length > 0) {
    console.error(`bundle-size: ${over.length} chunk(s) over budget`)
    process.exit(1)
  }
  console.log(`bundle-size: ${rows.length} chunks within budget`)
}
