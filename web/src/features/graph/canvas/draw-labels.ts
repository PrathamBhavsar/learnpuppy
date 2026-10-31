import { lodFor, nodeScreenSide } from "./camera"
import type { FrameState } from "./frame"
import type { Scene } from "../model/scene"

const MAX_LABEL_CHARS = 38
const MAX_AUTO_LABELS = 70
const LINE_HEIGHT = 14
const widthCache = new WeakMap<Scene, Map<number, number>>()

function truncate(text: string, max = MAX_LABEL_CHARS): string {
  return text.length > max ? `${text.slice(0, max - 1).trimEnd()}…` : text
}

type Placed = { x0: number; y0: number; x1: number; y1: number }

/** Coarse occupancy grid so greedy label placement is O(labels), not O(labels²). */
class Occupancy {
  private cells = new Map<number, Placed[]>()
  private readonly cell = 96
  private keys(r: Placed): number[] {
    const out: number[] = []
    for (let cx = Math.floor(r.x0 / this.cell); cx <= Math.floor(r.x1 / this.cell); cx++)
      for (let cy = Math.floor(r.y0 / this.cell); cy <= Math.floor(r.y1 / this.cell); cy++)
        out.push(cx * 100003 + cy)
    return out
  }
  hits(r: Placed): boolean {
    for (const key of this.keys(r))
      for (const o of this.cells.get(key) ?? [])
        if (r.x0 < o.x1 && r.x1 > o.x0 && r.y0 < o.y1 && r.y1 > o.y0) return true
    return false
  }
  add(r: Placed): void {
    for (const key of this.keys(r)) {
      const list = this.cells.get(key)
      if (list) list.push(r)
      else this.cells.set(key, [r])
    }
  }
}

/**
 * Node labels with a priority order: hovered, active, selected, search matches, then the most
 * connected nodes while there is room. Auto mode fades them in as the cluster hulls fade out.
 */
export function drawNodeLabels(ctx: CanvasRenderingContext2D, f: FrameState): void {
  if (f.performance) return
  const { scene, px, py, cam, size, theme } = f
  const lod = lodFor(f.z)
  const bulk = f.labels === "never" || f.interacting ? 0 : f.labels === "always" ? 1 : lod.nodeLabels
  const hasFocus = f.hoverIdx >= 0 || f.activeIdx >= 0 || f.selectedAny || f.matchList !== null
  if (bulk <= 0.02 && !hasFocus) return

  ctx.font = `11px ${theme.font}`
  ctx.textAlign = "start"
  ctx.textBaseline = "middle"
  ctx.lineJoin = "round"
  ctx.lineWidth = 3.5
  let widths = widthCache.get(scene)
  if (!widths) widthCache.set(scene, (widths = new Map()))

  const occupied = new Occupancy()
  const placed = new Set<number>()

  // `strong` draws in the foreground colour; `force` skips collision avoidance (hovered and active only,
  // so a large selection or match set never piles labels on top of each other).
  const place = (i: number, alpha: number, strong: boolean, force = false): boolean => {
    if (placed.has(i) || (f.visible && !f.visible[i])) return false
    const sx = px[i]! * cam.k + cam.x
    const sy = py[i]! * cam.k + cam.y
    if (sx < -10 || sy < -10 || sx > size.w + 10 || sy > size.h + 10) return false
    let w = widths.get(i)
    if (w === undefined) {
      w = ctx.measureText(truncate(scene.texts[i]!)).width
      widths.set(i, w)
    }
    const side = nodeScreenSide(f.basePx, scene.weight[i]!, f.z)
    const x0 = sx + side / 2 + 5
    const rect = { x0: x0 - 2, y0: sy - LINE_HEIGHT / 2, x1: x0 + w + 2, y1: sy + LINE_HEIGHT / 2 }
    if (!force && occupied.hits(rect)) return false
    occupied.add(rect)
    placed.add(i)
    const text = truncate(scene.texts[i]!)
    ctx.globalAlpha = alpha * 0.9
    ctx.strokeStyle = theme.background
    ctx.strokeText(text, x0, sy)
    ctx.globalAlpha = alpha
    ctx.fillStyle = strong ? theme.foreground : theme.mutedForeground
    ctx.fillText(text, x0, sy)
    return true
  }

  if (f.hoverIdx >= 0) place(f.hoverIdx, Math.max(f.hoverMix, 0.001), true, true)
  if (f.activeIdx >= 0) place(f.activeIdx, 1, true, true)
  if (f.selectedAny) {
    let n = 0
    for (let i = 0; i < scene.count && n < 12; i++)
      if (f.selected[i] && place(i, 1, true)) n++
  }
  if (f.matchList) {
    let n = 0
    for (const i of f.matchList) {
      if (n >= 24) break
      if (place(i, 1, true)) n++
    }
  }
  if (bulk > 0.02) {
    let accepted = 0
    const limit = f.labels === "always" ? MAX_AUTO_LABELS * 2 : MAX_AUTO_LABELS
    for (let o = 0; o < scene.order.length && accepted < limit && o < 1500; o++) {
      const i = scene.order[o]!
      if (f.match && !f.match[i]) continue
      if (place(i, bulk, false)) accepted++
    }
  }
  ctx.globalAlpha = 1
}
