// GraphEngine: the imperative core behind the canvas. It owns the scene arrays, live positions, camera,
// animations and the rAF loop, and renders only when something changed. React sets state on it and
// listens to its events; nothing here re-renders React per frame.
import {
  type Bounds,
  type Cam,
  FIT_PADDING,
  type Size,
  baseSidePx,
  centerOn,
  fitCam,
  flyDuration,
  isOffCenter,
  nodeScreenSide,
  screenToWorld,
  worldToScreen,
} from "./camera"
import { CameraController } from "./camera-controller"
import { ANIM_MS, type FrameState, type LabelMode, type NodeAnim } from "./frame"
import { type HitIndex, buildHitIndex, hitTest, nodesInRect } from "./hit"
import { renderFrame, renderPending } from "./renderer"
import { type CanvasTheme, readCanvasTheme } from "./theme"
import { TweenClock, easings } from "./tween"
import { diffScenes, shouldTween } from "../model/diff"
import { type Scene } from "../model/scene"

export type EngineOptions = {
  hulls: boolean
  labels: LabelMode
  performance: boolean
  reducedMotion: boolean
  followNew: boolean
}

export type Insets = { left: number; right: number; top: number; bottom: number }

export type MarqueeRect = { x: number; y: number; w: number; h: number }

export type EngineEvents = {
  hover: (idx: number) => void
  click: (e: { idx: number; shift: boolean; meta: boolean }) => void
  dblclick: (e: { idx: number }) => void
  marquee: (e: { idx: number[] }) => void
  marqueeRect: (rect: MarqueeRect | null) => void
  camera: (cam: Cam) => void
  births: (e: { idx: number[] }) => void
}

type Listener<K extends keyof EngineEvents> = EngineEvents[K]

/** Fraction of the smaller graph's nodes present in both scenes. */
function sharedFraction(a: Scene, b: Scene): number {
  const [small, big] = a.count <= b.count ? [a, b] : [b, a]
  if (small.count === 0) return 0
  let shared = 0
  for (const id of small.ids) if (big.index.has(id)) shared++
  return shared / small.count
}

const HOVER_MS = 140
const LOAD_STAGGER_MS = 40
const LOAD_CAP_MS = 600
const FRAME_SAMPLES = 240

export class GraphEngine {
  scene: Scene | null = null
  px = new Float32Array(0)
  py = new Float32Array(0)
  readonly camera: CameraController
  size: Size = { w: 1, h: 1 }
  fitK = 1
  hoverIdx = -1

  private readonly ctx: CanvasRenderingContext2D
  private readonly tweens = new TweenClock()
  private readonly listeners = new Map<keyof EngineEvents, Set<(...a: never[]) => void>>()
  private dpr = 1
  private theme: CanvasTheme
  private opts: EngineOptions = { hulls: true, labels: "auto", performance: false, reducedMotion: false, followNew: false }
  private insets: Insets = { left: 0, right: 0, top: 0, bottom: 0 }
  private basePx = 8
  private visible: Uint8Array | null = null
  private match: Uint8Array | null = null
  private matchList: Uint32Array | null = null
  private selected = new Uint8Array(0)
  private selectedAny = false
  private activeIdx = -1
  private special = new Uint8Array(0)
  private neighbour: Uint8Array | null = null
  private hoverMix = 0
  private hoverWanted = -1
  private pendingHover: { x: number; y: number } | null = null
  private clusterAlpha: Float32Array | null = null
  private edgesAlpha = 1
  private hullFade = 1
  private births: NodeAnim[] = []
  private dying: NodeAnim[] = []
  private pulses: NodeAnim[] = []
  private deleteTint: Uint8Array | null = null
  private hit: HitIndex | null = null
  private raf = 0
  private last = 0
  private destroyed = false
  private pendingLayout = false
  private lastCamKey = ""
  private marquee: { sx: number; sy: number; x: number; y: number; active: boolean } | null = null
  private down: { x: number; y: number; t: number; shift: boolean; meta: boolean } | null = null
  private frameCost: number[] = []
  private frameGap: number[] = []
  private frame_: FrameState | null = null
  private pendingSizes: readonly number[] | null = null
  /** The camera still shows the fit view untouched, so a resize should re-fit instead of just re-centering. */
  private pristine = true

