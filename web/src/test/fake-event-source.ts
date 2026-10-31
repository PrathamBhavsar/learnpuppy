import type { EventSourceLike } from "@/api/events"

type Listener = (event: MessageEvent<string>) => void

/**
 * Scriptable stand-in for `EventSource` (Vitest and mock mode). Nothing happens until the test calls
 * `open()`, `emit()` or `drop()`, so reconnect, replay and resync paths are fully deterministic.
 */
export class FakeEventSource implements EventSourceLike {
  static instances: FakeEventSource[] = []
  static reset() {
    for (const source of FakeEventSource.instances) source.close()
    FakeEventSource.instances = []
  }
  static get last(): FakeEventSource {
    const last = FakeEventSource.instances[FakeEventSource.instances.length - 1]
    if (!last) throw new Error("no FakeEventSource has been created")
    return last
  }

  readyState = 0
  onopen: ((event: Event) => void) | null = null
  onerror: ((event: Event) => void) | null = null
  readonly url: string
  /** Cumulative last event id, as `MessageEvent.lastEventId` reports it. */
  lastEventId = ""
  private readonly listeners = new Map<string, Listener[]>()

  constructor(url: string) {
    this.url = url
    FakeEventSource.instances.push(this)
  }

  get workspace(): string | null {
    return new URL(this.url, "http://localhost").searchParams.get("workspace")
  }
  get lastEventIdParam(): string | null {
    return new URL(this.url, "http://localhost").searchParams.get("lastEventId")
  }

  addEventListener(type: string, listener: Listener): void {
    this.listeners.set(type, [...(this.listeners.get(type) ?? []), listener])
  }

  close(): void {
    this.readyState = 2
  }

  /** The connection is established. */
  open(): void {
    this.readyState = 1
    this.onopen?.(new Event("open"))
  }

  /** Deliver a server frame. `id` omitted = the frame carries no `id:` field. */
  emit(type: string, data: unknown = {}, id?: string | number): void {
    if (id !== undefined) this.lastEventId = String(id)
    const event = new MessageEvent(type, {
      data: JSON.stringify(data),
      lastEventId: this.lastEventId,
    })
    for (const listener of this.listeners.get(type) ?? []) listener(event)
  }

  /** Deliver a frame whose payload is not JSON (malformed server output). */
  emitRaw(type: string, raw: string): void {
    const event = new MessageEvent(type, {
      data: raw,
      lastEventId: this.lastEventId,
    })
    for (const listener of this.listeners.get(type) ?? []) listener(event)
  }

  /**
   * The connection broke. `native`: the browser keeps retrying by itself (readyState CONNECTING).
   * `closed`: the browser gave up (HTTP error), readyState CLOSED.
   */
  drop(mode: "native" | "closed" = "native"): void {
    this.readyState = mode === "native" ? 0 : 2
    this.onerror?.(new Event("error"))
  }
}
