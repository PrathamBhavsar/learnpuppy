import { describe, expect, it } from "vitest"

import {
  INERTIA_FRICTION,
  baseSidePx,
  blendVelocity,
  camEquals,
  centerOn,
  clampK,
  fitCam,
  flyDuration,
  inertiaStep,
  isOffCenter,
  lerpCam,
  lodFor,
  nodeScreenSide,
  screenToWorld,
  smoothToward,
  viewBounds,
  worldToScreen,
  zoomAbout,
} from "./camera"

const size = { w: 1000, h: 600 }
const extent = [0.1, 20] as const

describe("camera transforms", () => {
  it("round-trips screen and world coordinates", () => {
    const cam = { x: 120, y: -40, k: 2.5 }
    const [wx, wy] = screenToWorld(cam, 333, 222)
    const [sx, sy] = worldToScreen(cam, wx, wy)
    expect(sx).toBeCloseTo(333)
    expect(sy).toBeCloseTo(222)
  })

  it("computes the visible world rectangle with a margin", () => {
    const b = viewBounds({ x: 0, y: 0, k: 2 }, size, 20)
    expect(b).toEqual({ minX: -10, minY: -10, maxX: 510, maxY: 310 })
  })

  it("clamps scale into the extent", () => {
    expect(clampK(0.01, extent)).toBe(0.1)
    expect(clampK(99, extent)).toBe(20)
    expect(clampK(3, extent)).toBe(3)
  })
})

describe("fitCam", () => {
  it("centers the bounds and leaves padding", () => {
    const cam = fitCam({ minX: 0, minY: 0, maxX: 800, maxY: 400 }, size, 50)
    expect(cam.k).toBeCloseTo(900 / 800)
    const [cx, cy] = worldToScreen(cam, 400, 200)
    expect(cx).toBeCloseTo(500)
    expect(cy).toBeCloseTo(300)
  })

  it("is limited by the tighter axis", () => {
    const cam = fitCam({ minX: 0, minY: 0, maxX: 100, maxY: 1000 }, size, 50)
    expect(cam.k).toBeCloseTo(500 / 1000)
  })

  it("never zooms past maxK for tiny content", () => {
    expect(fitCam({ minX: 0, minY: 0, maxX: 1, maxY: 1 }, size, 50, 4).k).toBe(
      4
    )
  })
})

describe("zoomAbout", () => {
  it("keeps the point under the cursor fixed", () => {
    const cam = { x: 50, y: 30, k: 1.2 }
    const [wx, wy] = screenToWorld(cam, 400, 250)
    const next = zoomAbout(cam, 400, 250, 1.8, extent)
    const [sx, sy] = worldToScreen(next, wx, wy)
    expect(sx).toBeCloseTo(400)
    expect(sy).toBeCloseTo(250)
    expect(next.k).toBeCloseTo(1.2 * 1.8)
  })

  it("respects the extent without drifting", () => {
    const cam = { x: 0, y: 0, k: 19 }
    const next = zoomAbout(cam, 100, 100, 5, extent)
    expect(next.k).toBe(20)
    const [wx] = screenToWorld(cam, 100, 100)
    expect(worldToScreen(next, wx, 0)[0]).toBeCloseTo(100)
  })
})

describe("centering", () => {
  it("centerOn puts the world point in the middle", () => {
    const cam = centerOn(300, 200, 2, size)
    expect(worldToScreen(cam, 300, 200)).toEqual([500, 300])
  })

  it("detects a node off-center by more than a fraction of the viewport", () => {
    const cam = { x: 0, y: 0, k: 1 }
    expect(isOffCenter(cam, 500, 300, size, 0.35)).toBe(false)
    expect(isOffCenter(cam, 900, 300, size, 0.35)).toBe(true)
    expect(isOffCenter(cam, 500, 590, size, 0.35)).toBe(true)
  })
})

