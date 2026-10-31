// Pure camera math for the graph canvas. screen = world * k + (x, y); the same convention as d3-zoom's
// ZoomTransform, so gesture output feeds straight in. Nothing here touches the DOM.

export type Cam = { x: number; y: number; k: number }
export type Bounds = { minX: number; minY: number; maxX: number; maxY: number }
export type Size = { w: number; h: number }

export const IDENTITY: Cam = { x: 0, y: 0, k: 1 }

/** Wheel zoom lerps toward its target by this fraction per 60 Hz frame (docs/web/05 #19). */
export const WHEEL_LERP = 0.2
/** Pan inertia: velocity multiplier per 60 Hz frame. */
export const INERTIA_FRICTION = 0.92
/** Below this speed (px/frame) inertia stops. */
const INERTIA_MIN_SPEED = 0.15
/** Double-click zoom factor. */
export const DOUBLE_CLICK_ZOOM = 1.8
/** Fit and fly-to leave this much room around the content, in px. */
export const FIT_PADDING = 56

const FRAME_MS = 1000 / 60

export function screenToWorld(cam: Cam, sx: number, sy: number): [number, number] {
  return [(sx - cam.x) / cam.k, (sy - cam.y) / cam.k]
}

export function worldToScreen(cam: Cam, wx: number, wy: number): [number, number] {
  return [wx * cam.k + cam.x, wy * cam.k + cam.y]
}

/** The world-space rectangle currently visible in a viewport of `size`, grown by `margin` px. */
export function viewBounds(cam: Cam, size: Size, margin = 0): Bounds {
  const [minX, minY] = screenToWorld(cam, -margin, -margin)
  const [maxX, maxY] = screenToWorld(cam, size.w + margin, size.h + margin)
  return { minX, minY, maxX, maxY }
}

export function clampK(k: number, [min, max]: readonly [number, number]): number {
  return Math.min(max, Math.max(min, k))
}

/** Camera that shows `bounds` centered with `padding` px around, never zoomed past `maxK`. */
export function fitCam(
  bounds: Bounds,
  size: Size,
  padding = FIT_PADDING,
  maxK = 4
): Cam {
  const bw = Math.max(bounds.maxX - bounds.minX, 1)
  const bh = Math.max(bounds.maxY - bounds.minY, 1)
  const availW = Math.max(size.w - padding * 2, 1)
  const availH = Math.max(size.h - padding * 2, 1)
  const k = Math.min(availW / bw, availH / bh, maxK)
  const cx = (bounds.minX + bounds.maxX) / 2
  const cy = (bounds.minY + bounds.maxY) / 2
  return { k, x: size.w / 2 - cx * k, y: size.h / 2 - cy * k }
}

/** Scales by `factor` while keeping the world point under screen point (px, py) fixed. */
export function zoomAbout(
  cam: Cam,
  px: number,
  py: number,
  factor: number,
  extent: readonly [number, number]
): Cam {
  const k = clampK(cam.k * factor, extent)
  const ratio = k / cam.k
  return { k, x: px - (px - cam.x) * ratio, y: py - (py - cam.y) * ratio }
}

/** Camera with world point (wx, wy) at the viewport center, at scale `k`. */
export function centerOn(wx: number, wy: number, k: number, size: Size): Cam {
  return { k, x: size.w / 2 - wx * k, y: size.h / 2 - wy * k }
}

/** True when the world point sits further than `fraction` of the viewport from its center. */
export function isOffCenter(
  cam: Cam,
  wx: number,
  wy: number,
  size: Size,
  fraction: number
): boolean {
  const [sx, sy] = worldToScreen(cam, wx, wy)
  return (
    Math.abs(sx - size.w / 2) > size.w * fraction ||
    Math.abs(sy - size.h / 2) > size.h * fraction
  )
}

export function camEquals(a: Cam, b: Cam, eps = 0.01): boolean {
  return (
    Math.abs(a.x - b.x) < eps &&
    Math.abs(a.y - b.y) < eps &&
    Math.abs(a.k / b.k - 1) < eps / 100
  )
}

