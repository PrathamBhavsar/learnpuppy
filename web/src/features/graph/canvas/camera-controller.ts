// Owns the camera: gesture input through d3-zoom (drag, wheel, pinch), smoothed wheel zoom, pan
// inertia, and eased programmatic moves (fit, fly-to, double-click zoom). d3-zoom holds the *target*
// transform; `cam` is what is drawn and chases it.
import { select } from "d3-selection"
import {
  type D3ZoomEvent,
  type ZoomBehavior,
  zoom,
  zoomIdentity,
} from "d3-zoom"

import {
  type Cam,
  DOUBLE_CLICK_ZOOM,
  IDENTITY,
  WHEEL_LERP,
  type Velocity,
  blendVelocity,
  camEquals,
  clampK,
  inertiaStep,
  lerpCam,
  smoothToward,
  zoomAbout,
} from "./camera"
import { TweenClock, easings } from "./tween"

type Hooks = {
  /** Ask the engine for another frame. */
  invalidate: () => void
  reducedMotion: () => boolean
  /** The user grabbed the canvas (drag/wheel/pinch started). */
  onUserInput: () => void
}

const CAM_TWEEN = "cam"

export class CameraController {
  cam: Cam = IDENTITY
  target: Cam = IDENTITY
  extent: readonly [number, number] = [0.05, 200]
  /** True while a user gesture is moving the camera (drag, wheel settling, inertia). */
  interacting = false

  private readonly behavior: ZoomBehavior<HTMLCanvasElement, unknown>
  private programmatic = 0
  private velocity: Velocity = { vx: 0, vy: 0 }
  private lastMove = 0
  private inertia: Velocity | null = null
  private lastInertia = 0
  private readonly onCapture: () => void
  private readonly canvas: HTMLCanvasElement
  private readonly tweens: TweenClock
  private readonly hooks: Hooks

  constructor(canvas: HTMLCanvasElement, tweens: TweenClock, hooks: Hooks) {
    this.canvas = canvas
    this.tweens = tweens
    this.hooks = hooks
    this.behavior = zoom<HTMLCanvasElement, unknown>()
      .scaleExtent([this.extent[0], this.extent[1]])
      .clickDistance(4)
      .filter((event: Event) => {
        const e = event as MouseEvent
        if (e.type === "dblclick") return false
        // Shift-drag is the marquee; secondary buttons belong to the context menu.
        if (e.type === "mousedown" && (e.shiftKey || e.button !== 0))
          return false
        return e.type === "wheel" ? true : !e.ctrlKey
      })
      .wheelDelta((event: WheelEvent) => {
        const unit = event.deltaMode === 1 ? 0.05 : event.deltaMode ? 1 : 0.002
        return -event.deltaY * unit * (event.ctrlKey ? 8 : 1)
      })
      .on("start", (e: D3ZoomEvent<HTMLCanvasElement, unknown>) => {
        if (e.sourceEvent) {
          this.interacting = true
          this.lastMove = performance.now()
          this.velocity = { vx: 0, vy: 0 }
          hooks.onUserInput()
        }
      })
      .on("zoom", (e: D3ZoomEvent<HTMLCanvasElement, unknown>) =>
        this.onZoom(e)
      )
      .on("end", (e: D3ZoomEvent<HTMLCanvasElement, unknown>) => this.onEnd(e))
    select(canvas).call(this.behavior).on("dblclick.zoom", null)
    // Capture phase: a new gesture must stop an in-flight fly-to or inertia before d3 reads its state.
    this.onCapture = () => this.stop()
    canvas.addEventListener("pointerdown", this.onCapture, true)
    canvas.addEventListener("wheel", this.onCapture, true)
  }

  destroy(): void {
    this.canvas.removeEventListener("pointerdown", this.onCapture, true)
    this.canvas.removeEventListener("wheel", this.onCapture, true)
    select(this.canvas).on(".zoom", null)
  }

  setExtent(fitK: number): void {
    this.extent = [fitK * 0.35, fitK * 48]
    this.behavior.scaleExtent([this.extent[0], this.extent[1]])
  }

