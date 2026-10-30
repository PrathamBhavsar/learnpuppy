import { readFileSync } from "node:fs"
import { join } from "node:path"
import { describe, expect, it } from "vitest"

import { contrast } from "./color.ts"
import { failures } from "./contrast.ts"
import { HUES, buildTokens, hexToOklch, renderTokensCss } from "./hex2oklch.ts"

describe("hex2oklch", () => {
  it("converts known colors", () => {
    expect(hexToOklch("#ffffff").l).toBeCloseTo(1, 2)
    expect(hexToOklch("#000000").l).toBeCloseTo(0, 2)
    const violet = hexToOklch("#a78bfa")
    expect(violet.l).toBeCloseTo(0.709, 2)
    expect(violet.h).toBeGreaterThan(280)
    expect(violet.h).toBeLessThan(300)
  })

  it("keeps src/styles/tokens.css in sync with the generator", () => {
    const onDisk = readFileSync(
      join(process.cwd(), "src/styles/tokens.css"),
      "utf8"
    )
    expect(onDisk).toBe(renderTokensCss())
  })

  it("defines the same tokens for both themes, including the semantic families", () => {
    const { dark, light } = buildTokens()
    expect(Object.keys(light).sort()).toEqual(Object.keys(dark).sort())
    for (const name of [
      "success",
      "warning",
      "info",
      "mastery-0",
      "mastery-4",
      "source-claude",
      "source-codex",
      "source-agy",
      "source-git",
      "cluster-1",
      "cluster-8",
    ]) {
      expect(dark).toHaveProperty(name)
    }
    expect(HUES).toHaveLength(8)
  })

  it("emits dark values on .dark and light on :root, .light", () => {
    const css = renderTokensCss()
    expect(css).toContain(":root, .light {")
    expect(css).toContain(".dark {")
    expect(css).toMatch(/--background: oklch\(0\.14\d? /)
  })
})

describe("contrast gate", () => {
  it("passes for the shipped palette in both themes", () => {
    const { dark, light } = buildTokens()
    expect(failures("dark", dark)).toEqual([])
    expect(failures("light", light)).toEqual([])
  })

  it("fails when a text pair drops below 4.5:1", () => {
    const { dark } = buildTokens()
    const broken = { ...dark, foreground: dark["muted"]! }
    const errors = failures("dark", broken)
    expect(errors.some((e) => e.includes("foreground on background"))).toBe(
      true
    )
  })

  it("fails when destructive text is unreadable on its own tint", () => {
    const { light } = buildTokens()
    const washed = {
      ...light,
      destructive: { ...light["destructive"]!, l: 0.75 },
    }
    expect(
      failures("light", washed).some((e) =>
        e.includes("destructive on destructive/20")
      )
    ).toBe(true)
  })

  it("flags missing tokens instead of passing silently", () => {
    const { dark } = buildTokens()
    const partial = { ...dark }
    delete partial["success"]
    expect(
      failures("dark", partial).some((e) => e.includes("missing token success"))
    ).toBe(true)
  })

  it("computes WCAG ratios (black on white = 21)", () => {
    expect(contrast({ l: 0, c: 0, h: 0 }, { l: 1, c: 0, h: 0 })).toBeCloseTo(
      21,
      0
    )
  })
})
