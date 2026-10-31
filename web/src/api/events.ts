import type { QueryClient } from "@tanstack/react-query"

import { useConnection } from "@/stores/connection"

import { API_PREFIX } from "./client"
import {
  EVENT_NAMES,
  type EventMap,
  type EventName,
  type UiHooks,
  applyEvent,
} from "./event-table"

/** The slice of `EventSource` the manager uses; tests and mock mode supply a fake. */
export type EventSourceLike = {
  readonly readyState: number
  onopen: ((event: Event) => void) | null
  onerror: ((event: Event) => void) | null
  addEventListener(
    type: string,
    listener: (event: MessageEvent<string>) => void
  ): void
  close(): void
}
export type EventSourceFactory = (url: string) => EventSourceLike

const CONNECTING = 0
const CLOSED = 2

export const BACKOFF_MIN_MS = 500
export const BACKOFF_MAX_MS = 8000
/** No frame for this long means the stream may be half-open: probe the server. */
export const WATCHDOG_MS = 35_000
/** How long the browser's own reconnect (which sends `Last-Event-ID`) gets before we take over. */
export const NATIVE_RETRY_GRACE_MS = 4000

/** 0.5 s doubling to 8 s, ±20% jitter, never above the cap. */
export function backoffDelay(attempt: number, random: () => number): number {
  const base = Math.min(BACKOFF_MAX_MS, BACKOFF_MIN_MS * 2 ** attempt)
  return Math.min(BACKOFF_MAX_MS, Math.round(base * (0.8 + 0.4 * random())))
}

export type SseOptions = {
  queryClient: QueryClient
  factory?: EventSourceFactory
  random?: () => number
  /** `GET /health`-style liveness probe used by the watchdog. */
  probe?: () => Promise<boolean>
  ui?: UiHooks
}

export function eventsUrl(
  workspaceId: string | null,
  lastEventId?: string
): string {
  const params = new URLSearchParams()
  if (workspaceId) params.set("workspace", workspaceId)
  // The server resumes from the `Last-Event-ID` header the browser sends on its own reconnects; a stream
  // we re-open ourselves cannot set headers, so the id also travels in the query for servers that read it.
  if (lastEventId) params.set("lastEventId", lastEventId)
  const query = params.toString()
  return `${API_PREFIX}/events${query ? `?${query}` : ""}`
}

const defaultFactory: EventSourceFactory = (url) =>
  new EventSource(url) as unknown as EventSourceLike

async function defaultProbe(): Promise<boolean> {
  try {
    const response = await fetch(`${API_PREFIX}/health`, { cache: "no-store" })
    return response.ok
  } catch {
    return false
  }
}

/**
 * One `EventSource` per tab, scoped to the current workspace route (docs/web/03 §SSE, 04 §Data layer).
 * Maps events to query invalidations, reconnects with jittered backoff, and treats anything it cannot
 * prove it received (fresh stream lineage, `resync`) as "refetch everything".
 */
export class SseManager {
  private readonly qc: QueryClient
  private factory: EventSourceFactory
  private readonly random: () => number
  private readonly probe: () => Promise<boolean>
  private ui: UiHooks | undefined

  private source: EventSourceLike | null = null
  private workspaceId: string | null = null
  private wanted = false
  private lastId = ""
  private everOpened = false
  private manualReopen = false
  /** The stream we re-opened replaces one that had been healthy, so events may have been missed. */
  private gapPossible = false
  private attempt = 0
  private retryTimer: ReturnType<typeof setTimeout> | null = null
  private watchdog: ReturnType<typeof setTimeout> | null = null

  constructor(options: SseOptions) {
    this.qc = options.queryClient
    this.factory = options.factory ?? defaultFactory
    this.random = options.random ?? Math.random
    this.probe = options.probe ?? defaultProbe
    this.ui = options.ui
  }

  /** Mock mode swaps in an in-page event source before the first `connect`. */
  setFactory(factory: EventSourceFactory) {
    this.factory = factory
  }

  setUiHooks(ui: UiHooks | undefined) {
    this.ui = ui
  }

