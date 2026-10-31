import { lodFor, nodeScreenSide } from "./camera"
import { HOVER_DIM, type FrameState } from "./frame"

/** Past this many on-screen edges, curves fall back to straight lines. */
const CURVE_LIMIT = 2200
const ARROW_LIMIT = 900

type Buckets = {
  base: Path2D
  lo: Path2D
  hi: Path2D
  sel: Path2D
  faint: Path2D
  arrows: Path2D
}

export function drawEdges(ctx: CanvasRenderingContext2D, f: FrameState): void {
  const { scene, px, py, cam, size, visible, theme } = f
  if (scene.edgeCount === 0 || f.edgesAlpha <= 0) return
  const k = cam.k
  const ox = cam.x
  const oy = cam.y
  const margin = 24
  const w = size.w + margin
  const h = size.h + margin
  const buckets: Buckets = {
    base: new Path2D(),
    lo: new Path2D(),
    hi: new Path2D(),
    sel: new Path2D(),
    faint: new Path2D(),
    arrows: new Path2D(),
  }
  const hoverActive = f.hoverIdx >= 0 && f.hoverMix > 0.001
  const searching = f.match !== null
  const lod = lodFor(f.z)
  const simple = f.performance || f.interacting

  // Count first so curve / arrow decisions are made for the whole frame, not edge by edge.
  const onScreen: number[] = []
  for (let e = 0; e < scene.edgeCount; e++) {
    const a = scene.edgeFrom[e]!
    const b = scene.edgeTo[e]!
    if (visible && (!visible[a] || !visible[b])) continue
    const ax = px[a]! * k + ox
    const ay = py[a]! * k + oy
    const bx = px[b]! * k + ox
    const by = py[b]! * k + oy
    if ((ax < -margin && bx < -margin) || (ax > w && bx > w)) continue
    if ((ay < -margin && by < -margin) || (ay > h && by > h)) continue
    onScreen.push(e)
  }
  const curved = !simple && onScreen.length <= CURVE_LIMIT
  const arrows = !simple && lod.arrows > 0.05 && onScreen.length <= ARROW_LIMIT

  for (const e of onScreen) {
    const a = scene.edgeFrom[e]!
    const b = scene.edgeTo[e]!
    const ax = px[a]! * k + ox
    const ay = py[a]! * k + oy
    const bx = px[b]! * k + ox
    const by = py[b]! * k + oy

    let bucket: Path2D = buckets.base
    if (hoverActive && (a === f.hoverIdx || b === f.hoverIdx)) bucket = buckets.hi
    else if (f.selectedAny && (f.selected[a] || f.selected[b])) bucket = buckets.sel
    else if (searching && !(f.match![a] || f.match![b])) bucket = buckets.faint
    else if (hoverActive) bucket = buckets.lo

    bucket.moveTo(ax, ay)
    if (curved) {
      // Cubic with both controls pushed to the same side of the chord: a gentle, stable S-less bow.
      const dx = bx - ax
      const dy = by - ay
      const bow = 0.14
      const nx = -dy * bow
      const ny = dx * bow
      bucket.bezierCurveTo(ax + dx / 3 + nx, ay + dy / 3 + ny, ax + (2 * dx) / 3 + nx, ay + (2 * dy) / 3 + ny, bx, by)
    } else {
      bucket.lineTo(bx, by)
    }

    if (arrows) {
      const len = Math.hypot(bx - ax, by - ay)
      const half = nodeScreenSide(f.basePx, scene.weight[b]!, f.z) / 2
      if (len > half * 4) {
        const ux = (bx - ax) / len
        const uy = (by - ay) / len
        const tipX = bx - ux * (half + 2)
        const tipY = by - uy * (half + 2)
        const size2 = 5
        buckets.arrows.moveTo(tipX, tipY)
        buckets.arrows.lineTo(tipX - ux * size2 - uy * size2 * 0.55, tipY - uy * size2 + ux * size2 * 0.55)
        buckets.arrows.lineTo(tipX - ux * size2 + uy * size2 * 0.55, tipY - uy * size2 - ux * size2 * 0.55)
        buckets.arrows.closePath()
      }
    }
  }

  ctx.lineJoin = "round"
  ctx.lineCap = "butt"
  const baseAlpha = f.performance ? 0.5 : 0.4
  const hoverDim = 1 - f.hoverMix * (1 - HOVER_DIM)

  ctx.strokeStyle = theme.mutedForeground
  ctx.lineWidth = 1
  ctx.globalAlpha = baseAlpha * f.edgesAlpha * (hoverActive ? 1 : 1)
  ctx.stroke(buckets.base)
  if (!f.performance) {
    ctx.globalAlpha = baseAlpha * f.edgesAlpha * hoverDim * 0.5
    ctx.stroke(buckets.lo)
    ctx.globalAlpha = 0.07 * f.edgesAlpha
    ctx.stroke(buckets.faint)
    ctx.strokeStyle = theme.foreground
    ctx.lineWidth = 1.5
    ctx.globalAlpha = 0.8 * f.edgesAlpha
    ctx.stroke(buckets.sel)
    ctx.strokeStyle = theme.primary
    ctx.lineWidth = 2
    ctx.globalAlpha = (0.35 + 0.6 * f.hoverMix) * f.edgesAlpha
    ctx.stroke(buckets.hi)
    if (arrows) {
      ctx.fillStyle = theme.mutedForeground
      ctx.globalAlpha = 0.75 * lod.arrows * f.edgesAlpha
      ctx.fill(buckets.arrows)
    }
  }
  ctx.globalAlpha = 1
}
