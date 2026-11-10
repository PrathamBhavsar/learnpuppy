import { renderHook } from "@testing-library/react"
import { afterEach, describe, expect, it, vi } from "vitest"

import { useFocusGuard } from "./use-focus-guard"

function setHidden(hidden: boolean) {
  Object.defineProperty(document, "hidden", {
    configurable: true,
    get: () => hidden,
  })
  document.dispatchEvent(new Event("visibilitychange"))
}

afterEach(() => {
  setHidden(false)
})

describe("useFocusGuard (F25)", () => {
  it("reports each time the tab becomes hidden", () => {
    const onLeave = vi.fn()
    renderHook(() => useFocusGuard(onLeave))
    setHidden(true)
    expect(onLeave).toHaveBeenCalledTimes(1)
    setHidden(false)
    expect(onLeave).toHaveBeenCalledTimes(1)
    setHidden(true)
    expect(onLeave).toHaveBeenCalledTimes(2)
  })

  it("does nothing once disabled", () => {
    const onLeave = vi.fn()
    const { rerender } = renderHook(
      ({ enabled }) => useFocusGuard(onLeave, enabled),
      { initialProps: { enabled: true } }
    )
    rerender({ enabled: false })
    setHidden(true)
    expect(onLeave).not.toHaveBeenCalled()
  })

  it("always calls the latest callback, even without re-registering the listener", () => {
    const first = vi.fn()
    const second = vi.fn()
    const { rerender } = renderHook(({ cb }) => useFocusGuard(cb), {
      initialProps: { cb: first },
    })
    rerender({ cb: second })
    setHidden(true)
    expect(first).not.toHaveBeenCalled()
    expect(second).toHaveBeenCalledTimes(1)
  })
})