  private readonly canvas: HTMLCanvasElement

  constructor(canvas: HTMLCanvasElement) {
    this.canvas = canvas
    const ctx = canvas.getContext("2d", { alpha: true })
    if (!ctx) throw new Error("2D canvas is not available")
    this.ctx = ctx
    this.theme = readCanvasTheme()
    this.camera = new CameraController(canvas, this.tweens, {
      invalidate: () => this.invalidate(),
      reducedMotion: () => this.opts.reducedMotion,
      onUserInput: () => {
        this.pristine = false
      },
    })
    canvas.addEventListener("pointerdown", this.onDown)
    canvas.addEventListener("pointermove", this.onMove)
    canvas.addEventListener("pointerup", this.onUp)
    canvas.addEventListener("pointercancel", this.onCancel)
    canvas.addEventListener("pointerleave", this.onLeave)
    canvas.addEventListener("dblclick", this.onDouble)
  }

  // ───────────────────────────── events ─────────────────────────────

  on<K extends keyof EngineEvents>(event: K, fn: Listener<K>): () => void {
    const set = this.listeners.get(event) ?? new Set()
    set.add(fn as (...a: never[]) => void)
    this.listeners.set(event, set)
    return () => set.delete(fn as (...a: never[]) => void)
  }

  private emit<K extends keyof EngineEvents>(event: K, ...args: Parameters<Listener<K>>): void {
    for (const fn of this.listeners.get(event) ?? []) (fn as (...a: unknown[]) => void)(...args)
  }

  // ───────────────────────────── configuration ─────────────────────────────

  setTheme(theme?: CanvasTheme): void {
    this.theme = theme ?? readCanvasTheme()
    this.invalidate()
  }

  setOptions(next: Partial<EngineOptions>): void {
    const prev = this.opts
    this.opts = { ...prev, ...next }
    if (next.hulls !== undefined && next.hulls !== prev.hulls && !this.opts.reducedMotion)
      this.fade("hulls", this.hullFade, next.hulls ? 1 : 0, (v) => (this.hullFade = v))
    else if (next.hulls !== undefined) this.hullFade = next.hulls ? 1 : 0
    if (next.performance !== undefined && next.performance !== prev.performance) this.hit = null
    if (next.reducedMotion) this.tweens.finishAll()
    this.invalidate()
  }

  setInsets(insets: Insets): void {
    const same =
      insets.left === this.insets.left &&
      insets.right === this.insets.right &&
      insets.top === this.insets.top &&
      insets.bottom === this.insets.bottom
    this.insets = insets
    // Fit zoom (and with it the LOD scale) depends on how much room the overlays leave.
    if (!same) this.refit()
  }

  resize(w: number, h: number, dpr: number): void {
    const cw = Math.max(1, Math.round(w))
    const ch = Math.max(1, Math.round(h))
    if (cw === this.size.w && ch === this.size.h && dpr === this.dpr) return
    const first = this.size.w === 1 && this.size.h === 1
    const dx = (cw - this.size.w) / 2
    const dy = (ch - this.size.h) / 2
    this.size = { w: cw, h: ch }
    this.dpr = dpr
    this.canvas.width = Math.round(cw * dpr)
    this.canvas.height = Math.round(ch * dpr)
    if (this.scene) {
      this.basePx = baseSidePx(this.scene.count, this.size)
      this.refit()
      if (first || this.pristine) this.camera.jumpTo(this.fitCamera())
      else this.camera.jumpTo({ ...this.camera.cam, x: this.camera.cam.x + dx, y: this.camera.cam.y + dy })
    }
    this.invalidate()
  }

  // ───────────────────────────── scene ─────────────────────────────

