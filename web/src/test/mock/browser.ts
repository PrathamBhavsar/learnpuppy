import type { QueryClient } from "@tanstack/react-query"
import { setupWorker } from "msw/browser"

import { SseManager } from "@/api/events"
import { setSseManager } from "@/hooks/use-server-events"
import { useConnection } from "@/stores/connection"

import { FakeEventSource } from "../fake-event-source"
import { state } from "../msw/db"
import { handlers } from "../msw/handlers"

import { bus } from "./bus"

declare global {
  interface Window {
    /** Dev-mock controls: `__lpMock.dropSse()` / `restoreSse()` exercise the reconnect banner by hand. */
    __lpMock?: {
      dropSse: () => void
      restoreSse: () => void
      emit: typeof bus.emit
    }
    __lp?: { sse: { state: () => string } }
  }
}

/** Boots MSW and swaps the SSE manager onto the in-page event bus. */
export async function startMock(queryClient: QueryClient): Promise<void> {
  // Browser-only: stream scripted assistant replies so Learn has something to render live.
  state.db.simulateTurns = true
  const worker = setupWorker(...handlers)
  await worker.start({
    onUnhandledFrame: "bypass",
    serviceWorker: { url: "/mockServiceWorker.js" },
    quiet: true,
  })
  setSseManager(
    new SseManager({
      queryClient,
      factory: (url) => {
        const source = new FakeEventSource(url)
        bus.attach(source)
        return source
      },
    })
  )
  window.__lpMock = {
    dropSse: () => bus.drop(),
    restoreSse: () => bus.restore(),
    emit: bus.emit.bind(bus),
  }
  window.__lp = { sse: { state: () => useConnection.getState().status } }
}
