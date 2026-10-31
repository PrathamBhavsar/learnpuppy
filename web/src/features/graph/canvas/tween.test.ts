import { describe, expect, it, vi } from "vitest"

import { TweenClock, easings } from "./tween"

describe("TweenClock", () => {
  it("reports progress over the duration and completes once", () => {
    const clock = new TweenClock()
    const update = vi.fn()
    const done = vi.fn()
    clock.start({ key: "a", duration: 100, ease: easings.linear, update, done }, 0)
    expect(clock.tick(50)).toBe(true)
    expect(update).toHaveBeenLastCalledWith(0.5)
    expect(clock.tick(100)).toBe(false)
    expect(update).toHaveBeenLastCalledWith(1)
    expect(done).toHaveBeenCalledTimes(1)
    expect(clock.tick(200)).toBe(false)
    expect(done).toHaveBeenCalledTimes(1)
  })

  it("waits out the delay before moving", () => {
    const clock = new TweenClock()
    const update = vi.fn()
    clock.start({ key: "a", duration: 100, delay: 40, ease: easings.linear, update }, 0)
    clock.tick(30)
    expect(update).not.toHaveBeenCalled()
    clock.tick(90)
    expect(update).toHaveBeenLastCalledWith(0.5)
  })

  it("retargets: starting the same key replaces the running tween", () => {
    const clock = new TweenClock()
    const first = vi.fn()
    const second = vi.fn()
    clock.start({ key: "cam", duration: 100, ease: easings.linear, update: first }, 0)
    clock.tick(40)
    clock.start({ key: "cam", duration: 100, ease: easings.linear, update: second }, 40)
    clock.tick(90)
    expect(first).toHaveBeenCalledTimes(1)
    expect(second).toHaveBeenLastCalledWith(0.5)
    expect(clock.size).toBe(1)
  })

  it("cancel drops a tween without calling done", () => {
    const clock = new TweenClock()
    const done = vi.fn()
    clock.start({ key: "a", duration: 100, update: () => {}, done }, 0)
    clock.cancel("a")
    expect(clock.has("a")).toBe(false)
    expect(clock.tick(500)).toBe(false)
    expect(done).not.toHaveBeenCalled()
  })

  it("lets a done callback restart the same key", () => {
    const clock = new TweenClock()
    let runs = 0
    const loop = (now: number) =>
      clock.start({ key: "a", duration: 10, update: () => {}, done: () => runs++ < 1 && loop(now + 10) }, now)
    loop(0)
    clock.tick(10)
    expect(clock.has("a")).toBe(true)
    clock.tick(20)
    expect(clock.has("a")).toBe(false)
    expect(runs).toBe(2)
  })

  it("finishAll jumps every tween to its end (reduced motion)", () => {
    const clock = new TweenClock()
    const a = vi.fn()
    const b = vi.fn()
    clock.start({ key: "a", duration: 1000, update: a }, 0)
    clock.start({ key: "b", duration: 1000, delay: 500, update: b }, 0)
    clock.finishAll()
    expect(a).toHaveBeenLastCalledWith(1)
    expect(b).toHaveBeenLastCalledWith(1)
    expect(clock.size).toBe(0)
  })

  it("treats a zero duration as instant", () => {
    const clock = new TweenClock()
    const update = vi.fn()
    clock.start({ key: "a", duration: 0, update }, 0)
    clock.tick(0)
    expect(update).toHaveBeenCalledWith(1)
  })
})