  /** Installs a new snapshot. The first one fades in cluster by cluster; later ones tween or birth. */
  setScene(scene: Scene | null): void {
    // A snapshot that shares few nodes with the current one is a different graph (another workspace,
    // a wiped one), so it loads fresh instead of tweening from unrelated positions.
    const prev = scene && this.scene && sharedFraction(this.scene, scene) < 0.5 ? null : this.scene
    this.scene = scene
    this.hit = null
    this.pendingLayout = false
    this.births = []
    this.dying = []
    this.pulses = []
    this.tweens.cancel("layout")
    this.tweens.cancel("births")
    if (!scene) {
      this.px = this.py = new Float32Array(0)
      this.selected = new Uint8Array(0)
      this.special = new Uint8Array(0)
      this.invalidate()
      return
    }
    const now = performance.now()
    this.basePx = baseSidePx(scene.count, this.size)
    this.selected = new Uint8Array(scene.count)
    this.special = new Uint8Array(scene.count)
    this.setHoverImmediate(-1)
    this.visible = null
    this.match = null
    this.matchList = null
    this.refit()

    if (!prev) {
      this.pristine = true
      this.px = Float32Array.from(scene.x)
      this.py = Float32Array.from(scene.y)
      this.camera.jumpTo(this.fitCamera())
      this.startLoadIn(scene, now)
    } else {
      const diff = diffScenes(prev, scene)
      const tween = shouldTween(diff, scene.count) && !this.opts.reducedMotion
      this.px = Float32Array.from(tween ? diff.fromX : scene.x)
      this.py = Float32Array.from(tween ? diff.fromY : scene.y)
      if (tween) this.startLayoutTween(scene, now)
      else if (diff.layoutChanged) this.snapFade(now)
      if (diff.born.length > 0 && diff.born.length <= 200 && !this.opts.reducedMotion)
        this.births = diff.born.map((idx) => ({ idx, t0: now }))
      if (diff.born.length > 0) {
        this.emit("births", { idx: diff.born })
        if (this.opts.followNew) this.flyTo(diff.born)
      }
      if (this.births.length > 0)
        this.tweens.start(
          { key: "births", duration: ANIM_MS.glow + 40, update: () => {}, done: () => { this.births = []; this.refreshSpecial() } },
          now
        )
    }
    this.refreshSpecial()
    this.invalidate()
  }

  private startLoadIn(scene: Scene, now: number): void {
    this.clusterAlpha = null
    this.edgesAlpha = 1
    if (this.opts.reducedMotion || scene.count === 0) return
    const order = scene.clusters
      .map((c, i) => ({ i, n: c.nodes.length }))
      .sort((a, b) => b.n - a.n)
      .map((c) => c.i)
    const slots = order.length + 1
    const gap = Math.min(LOAD_STAGGER_MS, (LOAD_CAP_MS - 220) / Math.max(slots, 1))
    const alpha = new Float32Array(scene.clusters.length + 1)
    this.clusterAlpha = alpha
    this.edgesAlpha = 0
    order.forEach((ci, rank) => {
      this.tweens.start(
        { key: `load:${ci}`, delay: rank * gap, duration: 220, ease: easings.out, update: (p) => (alpha[ci] = p) },
        now
      )
    })
    const unclustered = scene.clusters.length
    this.tweens.start(
      { key: "load:rest", delay: order.length * gap, duration: 220, update: (p) => (alpha[unclustered] = p) },
      now
    )
    const nodesEnd = Math.min(order.length * gap + 220, LOAD_CAP_MS)
    this.tweens.start(
      {
        key: "load:edges",
        delay: nodesEnd,
        duration: 280,
        update: (p) => (this.edgesAlpha = p),
        done: () => { this.clusterAlpha = null },
      },
      now
    )
  }

