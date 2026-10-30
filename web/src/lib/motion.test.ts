import { describe, expect, it } from "vitest"

import { dur, ease, spring, staggerDelay } from "./motion"

describe("motion tokens (docs/web/05)", () => {
  it("matches the spec table", () => {
    expect(dur).toEqual({
      instant: 0.08,
      fast: 0.14,
      base: 0.22,
      slow: 0.36,
      hero: 0.6,
    })
    expect(ease.out).toEqual([0.22, 1, 0.36, 1])
    expect(ease.in).toEqual([0.64, 0, 0.78, 0])
    expect(ease.inout).toEqual([0.65, 0, 0.35, 1])
    expect(spring.snappy).toMatchObject({ stiffness: 520, damping: 38 })
    expect(spring.gentle).toMatchObject({ stiffness: 220, damping: 26 })
  })

  it("staggers 24 ms per item, caps at 8 items, and never staggers long lists", () => {
    expect(staggerDelay(0, 5)).toBe(0)
    expect(staggerDelay(3, 5)).toBeCloseTo(0.072)
    expect(staggerDelay(7, 20)).toBeCloseTo(0.168)
    expect(staggerDelay(15, 20)).toBeCloseTo(0.168)
    expect(staggerDelay(10, 31)).toBe(0)
  })
})
