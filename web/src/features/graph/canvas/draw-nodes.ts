import { nodeScreenSide } from "./camera"
import { ANIM_MS, HOVER_DIM, SEARCH_DIM, type FrameState } from "./frame"

const N_SOURCES = 5
const KIND_CONCEPT = 1
const KIND_PROCEDURE = 2
const KIND_DECISION = 3

const easeOutBack = (t: number): number => {
  const c1 = 1.70158
  const c3 = c1 + 1
  return 1 + c3 * Math.pow(t - 1, 3) + c1 * Math.pow(t - 1, 2)
}

/** Half-pixel snap keeps 1px strokes crisp on squares. */
const snap = (v: number): number => Math.round(v)

/**
 * Bulk pass: every ordinary node, batched into one Path2D per (cluster fade, dim level, shape, source)
 * so a 5k-node frame is a handful of fill calls instead of 5k.
 */
export function drawNodes(ctx: CanvasRenderingContext2D, f: FrameState): void {
  const { scene, px, py, cam, size, theme, visible } = f
  const groups = f.clusterAlpha ? scene.clusters.length + 1 : 1
  const levels = f.performance ? 1 : 3
  const buckets = new Array<Path2D | undefined>(groups * levels * 2 * N_SOURCES)
  const decorDots = new Path2D()
  const decorRings = new Path2D()
  const hoverActive = f.hoverIdx >= 0 && f.hoverMix > 0.001
  const searching = f.match !== null
  const margin = 20
  const detail = !f.performance && !f.interacting && scene.count <= 2500

  for (let i = 0; i < scene.count; i++) {
    if (f.special[i] || (visible && !visible[i])) continue
    const sx = px[i]! * cam.k + cam.x
    const sy = py[i]! * cam.k + cam.y
    if (sx < -margin || sy < -margin || sx > size.w + margin || sy > size.h + margin) continue

    let level = 0
    if (!f.performance) {
      if (searching && !f.match![i]) level = 2
      else if (hoverActive && !f.neighbour![i]) level = 1
    }
    const group = f.clusterAlpha ? scene.cluster[i]! + 1 : 0
    const hollow = !f.performance && scene.kind[i] === KIND_CONCEPT ? 1 : 0
    const key = ((group * levels + level) * 2 + hollow) * N_SOURCES + scene.source[i]!
    let path = buckets[key]
    if (!path) path = buckets[key] = new Path2D()

    const side = nodeScreenSide(f.basePx, scene.weight[i]!, f.z)
    const half = side / 2
    const x0 = snap(sx - half)
    const y0 = snap(sy - half)
    const s = snap(side)
    path.rect(x0, y0, s, s)

    if (detail && level === 0 && s >= 8) {
      const kind = scene.kind[i]
      if (kind === KIND_PROCEDURE) {
        const d = Math.max(2, snap(s * 0.3))
        decorDots.rect(snap(sx - d / 2), snap(sy - d / 2), d, d)
      } else if (kind === KIND_DECISION) decorRings.rect(x0 - 2.5, y0 - 2.5, s + 5, s + 5)
    }
  }

  const hoverDim = 1 - f.hoverMix * (1 - HOVER_DIM)
  const levelAlpha = [1, hoverDim, SEARCH_DIM * (hoverActive ? hoverDim : 1)]
  for (let key = 0; key < buckets.length; key++) {
    const path = buckets[key]
    if (!path) continue
    const source = key % N_SOURCES
    const hollow = Math.floor(key / N_SOURCES) % 2 === 1
    const level = Math.floor(key / (N_SOURCES * 2)) % levels
    const group = Math.floor(key / (N_SOURCES * 2 * levels))
    const fade = f.clusterAlpha ? (f.clusterAlpha[group - 1 < 0 ? f.clusterAlpha.length - 1 : group - 1] ?? 1) : 1
    const color = theme.sources[source] ?? theme.foreground
    const alpha = levelAlpha[level]! * fade
    if (alpha <= 0.001) continue
    ctx.globalAlpha = hollow ? alpha * 0.2 : alpha
    ctx.fillStyle = color
    ctx.fill(path)
    if (hollow) {
      ctx.globalAlpha = alpha
      ctx.strokeStyle = color
      ctx.lineWidth = 1.25
      ctx.stroke(path)
    }
  }

  if (detail) {
    ctx.globalAlpha = 1
    ctx.fillStyle = theme.background
    ctx.fill(decorDots)
    ctx.strokeStyle = theme.mutedForeground
    ctx.lineWidth = 1
    ctx.globalAlpha = 0.7
    ctx.stroke(decorRings)
  }
  ctx.globalAlpha = 1
}