  private startLayoutTween(scene: Scene, now: number): void {
    const fromX = Float32Array.from(this.px)
    const fromY = Float32Array.from(this.py)
    this.pendingLayout = true
    this.fade("hulls", this.hullFade, 0, (v) => (this.hullFade = v), 160)
    this.tweens.start(
      {
        key: "layout",
        duration: 600,
        ease: easings.inout,
        update: (p) => {
          for (let i = 0; i < scene.count; i++) {
            this.px[i] = fromX[i]! + (scene.x[i]! - fromX[i]!) * p
            this.py[i] = fromY[i]! + (scene.y[i]! - fromY[i]!) * p
          }
        },
        done: () => {
          this.pendingLayout = false
          this.hit = null
          if (this.opts.hulls) this.fade("hulls", 0, 1, (v) => (this.hullFade = v))
        },
      },
      now
    )
  }

  /** Large graphs skip the position tween: snap, with a brief opacity cross-fade (docs/web/05 #16). */
  private snapFade(now: number): void {
    this.canvas.style.opacity = "0.55"
    this.tweens.start(
      { key: "snap", duration: 260, update: (p) => (this.canvas.style.opacity = String(0.55 + 0.45 * p)), done: () => (this.canvas.style.opacity = "") },
      now
    )
  }

  private fade(key: string, from: number, to: number, apply: (v: number) => void, ms = 220): void {
    if (this.opts.reducedMotion) return apply(to)
    this.tweens.start(
      { key, duration: ms, ease: easings.out, update: (p) => apply(from + (to - from) * p) },
      performance.now()
    )
    this.invalidate()
  }

  setVisible(mask: Uint8Array | null): void {
    this.visible = mask
    this.hit = null
    this.invalidate()
  }

  setMatch(mask: Uint8Array | null): void {
    this.match = mask
    if (mask) {
      const list: number[] = []
      for (let i = 0; i < mask.length && list.length < 5000; i++) if (mask[i]) list.push(i)
      this.matchList = Uint32Array.from(list)
    } else this.matchList = null
    this.invalidate()
  }

  /** `pulse` lists nodes that were just picked, for the one-shot ring. */
  setSelection(selected: readonly number[], active: number, pulse: readonly number[] = []): void {
    const scene = this.scene
    if (!scene) return
    this.selected.fill(0)
    for (const i of selected) this.selected[i] = 1
    this.selectedAny = selected.length > 0
    this.activeIdx = active
    if (!this.opts.reducedMotion && pulse.length > 0) {
      const now = performance.now()
      this.pulses = pulse.slice(0, 30).map((idx, n) => ({ idx, t0: now + Math.min(n * 30, 300) }))
      this.tweens.start(
        { key: "pulses", delay: 0, duration: ANIM_MS.pulse + 340, update: () => {}, done: () => (this.pulses = []) },
        now
      )
    }
    this.refreshSpecial()
    this.invalidate()
  }

  setDeleteTint(idx: readonly number[] | null): void {
    if (!this.scene) return
    if (!idx || idx.length === 0) this.deleteTint = null
    else {
      this.deleteTint = new Uint8Array(this.scene.count)
      for (const i of idx) this.deleteTint[i] = 1
    }
    this.refreshSpecial()
    this.invalidate()
  }

  /** Shrink + fade nodes before the refetch removes them (docs/web/05 #22). Resolves when done. */
  markDying(idx: readonly number[]): Promise<void> {
    this.deleteTint = null
    if (this.opts.reducedMotion || idx.length === 0) return Promise.resolve()
    const now = performance.now()
    this.dying = idx.map((i) => ({ idx: i, t0: now }))
    this.refreshSpecial()
    this.invalidate()
    return new Promise((resolve) =>
      this.tweens.start({ key: "dying", duration: ANIM_MS.dying, update: () => {}, done: () => resolve() }, now)
    )
  }

  private refreshSpecial(): void {
    this.special.fill(0)
    if (this.hoverIdx >= 0) this.special[this.hoverIdx] = 1
    for (let i = 0; i < this.selected.length; i++) if (this.selected[i]) this.special[i] = 1
    for (const a of this.births) this.special[a.idx] = 1
    for (const a of this.dying) this.special[a.idx] = 1
    if (this.deleteTint) for (let i = 0; i < this.deleteTint.length; i++) if (this.deleteTint[i]) this.special[i] = 1
  }

