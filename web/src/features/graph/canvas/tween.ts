// Tween clock: one rAF-driven timeline for every canvas animation (docs/web/05 "own rAF clock").
// Tweens are keyed, so starting one with an existing key retargets instead of queueing (interruptible).
import { easeCubicInOut, easeCubicOut, easeLinear } from "d3-ease"

type Easing = (t: number) => number
export const easings = {
  out: easeCubicOut,
  inout: easeCubicInOut,
  linear: easeLinear,
} as const

export type TweenSpec = {
  key: string
  duration: number
  /** ms before the tween starts moving (stagger). */
  delay?: number
  ease?: Easing
  /** Called with eased progress 0..1 each tick; called once with 1 on completion. */
  update: (progress: number) => void
  done?: () => void
}

type Active = Required<Pick<TweenSpec, "duration" | "update">> &
  Pick<TweenSpec, "done" | "ease"> & { start: number; delay: number }

export class TweenClock {
  private active = new Map<string, Active>()

  /** Starts (or replaces) the tween `spec.key`. `now` is a `performance.now()` timestamp. */
  start(spec: TweenSpec, now: number): void {
    this.active.set(spec.key, {
      duration: Math.max(spec.duration, 0),
      update: spec.update,
      done: spec.done,
      ease: spec.ease,
      start: now,
      delay: spec.delay ?? 0,
    })
  }

  cancel(key: string): void {
    this.active.delete(key)
  }

  has(key: string): boolean {
    return this.active.has(key)
  }

  get size(): number {
    return this.active.size
  }

  /** Advances every tween; returns true while any remain (the caller keeps its rAF loop going). */
  tick(now: number): boolean {
    for (const [key, tween] of [...this.active]) {
      if (this.active.get(key) !== tween) continue
      const elapsed = now - tween.start - tween.delay
      if (elapsed < 0) continue
      const raw =
        tween.duration === 0 ? 1 : Math.min(1, elapsed / tween.duration)
      const eased = raw >= 1 ? 1 : (tween.ease ?? easings.out)(raw)
      tween.update(eased)
      if (raw >= 1) {
        // A callback may have restarted this key; only drop the tween we just finished.
        if (this.active.get(key) === tween) this.active.delete(key)
        tween.done?.()
      }
    }
    return this.active.size > 0
  }

  /** Jumps every tween to its end state (reduced motion / teardown). */
  finishAll(): void {
    for (const [key, tween] of [...this.active]) {
      this.active.delete(key)
      tween.update(1)
      tween.done?.()
    }
  }

  clear(): void {
    this.active.clear()
  }
}
