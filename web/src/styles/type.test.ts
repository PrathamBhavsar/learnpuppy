import { readFileSync } from "node:fs"
import { resolve } from "node:path"
import { describe, expect, it } from "vitest"

const css = readFileSync(resolve(import.meta.dirname, "type.css"), "utf8")

describe("type ladder tokens", () => {
  it("defines one size and line height per rung, 11 to 32 px", () => {
    const rungs = [...css.matchAll(/--text-([a-z0-9]+): (\d+)px;/g)].map(
      (m) => [m[1], Number(m[2])] as const
    )
    expect(rungs).toEqual([
      ["meta", 11],
      ["small", 12],
      ["dense", 13],
      ["body", 14],
      ["lead", 16],
      ["h3", 20],
      ["h2", 24],
      ["h1", 32],
    ])
    for (const [name] of rungs) expect(css).toContain(`--text-${name}-lh:`)
  })

  it("switches --font-prose per reading font and loads the font locally", () => {
    expect(css).toContain('[data-reading-font="serif"]')
    expect(css).toContain('[data-reading-font="mono"]')
    expect(css).toContain("font-display: swap")
    expect(css).toContain("size-adjust")
    expect(css).not.toMatch(/https?:\/\//)
  })
})