  // ───────────────────────────── camera helpers ─────────────────────────────

  private usable(): { size: Size; left: number; top: number } {
    const { left, right, top, bottom } = this.insets
    return {
      size: { w: Math.max(this.size.w - left - right, 120), h: Math.max(this.size.h - top - bottom, 120) },
      left,
      top,
    }
  }

  private boundsOf(idx?: readonly number[]): Bounds | null {
    const s = this.scene
    if (!s || s.count === 0) return null
    let minX = Infinity, minY = Infinity, maxX = -Infinity, maxY = -Infinity
    const visit = (i: number): void => {
      const x = s.x[i]!, y = s.y[i]!
      if (x < minX) minX = x
      if (x > maxX) maxX = x
      if (y < minY) minY = y
      if (y > maxY) maxY = y
    }
    if (idx) for (const i of idx) visit(i)
    else for (let i = 0; i < s.count; i++) if (!this.visible || this.visible[i]) visit(i)
    return Number.isFinite(minX) ? { minX, minY, maxX, maxY } : null
  }

  private fitCamera(idx?: readonly number[], maxK = 4): Cam {
    const bounds = this.boundsOf(idx) ?? { minX: -1, minY: -1, maxX: 1, maxY: 1 }
    const { size, left, top } = this.usable()
    const cam = fitCam(bounds, size, FIT_PADDING, maxK)
    return { ...cam, x: cam.x + left, y: cam.y + top }
  }

  private refit(): void {
    const all = this.scene ? this.boundsOf() : null
    if (!all) return
    const { size } = this.usable()
    this.fitK = fitCam(all, size, FIT_PADDING, 4).k
    this.camera.setExtent(this.fitK)
  }

  fit(animate = true): void {
    this.pristine = true
    this.camera.animateTo(this.fitCamera(), animate ? 520 : 0, performance.now())
  }

  /** Centers on one node (zooming in only if the view is tiny) or fits a group. */
  flyTo(idx: readonly number[]): void {
    const s = this.scene
    if (!s || idx.length === 0) return
    this.pristine = false
    const now = performance.now()
    const cam = this.camera.target
    let to: Cam
    if (idx.length === 1) {
      const i = idx[0]!
      const { size, left, top } = this.usable()
      const wx = s.x[i]!
      const wy = s.y[i]!
      const [sx, sy] = worldToScreen(cam, wx, wy)
      const local = { w: size.w, h: size.h }
      if (!isOffCenter({ ...cam, x: cam.x - left, y: cam.y - top }, wx, wy, local, 0.35) && sx > 0 && sy > 0 && cam.k >= this.fitK * 1.2) return
      const k = cam.k < this.fitK * 1.6 ? this.fitK * 2.2 : cam.k
      const centered = centerOn(wx, wy, k, size)
      to = { ...centered, x: centered.x + left, y: centered.y + top }
    } else to = this.fitCamera(idx, this.fitK * 6)
    this.camera.animateTo(to, flyDuration(this.camera.cam, to, this.size), now)
  }

  zoomBy(factor: number): void {
    this.pristine = false
    const { size, left, top } = this.usable()
    this.camera.zoomAt(left + size.w / 2, top + size.h / 2, factor, performance.now())
  }

  /** Relative zoom (1 = fit). */
  get zoomLevel(): number {
    return this.camera.cam.k / this.fitK
  }

  // ───────────────────────────── queries ─────────────────────────────

  private ensureHit(): HitIndex | null {
    const s = this.scene
    if (!s) return null
    if (!this.hit) this.hit = buildHitIndex(this.px, this.py, s.count, this.visible)
    return this.hit
  }