/**
 * Frame-rate independent lerp: `rate` is the per-60Hz-frame fraction, `dtMs` the elapsed time.
 * Position lerps linearly; scale lerps in log space so zooming feels uniform.
 */
export function smoothToward(
  current: Cam,
  target: Cam,
  rate: number,
  dtMs: number
): Cam {
  const alpha = 1 - Math.pow(1 - rate, Math.max(dtMs, 0) / FRAME_MS)
  const k = current.k * Math.pow(target.k / current.k, alpha)
  return {
    k,
    x: current.x + (target.x - current.x) * alpha,
    y: current.y + (target.y - current.y) * alpha,
  }
}

export function lerpCam(a: Cam, b: Cam, t: number): Cam {
  return {
    k: a.k * Math.pow(b.k / a.k, t),
    x: a.x + (b.x - a.x) * t,
    y: a.y + (b.y - a.y) * t,
  }
}

export type Velocity = { vx: number; vy: number }

/** One inertia step: moves by the velocity and decays it. Returns null once it has stopped. */
export function inertiaStep(
  v: Velocity,
  dtMs: number,
  friction = INERTIA_FRICTION
): { dx: number; dy: number; next: Velocity } | null {
  const frames = Math.max(dtMs, 0) / FRAME_MS
  const decay = Math.pow(friction, frames)
  const next = { vx: v.vx * decay, vy: v.vy * decay }
  // Distance travelled over `frames` frames of geometric decay.
  const travel = frames === 0 ? 0 : (1 - decay) / (1 - friction)
  if (Math.hypot(next.vx, next.vy) < INERTIA_MIN_SPEED) return null
  return { dx: v.vx * travel, dy: v.vy * travel, next }
}

/** Exponentially weighted velocity (px per 60 Hz frame) from successive pointer moves. */
export function blendVelocity(
  prev: Velocity,
  dx: number,
  dy: number,
  dtMs: number
): Velocity {
  if (dtMs <= 0) return prev
  const scale = FRAME_MS / dtMs
  const w = 0.35
  return {
    vx: prev.vx * (1 - w) + dx * scale * w,
    vy: prev.vy * (1 - w) + dy * scale * w,
  }
}

const smoothstep = (edge0: number, edge1: number, x: number): number => {
  const t = Math.min(1, Math.max(0, (x - edge0) / (edge1 - edge0)))
  return t * t * (3 - 2 * t)
}

/**
 * Level-of-detail weights from the zoom relative to the fit scale (`z = k / fitK`, docs/web/05 #21):
 * hulls and cluster labels are solid when zoomed out and gone past z = 2.5; node labels take over.
 */
export type Lod = { hulls: number; nodeLabels: number; arrows: number }

export function lodFor(z: number): Lod {
  return {
    hulls: 1 - smoothstep(1.5, 2.5, z),
    nodeLabels: smoothstep(1.3, 2.2, z),
    arrows: smoothstep(2.4, 3.4, z),
  }
}

/**
 * Base node side in screen px at fit zoom: scales with how crowded the viewport is, so 150 nodes get
 * generous squares and 10k get small ones.
 */
export function baseSidePx(count: number, size: Size): number {
  const spacing = Math.sqrt((size.w * size.h) / Math.max(count, 1))
  return Math.min(12, Math.max(4, spacing * 0.42))
}

/** Screen side (px) of a node square: grows gently with relative zoom `z`, clamped to stay legible. */
export function nodeScreenSide(basePx: number, weight: number, z: number): number {
  return Math.min(Math.max(basePx * weight * Math.pow(Math.max(z, 0.05), 0.62), 3), 34)
}

/** Fly-to duration in ms: short hops are quick, long ones cap at 500 ms (docs/web/05 #18). */
export function flyDuration(from: Cam, to: Cam, size: Size): number {
  const dist = Math.hypot(to.x - from.x, to.y - from.y) / Math.max(size.w, 1)
  const zoom = Math.abs(Math.log2(to.k / from.k))
  return Math.round(Math.min(500, 260 + dist * 160 + zoom * 40))
}