  /** Open (or move) the stream to `workspaceId`; `null` listens to global events only. */
  connect(workspaceId: string | null) {
    if (this.wanted && this.workspaceId === workspaceId && this.source) return
    this.teardown()
    this.wanted = true
    this.workspaceId = workspaceId
    this.lastId = ""
    this.everOpened = false
    this.manualReopen = false
    this.gapPossible = false
    this.attempt = 0
    this.open()
  }

  disconnect() {
    this.wanted = false
    this.teardown()
    useConnection.getState().reset()
  }

  private teardown() {
    this.clearTimers()
    this.source?.close()
    this.source = null
  }

  private clearTimers() {
    if (this.retryTimer) clearTimeout(this.retryTimer)
    if (this.watchdog) clearTimeout(this.watchdog)
    this.retryTimer = null
    this.watchdog = null
  }

  private open() {
    const conn = useConnection.getState()
    if (conn.status !== "reconnecting") conn.set({ status: "connecting" })
    const source = this.factory(
      eventsUrl(this.workspaceId, this.manualReopen ? this.lastId : undefined)
    )
    this.source = source
    source.onopen = () => this.onOpen(source)
    source.onerror = () => this.onError(source)
    for (const name of EVENT_NAMES) {
      source.addEventListener(name, (event) =>
        this.onFrame(source, name, event)
      )
    }
  }

  private onOpen(source: EventSourceLike) {
    if (source !== this.source) return
    if (this.retryTimer) clearTimeout(this.retryTimer)
    this.retryTimer = null
    const conn = useConnection.getState()
    const recovered = conn.status === "reconnecting"
    this.everOpened = true
    this.attempt = 0
    conn.set({
      status: "open",
      downSince: null,
      attempt: 0,
      recoveredAt: recovered ? Date.now() : conn.recoveredAt,
    })
    this.armWatchdog()
  }

  private onError(source: EventSourceLike) {
    if (source !== this.source || !this.wanted) return
    this.clearTimers()
    const conn = useConnection.getState()
    this.attempt += 1
    conn.set({
      status: "reconnecting",
      downSince: conn.downSince ?? Date.now(),
      attempt: this.attempt,
    })
    const delay = backoffDelay(this.attempt - 1, this.random)
    if (source.readyState === CONNECTING) {
      // The browser is retrying by itself (and sends Last-Event-ID); only step in if it stays down.
      this.retryTimer = setTimeout(
        () => this.reopen(),
        delay + NATIVE_RETRY_GRACE_MS
      )
    } else if (source.readyState === CLOSED) {
      this.retryTimer = setTimeout(() => this.reopen(), delay)
    }
  }

  private reopen() {
    if (!this.wanted) return
    this.source?.close()
    this.source = null
    this.manualReopen = true
    this.gapPossible = this.everOpened
    this.open()
  }

  private armWatchdog() {
    if (this.watchdog) clearTimeout(this.watchdog)
    this.watchdog = setTimeout(() => void this.onWatchdog(), WATCHDOG_MS)
  }

  private async onWatchdog() {
    const source = this.source
    if (!source || !this.wanted) return
    if (await this.probe()) {
      // Server is up and the browser still believes the stream is open: just keep waiting.
      if (source === this.source) this.armWatchdog()
      return
    }
    if (source !== this.source) return
    source.close()
    // A closed source reports CLOSED, so the manual retry path applies.
    this.onError(source)
  }

  private onFrame(
    source: EventSourceLike,
    name: EventName,
    event: MessageEvent<string>
  ) {
    if (source !== this.source) return
    this.armWatchdog()
    if (event.lastEventId) this.lastId = event.lastEventId
    let data: unknown
    try {
      data = JSON.parse(event.data)
    } catch {
      return
    }
    if (name === "hello" && this.manualReopen) {
      this.manualReopen = false
      // A hello that carries an id starts a new lineage: the server did not replay what we missed.
      if (event.lastEventId && this.gapPossible) {
        void this.qc.invalidateQueries()
      }
    }
    applyEvent(name, data as EventMap[typeof name], {
      queryClient: this.qc,
      workspaceId: this.workspaceId,
      ui: this.ui,
    })
  }
}