  hitAt(sx: number, sy: number): number {
    const s = this.scene
    const index = this.ensureHit()
    if (!s || !index) return -1
    const { k } = this.camera.cam
    const z = this.zoomLevel
    const [wx, wy] = screenToWorld(this.camera.cam, sx, sy)
    return hitTest(
      index,
      this.px,
      this.py,
      (i) => nodeScreenSide(this.basePx, s.weight[i]!, z) / 2 / k,
      nodeScreenSide(this.basePx, 2.2, z) / 2 / k,
      wx,
      wy,
      3 / k
    )
  }

  nodeScreen(idx: number): [number, number] | null {
    if (!this.scene || idx < 0 || idx >= this.scene.count) return null
    return worldToScreen(this.camera.cam, this.px[idx]!, this.py[idx]!)
  }

  nodeSide(idx: number): number {
    return this.scene ? nodeScreenSide(this.basePx, this.scene.weight[idx]!, this.zoomLevel) : 0
  }

  stats(): { frames: number; costP95: number; gapP95: number; gapMax: number } {
    const p95 = (xs: number[]): number => {
      if (xs.length === 0) return 0
      const sorted = [...xs].sort((a, b) => a - b)
      return sorted[Math.min(sorted.length - 1, Math.floor(sorted.length * 0.95))]!
    }
    return { frames: this.frameCost.length, costP95: p95(this.frameCost), gapP95: p95(this.frameGap), gapMax: Math.max(0, ...this.frameGap) }
  }

  resetStats(): void {
    this.frameCost = []
    this.frameGap = []
  }

  // ───────────────────────────── hover ─────────────────────────────

  private setHoverImmediate(idx: number): void {
    this.hoverWanted = idx
    this.hoverIdx = idx
    this.hoverMix = idx >= 0 ? 1 : 0
    this.neighbour = idx >= 0 ? this.neighbourFlags(idx) : null
    this.tweens.cancel("hover")
  }

  private neighbourFlags(idx: number): Uint8Array {
    const s = this.scene!
    const flags = new Uint8Array(s.count)
    flags[idx] = 1
    for (let p = s.adjStart[idx]!; p < s.adjStart[idx + 1]!; p++) flags[s.adjList[p]!] = 1
    return flags
  }

  private setHover(idx: number): void {
    if (idx === this.hoverWanted) return
    this.hoverWanted = idx
    this.emit("hover", idx)
    this.canvas.style.cursor = idx >= 0 ? "pointer" : ""
    if (this.opts.reducedMotion) {
      this.setHoverImmediate(idx)
      this.hoverWanted = idx
      this.refreshSpecial()
      return this.invalidate()
    }
    const now = performance.now()
    if (idx >= 0) {
      if (this.hoverIdx !== idx) {
        this.hoverIdx = idx
        this.neighbour = this.neighbourFlags(idx)
        this.refreshSpecial()
      }
      const from = this.hoverMix
      this.tweens.start(
        { key: "hover", duration: HOVER_MS * (1 - from * 0.5), ease: easings.out, update: (p) => (this.hoverMix = from + (1 - from) * p) },
        now
      )
    } else {
      const from = this.hoverMix
      this.tweens.start(
        {
          key: "hover",
          duration: HOVER_MS * Math.max(from, 0.2),
          ease: easings.out,
          update: (p) => (this.hoverMix = from * (1 - p)),
          done: () => {
            if (this.hoverWanted >= 0) return
            this.hoverIdx = -1
            this.neighbour = null
            this.refreshSpecial()
          },
        },
        now
      )
    }
    this.invalidate()
  }

  // ───────────────────────────── pointer input ─────────────────────────────

  private local(e: PointerEvent | MouseEvent): { x: number; y: number } {
    const r = this.canvas.getBoundingClientRect()
    return { x: e.clientX - r.left, y: e.clientY - r.top }
  }

  private readonly onDown = (e: PointerEvent): void => {
    if (e.button !== 0) return
    const p = this.local(e)
    this.down = { x: p.x, y: p.y, t: performance.now(), shift: e.shiftKey, meta: e.metaKey || e.ctrlKey }
    if (e.shiftKey) {
      this.marquee = { sx: p.x, sy: p.y, x: p.x, y: p.y, active: false }
      this.canvas.setPointerCapture(e.pointerId)
    }
    this.canvas.style.cursor = e.shiftKey ? "crosshair" : "grabbing"
  }