describe("smoothing", () => {
  it("covers one 60 Hz step of lerp in one frame", () => {
    const next = smoothToward(
      { x: 0, y: 0, k: 1 },
      { x: 100, y: 0, k: 1 },
      0.2,
      1000 / 60
    )
    expect(next.x).toBeCloseTo(20)
  })

  it("is frame-rate independent", () => {
    const target = { x: 100, y: 0, k: 4 }
    let a = { x: 0, y: 0, k: 1 }
    for (let i = 0; i < 6; i++) a = smoothToward(a, target, 0.2, 1000 / 60)
    const b = smoothToward({ x: 0, y: 0, k: 1 }, target, 0.2, 6 * (1000 / 60))
    expect(b.x).toBeCloseTo(a.x, 5)
    expect(b.k).toBeCloseTo(a.k, 5)
  })

  it("scales in log space so zooming feels uniform", () => {
    const mid = smoothToward(
      { x: 0, y: 0, k: 1 },
      { x: 0, y: 0, k: 16 },
      0.5,
      1000 / 60
    )
    expect(mid.k).toBeCloseTo(4)
  })

  it("converges and reports equality", () => {
    let cam = { x: 0, y: 0, k: 1 }
    const target = { x: 40, y: -20, k: 2 }
    for (let i = 0; i < 60; i++) cam = smoothToward(cam, target, 0.2, 1000 / 60)
    expect(camEquals(cam, target, 0.05)).toBe(true)
  })

  it("lerpCam interpolates scale geometrically", () => {
    expect(lerpCam({ x: 0, y: 0, k: 1 }, { x: 10, y: 0, k: 4 }, 0.5)).toEqual({
      x: 5,
      y: 0,
      k: 2,
    })
  })
})

describe("inertia", () => {
  it("decays by the friction per 60 Hz frame (0.92)", () => {
    const step = inertiaStep({ vx: 10, vy: 0 }, 1000 / 60)
    expect(step?.next.vx).toBeCloseTo(10 * INERTIA_FRICTION)
    expect(step?.dx).toBeCloseTo(10)
  })

  it("travels the geometric sum over longer steps", () => {
    const one = inertiaStep({ vx: 10, vy: 0 }, 1000 / 60)
    const two = inertiaStep(one?.next ?? { vx: 0, vy: 0 }, 1000 / 60)
    const both = inertiaStep({ vx: 10, vy: 0 }, 2 * (1000 / 60))
    expect((one?.dx ?? 0) + (two?.dx ?? 0)).toBeCloseTo(both?.dx ?? -1, 5)
  })

  it("stops once slow enough", () => {
    expect(inertiaStep({ vx: 0.05, vy: 0.05 }, 16)).toBeNull()
  })

  it("blends pointer deltas into a per-frame velocity", () => {
    const v = blendVelocity({ vx: 0, vy: 0 }, 16.667, 0, 16.667)
    expect(v.vx).toBeGreaterThan(0)
    expect(v.vx).toBeLessThan(16.667)
    expect(blendVelocity({ vx: 3, vy: 3 }, 5, 5, 0)).toEqual({ vx: 3, vy: 3 })
  })
})

describe("level of detail", () => {
  it("shows hulls zoomed out and removes them past 2.5x the fit zoom", () => {
    expect(lodFor(1).hulls).toBe(1)
    expect(lodFor(2.5).hulls).toBe(0)
    expect(lodFor(4).hulls).toBe(0)
    const mid = lodFor(2).hulls
    expect(mid).toBeGreaterThan(0)
    expect(mid).toBeLessThan(1)
  })

  it("brings node labels in as hulls leave", () => {
    expect(lodFor(1).nodeLabels).toBe(0)
    expect(lodFor(2.2).nodeLabels).toBe(1)
    expect(lodFor(1.75).nodeLabels).toBeGreaterThan(0)
  })

  it("hulls fade monotonically", () => {
    let prev = 1
    for (let z = 1; z <= 3; z += 0.1) {
      const h = lodFor(z).hulls
      expect(h).toBeLessThanOrEqual(prev + 1e-9)
      prev = h
    }
  })
})

describe("node sizing", () => {
  it("scales the base size by crowding", () => {
    expect(baseSidePx(150, size)).toBe(12)
    expect(baseSidePx(100_000, size)).toBe(4)
    expect(baseSidePx(5000, size)).toBeGreaterThan(4)
  })

  it("grows with weight and zoom but stays clamped", () => {
    expect(nodeScreenSide(8, 1, 1)).toBeCloseTo(8)
    expect(nodeScreenSide(8, 2, 1)).toBeCloseTo(16)
    expect(nodeScreenSide(8, 1, 4)).toBeGreaterThan(8)
    expect(nodeScreenSide(8, 1, 0.0001)).toBe(3)
    expect(nodeScreenSide(12, 2.2, 500)).toBe(34)
  })
})

describe("flyDuration", () => {
  it("is quick for short hops and capped at 500 ms", () => {
    const from = { x: 0, y: 0, k: 1 }
    const short = flyDuration(from, { x: 20, y: 0, k: 1 }, size)
    const far = flyDuration(from, { x: 90000, y: 0, k: 40 }, size)
    expect(short).toBeLessThan(300)
    expect(far).toBe(500)
  })
})