  /** Cancels fly-to and inertia, leaving the camera exactly where it is drawn. */
  stop(): void {
    const moving =
      this.tweens.has(CAM_TWEEN) ||
      this.inertia !== null ||
      !camEquals(this.cam, this.target)
    this.tweens.cancel(CAM_TWEEN)
    this.inertia = null
    if (moving) {
      this.target = this.cam
      this.sync(this.cam)
    }
  }

  jumpTo(cam: Cam): void {
    this.tweens.cancel(CAM_TWEEN)
    this.inertia = null
    this.cam = this.target = cam
    this.sync(cam)
    this.hooks.invalidate()
  }

  /** Eased move to `to` (interruptible; jumps under reduced motion). */
  animateTo(to: Cam, duration: number, now: number): void {
    if (this.hooks.reducedMotion() || duration <= 0) return this.jumpTo(to)
    const from = this.cam
    this.inertia = null
    this.target = to
    this.sync(to)
    this.tweens.start(
      {
        key: CAM_TWEEN,
        duration,
        ease: easings.inout,
        update: (p) => {
          this.cam = lerpCam(from, to, p)
        },
      },
      now
    )
    this.hooks.invalidate()
  }

  zoomAt(
    px: number,
    py: number,
    factor: number,
    now: number,
    animate = true
  ): void {
    const base = this.tweens.has(CAM_TWEEN) ? this.target : this.cam
    const to = zoomAbout(base, px, py, factor, this.extent)
    if (animate) this.animateTo(to, 260, now)
    else this.jumpTo(to)
  }

  zoomAtDoubleClick(px: number, py: number, now: number): void {
    this.zoomAt(px, py, DOUBLE_CLICK_ZOOM, now)
  }

  /** Advances smoothing and inertia; returns true while the camera is still moving. */
  tick(now: number, dtMs: number): boolean {
    let busy = false
    if (this.inertia) {
      const prev = this.lastInertia || now - dtMs
      this.lastInertia = now
      const step = inertiaStep(this.inertia, now - prev)
      if (step) {
        this.inertia = step.next
        this.target = {
          ...this.target,
          x: this.target.x + step.dx,
          y: this.target.y + step.dy,
        }
        this.cam = this.target
        this.sync(this.target)
        busy = true
      } else {
        this.inertia = null
        this.interacting = false
      }
    }
    if (!this.tweens.has(CAM_TWEEN) && !camEquals(this.cam, this.target)) {
      this.cam = this.hooks.reducedMotion()
        ? this.target
        : smoothToward(this.cam, this.target, WHEEL_LERP, dtMs)
      if (camEquals(this.cam, this.target)) this.cam = this.target
      busy = true
    } else if (this.tweens.has(CAM_TWEEN)) busy = true
    if (!busy && !this.inertia) this.interacting = false
    return busy
  }

  private sync(cam: Cam): void {
    this.programmatic += 1
    try {
      select(this.canvas).call(
        this.behavior.transform,
        zoomIdentity.translate(cam.x, cam.y).scale(cam.k)
      )
    } finally {
      this.programmatic -= 1
    }
  }

  private onZoom(e: D3ZoomEvent<HTMLCanvasElement, unknown>): void {
    if (this.programmatic > 0 || !e.sourceEvent) return
    const t: Cam = {
      x: e.transform.x,
      y: e.transform.y,
      k: clampK(e.transform.k, this.extent),
    }
    if (e.sourceEvent.type === "wheel") {
      this.target = t
    } else {
      const now = performance.now()
      this.velocity = blendVelocity(
        this.velocity,
        t.x - this.target.x,
        t.y - this.target.y,
        now - this.lastMove
      )
      this.lastMove = now
      this.cam = this.target = t
    }
    this.hooks.invalidate()
  }

  private onEnd(e: D3ZoomEvent<HTMLCanvasElement, unknown>): void {
    const type = e.sourceEvent?.type
    if (type === "mouseup" || type === "touchend" || type === "touchcancel") {
      const idle = performance.now() - this.lastMove
      const speed = Math.hypot(this.velocity.vx, this.velocity.vy)
      if (!this.hooks.reducedMotion() && idle < 80 && speed > 1) {
        this.inertia = this.velocity
        this.lastInertia = 0
        this.hooks.invalidate()
        return
      }
    }
    if (!this.inertia && camEquals(this.cam, this.target))
      this.interacting = false
  }
}