  private readonly onMove = (e: PointerEvent): void => {
    const p = this.local(e)
    if (this.marquee) {
      this.marquee.x = p.x
      this.marquee.y = p.y
      if (!this.marquee.active && Math.hypot(p.x - this.marquee.sx, p.y - this.marquee.sy) > 4) this.marquee.active = true
      if (this.marquee.active) this.emit("marqueeRect", this.marqueeRect())
      return
    }
    if (this.down) return
    this.pendingHover = p
    this.invalidate()
  }

  private marqueeRect(): MarqueeRect {
    const m = this.marquee!
    return { x: Math.min(m.sx, m.x), y: Math.min(m.sy, m.y), w: Math.abs(m.x - m.sx), h: Math.abs(m.y - m.sy) }
  }

  private readonly onUp = (e: PointerEvent): void => {
    const down = this.down
    this.down = null
    this.canvas.style.cursor = this.hoverWanted >= 0 ? "pointer" : ""
    if (this.marquee) {
      const m = this.marquee
      this.marquee = null
      if (this.canvas.hasPointerCapture(e.pointerId)) this.canvas.releasePointerCapture(e.pointerId)
      if (m.active) {
        const rect = { x: Math.min(m.sx, m.x), y: Math.min(m.sy, m.y), w: Math.abs(m.x - m.sx), h: Math.abs(m.y - m.sy) }
        this.emit("marqueeRect", null)
        const index = this.ensureHit()
        if (index) {
          const [x0, y0] = screenToWorld(this.camera.cam, rect.x, rect.y)
          const [x1, y1] = screenToWorld(this.camera.cam, rect.x + rect.w, rect.y + rect.h)
          this.emit("marquee", { idx: nodesInRect(index, this.px, this.py, { x0, y0, x1, y1 }) })
        }
        return
      }
    }
    if (!down) return
    const p = this.local(e)
    if (Math.hypot(p.x - down.x, p.y - down.y) > 4 || performance.now() - down.t > 600) return
    this.emit("click", { idx: this.hitAt(p.x, p.y), shift: down.shift, meta: down.meta })
  }

  private readonly onCancel = (): void => {
    this.down = null
    if (this.marquee) {
      this.marquee = null
      this.emit("marqueeRect", null)
    }
  }

  private readonly onLeave = (): void => {
    this.pendingHover = null
    if (!this.down) this.setHover(-1)
  }

  private readonly onDouble = (e: MouseEvent): void => {
    const p = this.local(e)
    const idx = this.hitAt(p.x, p.y)
    if (idx >= 0) this.emit("dblclick", { idx })
    else this.camera.zoomAtDoubleClick(p.x, p.y, performance.now())
  }

  cancelMarquee(): boolean {
    if (!this.marquee) return false
    this.onCancel()
    return true
  }

  // ───────────────────────────── loop ─────────────────────────────

  invalidate(): void {
    if (this.raf || this.destroyed) return
    this.raf = requestAnimationFrame(this.frame)
  }

  private readonly frame = (now: number): void => {
    this.raf = 0
    if (this.destroyed) return
    const gap = this.last ? now - this.last : 16.7
    const dt = Math.min(Math.max(gap, 0), 64)
    this.last = now
    const t0 = performance.now()

    if (this.pendingHover) {
      const { x, y } = this.pendingHover
      this.pendingHover = null
      this.setHover(this.hitAt(x, y))
    }
    let busy = this.tweens.tick(now)
    if (this.camera.tick(now, dt)) busy = true
    this.draw(now)
    this.keepAnimations(now)

    const key = `${this.camera.cam.x.toFixed(2)},${this.camera.cam.y.toFixed(2)},${this.camera.cam.k.toFixed(5)}`
    if (key !== this.lastCamKey) {
      this.lastCamKey = key
      this.emit("camera", this.camera.cam)
    }
    if (busy || this.hasLiveAnimations(now)) {
      this.frameGap.push(gap)
      this.frameCost.push(performance.now() - t0)
      if (this.frameCost.length > FRAME_SAMPLES) {
        this.frameCost.shift()
        this.frameGap.shift()
      }
      this.invalidate()
    } else this.last = 0
  }

