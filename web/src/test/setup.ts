import "@testing-library/jest-dom/vitest"
import { cleanup } from "@testing-library/react"
import { afterAll, afterEach, beforeAll, vi } from "vitest"

import { mockMatchMedia } from "./media"
import { server } from "./msw/server"

// jsdom lacks these browser APIs; Radix, cmdk and motion probe for them.
class ResizeObserverStub {
  observe() {}
  unobserve() {}
  disconnect() {}
}
vi.stubGlobal("ResizeObserver", ResizeObserverStub)
Element.prototype.scrollIntoView = vi.fn()
Element.prototype.hasPointerCapture = vi.fn(() => false)
Element.prototype.setPointerCapture = vi.fn()
Element.prototype.releasePointerCapture = vi.fn()

// Must exist before any module (the ui store) resolves the theme at import time.
mockMatchMedia()

beforeAll(() => {
  server.listen({ onUnhandledFrame: "error" })
})
afterEach(() => {
  cleanup()
  server.resetHandlers()
  localStorage.clear()
  document.documentElement.className = ""
  document.documentElement.removeAttribute("data-reduce-motion")
  document.documentElement.dir = "ltr"
  mockMatchMedia()
})
afterAll(() => server.close())
