import "@testing-library/jest-dom/vitest"
import { cleanup } from "@testing-library/react"
import { afterAll, afterEach, beforeAll, vi } from "vitest"

import { setSseManager } from "@/hooks/use-server-events"
import { useConnection } from "@/stores/connection"
import { useLive } from "@/stores/live"
import { usePalette } from "@/stores/palette"
import { useStatus } from "@/stores/status"

import { FakeEventSource } from "./fake-event-source"
import { mockMatchMedia } from "./media"
import { bus } from "./mock/bus"
import { resetDb } from "./msw/db"
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

// The shell opens an event stream; jsdom has no EventSource, so every test gets the scriptable fake.
vi.stubGlobal("EventSource", FakeEventSource)

// Must exist before any module (the ui store) resolves the theme at import time.
mockMatchMedia()

beforeAll(() => {
  server.listen({ onUnhandledFrame: "error" })
})
afterEach(() => {
  cleanup()
  server.resetHandlers()
  resetDb()
  bus.reset()
  setSseManager(null)
  FakeEventSource.reset()
  useConnection.getState().reset()
  useLive.getState().reset()
  useStatus.setState({ authRequired: false, engineUnavailable: false })
  usePalette.setState({
    open: false,
    query: "",
    scope: "workspace",
    recents: [],
  })
  localStorage.clear()
  document.documentElement.className = ""
  document.documentElement.removeAttribute("data-reduce-motion")
  document.documentElement.removeAttribute("data-density")
  document.documentElement.dir = "ltr"
  mockMatchMedia()
})
afterAll(() => server.close())