  private hasLiveAnimations(now: number): boolean {
    if (!this.scene && this.pendingSizes) return !this.opts.reducedMotion
    return (
      this.deleteTint !== null ||
      this.births.length > 0 ||
      this.dying.length > 0 ||
      this.pulses.some((p) => now - p.t0 < ANIM_MS.pulse) ||
      this.tweens.size > 0
    )
  }

  /** Drops finished one-shot animations so their nodes return to the bulk pass. */
  private keepAnimations(now: number): void {
    const alive = (a: NodeAnim, ms: number): boolean => now - a.t0 < ms
    if (this.pulses.length && !this.pulses.some((p) => alive(p, ANIM_MS.pulse))) this.pulses = []
    if (this.births.length && !this.births.some((b) => alive(b, ANIM_MS.glow))) {
      this.births = []
      this.refreshSpecial()
    }
  }

  private draw(now: number): void {
    const scene = this.scene
    if (!scene) {
      if (this.pendingSizes)
        renderPending(this.ctx, this.size, this.dpr, this.pendingSizes, this.theme.clusters, now, !this.opts.reducedMotion)
      else {
        this.ctx.setTransform(this.dpr, 0, 0, this.dpr, 0, 0)
        this.ctx.clearRect(0, 0, this.size.w, this.size.h)
      }
      return
    }
    const f = (this.frame_ ??= {} as FrameState)
    f.size = this.size
    f.dpr = this.dpr
    f.cam = this.camera.cam
    f.z = this.zoomLevel
    f.theme = this.theme
    f.scene = scene
    f.px = this.px
    f.py = this.py
    f.basePx = this.basePx
    f.visible = this.visible
    f.match = this.match
    f.matchList = this.matchList
    f.selected = this.selected
    f.selectedAny = this.selectedAny
    f.activeIdx = this.activeIdx
    f.hoverIdx = this.hoverIdx
    f.hoverMix = this.hoverMix
    f.neighbour = this.neighbour
    f.special = this.special
    f.clusterAlpha = this.clusterAlpha
    f.edgesAlpha = this.edgesAlpha
    f.hullFade = this.hullFade
    f.births = this.births
    f.dying = this.dying
    f.pulses = this.pulses.filter((p) => p.t0 <= now)
    f.deleteTint = this.deleteTint
    f.now = now
    f.hulls = this.opts.hulls
    f.labels = this.opts.labels
    f.performance = this.opts.performance
    f.interacting = this.camera.interacting
    renderFrame(this.ctx, f)
  }

  /** Shows soft cluster silhouettes while the server computes the layout (no scene yet). */
  setPending(clusterSizes: readonly number[] | null): void {
    this.pendingSizes = clusterSizes
    this.invalidate()
  }

  /** The node under the pointer right now (-1 when none); the drawn hover lags behind it. */
  get hovered(): number {
    return this.hoverWanted
  }

  /** Live positions are only stable once a layout tween has finished. */
  get layoutSettled(): boolean {
    return !this.pendingLayout
  }

  destroy(): void {
    this.destroyed = true
    cancelAnimationFrame(this.raf)
    this.tweens.clear()
    this.camera.destroy()
    this.canvas.removeEventListener("pointerdown", this.onDown)
    this.canvas.removeEventListener("pointermove", this.onMove)
    this.canvas.removeEventListener("pointerup", this.onUp)
    this.canvas.removeEventListener("pointercancel", this.onCancel)
    this.canvas.removeEventListener("pointerleave", this.onLeave)
    this.canvas.removeEventListener("dblclick", this.onDouble)
    this.listeners.clear()
  }
}