/** Individually drawn nodes: hovered, selected, newborn, dying and delete-preview nodes. */
export function drawSpecialNodes(ctx: CanvasRenderingContext2D, f: FrameState): void {
  const { scene, px, py, cam, theme } = f
  const drawn = new Set<number>()
  const base = (i: number): { sx: number; sy: number; side: number } => ({
    sx: px[i]! * cam.k + cam.x,
    sy: py[i]! * cam.k + cam.y,
    side: nodeScreenSide(f.basePx, scene.weight[i]!, f.z),
  })
  const square = (i: number, scale: number, alpha: number): void => {
    const { sx, sy, side } = base(i)
    const s = Math.max(side * scale, 0)
    ctx.globalAlpha = alpha
    ctx.fillStyle = theme.sources[scene.source[i]!] ?? theme.foreground
    ctx.fillRect(sx - s / 2, sy - s / 2, s, s)
    if (scene.kind[i] === KIND_CONCEPT) {
      ctx.fillStyle = theme.background
      const inner = Math.max(s - 5, 0)
      ctx.fillRect(sx - inner / 2, sy - inner / 2, inner, inner)
    }
  }
  const ring = (i: number, grow: number, width: number, color: string, alpha: number): void => {
    const { sx, sy, side } = base(i)
    const s = side + grow * 2
    ctx.globalAlpha = alpha
    ctx.strokeStyle = color
    ctx.lineWidth = width
    ctx.strokeRect(sx - s / 2, sy - s / 2, s, s)
  }

  // Births: scale 0→1 with overshoot, and a glow ring that decays over 600 ms.
  for (const { idx, t0 } of f.births) {
    if (f.visible && !f.visible[idx]) continue
    drawn.add(idx)
    const t = Math.min(1, (f.now - t0) / ANIM_MS.birth)
    square(idx, easeOutBack(t), Math.min(1, t * 3))
    const g = Math.min(1, (f.now - t0) / ANIM_MS.glow)
    ring(idx, 3 + g * 10, 2, theme.primary, 0.75 * (1 - g))
  }

  for (const { idx, t0 } of f.dying) {
    drawn.add(idx)
    const t = Math.min(1, (f.now - t0) / ANIM_MS.dying)
    square(idx, 1 - 0.8 * t, 1 - t)
    ring(idx, 2, 1.5, theme.destructive, 1 - t)
  }

  if (f.deleteTint) {
    const pulse = 0.55 + 0.45 * Math.sin(f.now / 140)
    for (let i = 0; i < scene.count; i++) {
      if (!f.deleteTint[i] || drawn.has(i) || (f.visible && !f.visible[i])) continue
      drawn.add(i)
      square(i, 1, 1)
      ring(i, 2.5, 2, theme.destructive, pulse)
      const { sx, sy, side } = base(i)
      ctx.globalAlpha = 0.28 * pulse
      ctx.fillStyle = theme.destructive
      ctx.fillRect(sx - side / 2, sy - side / 2, side, side)
    }
  }

  for (let i = 0; i < scene.count; i++) {
    if (!f.selected[i] || drawn.has(i) || (f.visible && !f.visible[i])) continue
    drawn.add(i)
    square(i, 1, 1)
    ring(i, i === f.activeIdx ? 4 : 3, i === f.activeIdx ? 2 : 1.5, theme.primary, 1)
  }

  if (f.hoverIdx >= 0 && !drawn.has(f.hoverIdx) && !(f.visible && !f.visible[f.hoverIdx])) {
    const i = f.hoverIdx
    drawn.add(i)
    square(i, 1 + 0.15 * f.hoverMix, 1)
    ring(i, 2.5, 1.5, theme.foreground, 0.9 * f.hoverMix)
  } else if (f.hoverIdx >= 0 && f.selected[f.hoverIdx]) {
    ring(f.hoverIdx, 6, 1, theme.foreground, 0.6 * f.hoverMix)
  }

  // Selection pulse: a ring that expands and fades once when a node is picked.
  for (const { idx, t0 } of f.pulses) {
    const t = Math.min(1, (f.now - t0) / ANIM_MS.pulse)
    ring(idx, 4 + t * 12, 1.5, theme.primary, 0.8 * (1 - t))
  }
  ctx.globalAlpha = 1
}

/** Marks search matches with a thin ring so they read at any zoom, up to a sane count. */
export function drawMatchRings(ctx: CanvasRenderingContext2D, f: FrameState): void {
  const list = f.matchList
  if (!list || list.length === 0 || list.length > 300) return
  const { scene, px, py, cam, size } = f
  ctx.strokeStyle = f.theme.primary
  ctx.lineWidth = 1.5
  ctx.globalAlpha = 0.9
  for (const i of list) {
    if (f.visible && !f.visible[i]) continue
    const sx = px[i]! * cam.k + cam.x
    const sy = py[i]! * cam.k + cam.y
    if (sx < -20 || sy < -20 || sx > size.w + 20 || sy > size.h + 20) continue
    const s = nodeScreenSide(f.basePx, scene.weight[i]!, f.z) + 6
    ctx.strokeRect(sx - s / 2, sy - s / 2, s, s)
  }
  ctx.globalAlpha = 1
}
