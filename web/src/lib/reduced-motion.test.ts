/// <reference types="node" />
import { readFileSync } from "node:fs"
import { afterEach, describe, expect, it } from "vitest"

import { applyReduceMotion } from "./theme"

const css = readFileSync("src/styles/motion.css", "utf8")

describe("reduced motion (docs/web/05)", () => {
  afterEach(() => applyReduceMotion(false))

  it("CSS collapses durations for the OS preference and the Settings override", () => {
    const os = css.slice(css.indexOf("@media (prefers-reduced-motion"))
    expect(os).toMatch(/animation-duration:\s*0\.01ms\s*!important/)
    expect(os).toMatch(/transition-duration:\s*0\.01ms\s*!important/)
    expect(css).toMatch(/:root\[data-reduce-motion="true"\] \*/)
    expect(css).toMatch(/\.lp-shimmer::after\s*\{\s*display:\s*none/)
  })

  it("the Settings toggle sets and clears the root attribute", () => {
    applyReduceMotion(true)
    expect(document.documentElement.dataset.reduceMotion).toBe("true")
    applyReduceMotion(false)
    expect(document.documentElement.dataset.reduceMotion).toBeUndefined()
  })
})
