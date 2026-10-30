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
  const rows = files.map((name) => {
    const gz = gzipSync(readFileSync(join(DIST, "assets", name))).length
    return { name, gz, budget: budgetFor(name, entry) }
  })
  rows.sort((a, b) => b.gz - a.gz)
  for (const r of rows) {
    const flag = r.gz > r.budget ? "OVER" : "ok  "
    console.log(
      `${flag} ${(r.gz / KB).toFixed(1).padStart(7)} kB gzip / ${(r.budget / KB).toFixed(0)} kB  ${r.name}${r.name === entry ? "  (entry)" : ""}`
    )
  }
  const over = rows.filter((r) => r.gz > r.budget)
  if (over.length > 0) {
    console.error(`bundle-size: ${over.length} chunk(s) over budget`)
    process.exit(1)
  }
  console.log(`bundle-size: ${rows.length} chunks within budget`)
}
