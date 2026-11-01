import { lodFor } from "./camera"
import type { FrameState } from "./frame"

/** Cluster silhouettes below the nodes, cross-fading out as the view zooms in (docs/web/05 #21). */
export function drawHulls(ctx: CanvasRenderingContext2D, f: FrameState): void {
  if (!f.hulls || f.performance) return
  const alpha = lodFor(f.z).hulls * f.hullFade
  if (alpha <= 0.01) return
  const { cam, theme, scene } = f
  ctx.lineJoin = "round"
  scene.clusters.forEach((cluster, ci) => {
    const pts = cluster.hull
    if (pts.length < 3) return
    const color =
      theme.clusters[(cluster.slot - 1) % theme.clusters.length] ??
      theme.primary
    const fade = f.clusterAlpha ? (f.clusterAlpha[ci] ?? 1) : 1
    const path = new Path2D()
    const n = pts.length
    const at = (i: number): [number, number] => {
      const p = pts[(i + n) % n]!
      return [p[0] * cam.k + cam.x, p[1] * cam.k + cam.y]
    }
    // Smooth blob: quadratic curves through edge midpoints, with the vertices as control points.
    const [sx, sy] = at(0)
    const [nx, ny] = at(1)
    path.moveTo((sx + nx) / 2, (sy + ny) / 2)
    for (let i = 1; i <= n; i++) {
      const [vx, vy] = at(i)
      const [wx, wy] = at(i + 1)
      path.quadraticCurveTo(vx, vy, (vx + wx) / 2, (vy + wy) / 2)
    }
    path.closePath()
    ctx.fillStyle = color
    ctx.globalAlpha = 0.07 * alpha * fade
    ctx.fill(path)
    ctx.strokeStyle = color
    ctx.lineWidth = 1
    ctx.globalAlpha = 0.34 * alpha * fade
    ctx.stroke(path)
  })
  ctx.globalAlpha = 1
}

/** Cluster names, fading with the hulls they belong to. */
export function drawClusterLabels(
  ctx: CanvasRenderingContext2D,
  f: FrameState
): void {
  if (!f.hulls || f.performance) return
  const alpha = lodFor(f.z).hulls * f.hullFade
  if (alpha <= 0.05) return
  const { cam, theme, scene, size } = f
  ctx.font = `600 11px ${theme.font}`
  ctx.textAlign = "center"
  ctx.textBaseline = "bottom"
  scene.clusters.forEach((cluster, ci) => {
    if (cluster.hull.length < 3) return
    let minY = Infinity
    for (const p of cluster.hull) if (p[1] < minY) minY = p[1]
    const sx = cluster.centroid[0] * cam.k + cam.x
    const sy = minY * cam.k + cam.y - 6
    if (sx < -80 || sx > size.w + 80 || sy < -20 || sy > size.h + 20) return
    const fade = f.clusterAlpha ? (f.clusterAlpha[ci] ?? 1) : 1
    ctx.globalAlpha = alpha * fade * 0.9
    ctx.fillStyle =
      theme.clusters[(cluster.slot - 1) % theme.clusters.length] ??
      theme.foreground
    ctx.fillText(cluster.label.toUpperCase(), sx, sy)
  })
  ctx.globalAlpha = 1
}
