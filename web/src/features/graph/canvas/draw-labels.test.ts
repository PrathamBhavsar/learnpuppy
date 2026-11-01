import { describe, expect, it } from "vitest"

import { sceneOf } from "../model/test-snapshot"

import type { FrameState } from "./frame"
import { drawNodeLabels } from "./draw-labels"
import type { CanvasTheme } from "./theme"

const theme: CanvasTheme = {
  background: "#000",
  foreground: "#fff",
  muted: "#222",
  mutedForeground: "#888",
  primary: "#a0f",
  destructive: "#f00",
  success: "#0f0",
  border: "#333",
  sources: ["#1", "#2", "#3", "#4", "#5"],
  clusters: Array.from({ length: 8 }, () => "#999"),
  font: "monospace",
}

/** A recording 2D context: 6px per character, and every filled string is logged. */
function fakeContext() {
  const drawn: string[] = []
  const ctx = {
    font: "",
    textAlign: "",
    textBaseline: "",
    lineJoin: "",
    lineWidth: 0,
    globalAlpha: 1,
    strokeStyle: "",
    fillStyle: "",
    measureText: (t: string) => ({ width: t.length * 6 }),
    strokeText: () => {},
    fillText: (t: string) => drawn.push(t),
  } as unknown as CanvasRenderingContext2D
  return { ctx, drawn }
}

// Five nodes 8px apart: every label overlaps its neighbours.
const crowded = () =>
  sceneOf(
    [0, 1, 2, 3, 4].map((i) => ({
      id: `n${i}`,
      x: i * 8,
      y: 0,
      text: `Proposition number ${i}`,
    }))
  )

function frame(over: Partial<FrameState>): FrameState {
  const scene = crowded()
  const n = scene.count
  return {
    size: { w: 400, h: 300 },
    dpr: 1,
    cam: { x: 100, y: 150, k: 1 },
    z: 1,
    theme,
    scene,
    px: Float32Array.from(scene.ids.map((_, i) => i * 8)),
    py: new Float32Array(n),
    basePx: 6,
    visible: null,
    match: null,
    matchList: null,
    selected: new Uint8Array(n),
    selectedAny: false,
    activeIdx: -1,
    hoverIdx: -1,
    hoverMix: 0,
    neighbour: null,
    special: new Uint8Array(n),
    clusterAlpha: null,
    edgesAlpha: 1,
    hullFade: 1,
    births: [],
    dying: [],
    pulses: [],
    deleteTint: null,
    now: 0,
    hulls: true,
    labels: "never",
    performance: false,
    interacting: false,
    ...over,
  }
}

describe("drawNodeLabels", () => {
  it("draws nothing when labels are off and nothing has focus", () => {
    const { ctx, drawn } = fakeContext()
    drawNodeLabels(ctx, frame({}))
    expect(drawn).toEqual([])
  })

  it("does not pile labels on top of each other for a large selection", () => {
    const { ctx, drawn } = fakeContext()
    drawNodeLabels(
      ctx,
      frame({ selectedAny: true, selected: new Uint8Array([1, 1, 1, 1, 1]) })
    )
    expect(drawn.length).toBeGreaterThanOrEqual(1)
    expect(drawn.length).toBeLessThan(5)
  })

  it("always draws the hovered and the active label, even when they collide", () => {
    const { ctx, drawn } = fakeContext()
    drawNodeLabels(ctx, frame({ hoverIdx: 2, hoverMix: 1, activeIdx: 3 }))
    expect(drawn).toEqual(["Proposition number 2", "Proposition number 3"])
  })

  it("draws the hovered label first, so selected neighbours yield to it", () => {
    const { ctx, drawn } = fakeContext()
    drawNodeLabels(
      ctx,
      frame({
        hoverIdx: 1,
        hoverMix: 1,
        selectedAny: true,
        selected: new Uint8Array([1, 1, 1, 1, 1]),
      })
    )
    expect(drawn[0]).toBe("Proposition number 1")
    expect(new Set(drawn).size).toBe(drawn.length)
  })

  it("skips every label in performance mode", () => {
    const { ctx, drawn } = fakeContext()
    drawNodeLabels(ctx, frame({ performance: true, hoverIdx: 2, hoverMix: 1 }))
    expect(drawn).toEqual([])
  })

  it("truncates long text with an ellipsis", () => {
    const long = "x".repeat(80)
    const scene = sceneOf([{ id: "a", x: 0, y: 0, text: long }])
    const { ctx, drawn } = fakeContext()
    drawNodeLabels(
      ctx,
      frame({
        scene,
        px: new Float32Array([0]),
        py: new Float32Array([0]),
        selected: new Uint8Array([1]),
        selectedAny: true,
        activeIdx: 0,
      })
    )
    expect(drawn).toHaveLength(1)
    expect(drawn[0]).toHaveLength(38)
    expect(drawn[0]?.endsWith("…")).toBe(true)
  })
})
