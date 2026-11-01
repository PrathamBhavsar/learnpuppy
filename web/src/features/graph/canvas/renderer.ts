// One frame of the graph, back to front: hulls, edges, nodes, special nodes, labels.
import { drawEdges } from "./draw-edges"
import { drawClusterLabels, drawHulls } from "./draw-hulls"
import { drawNodeLabels } from "./draw-labels"
import { drawMatchRings, drawNodes, drawSpecialNodes } from "./draw-nodes"
import type { FrameState } from "./frame"

export function renderFrame(
  ctx: CanvasRenderingContext2D,
  f: FrameState
): void {
  ctx.setTransform(f.dpr, 0, 0, f.dpr, 0, 0)
  ctx.clearRect(0, 0, f.size.w, f.size.h)
  drawHulls(ctx, f)
  drawEdges(ctx, f)
  drawNodes(ctx, f)
  drawMatchRings(ctx, f)
  drawSpecialNodes(ctx, f)
  drawClusterLabels(ctx, f)
  drawNodeLabels(ctx, f)
}

/** Soft cluster silhouettes with a sweeping highlight, shown while the server computes the layout. */
export function renderPending(
  ctx: CanvasRenderingContext2D,
  size: { w: number; h: number },
  dpr: number,
  clusterSizes: readonly number[],
  colors: readonly string[],
  now: number,
  animate: boolean
): void {
  ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
  ctx.clearRect(0, 0, size.w, size.h)
  const n = Math.max(clusterSizes.length, 1)
  const total = Math.max(
    clusterSizes.reduce((a, b) => a + b, 0),
    1
  )
  const cx = size.w / 2
  const cy = size.h / 2
  const ring = Math.min(size.w, size.h) * 0.22
  const pulse = animate ? 0.5 + 0.5 * Math.sin(now / 520) : 0.5
  clusterSizes.forEach((count, i) => {
    const angle = (i / n) * Math.PI * 2 - Math.PI / 2
    const r = Math.max(
      28,
      Math.min(size.w, size.h) * 0.16 * Math.sqrt(count / total) * 2.2
    )
    ctx.globalAlpha = 0.06 + 0.05 * pulse
    ctx.fillStyle = colors[i % colors.length] ?? "gray"
    ctx.beginPath()
    ctx.arc(
      cx + Math.cos(angle) * ring,
      cy + Math.sin(angle) * ring,
      r,
      0,
      Math.PI * 2
    )
    ctx.fill()
  })
  ctx.globalAlpha = 1
}
