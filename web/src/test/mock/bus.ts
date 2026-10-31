import { FakeEventSource } from "../fake-event-source"

const RING = 500

type Frame = {
  id: number
  type: string
  data: unknown
  workspaceId: string | null
}

/**
 * In-page stand-in for the server's SSE hub (mock mode): numbers events, keeps a replay ring and
 * delivers them to the open fake sources whose workspace scope matches.
 */
class MockEventBus {
  private nextId = 1
  private ring: Frame[] = []
  /** While true new connections fail, as if the server were unreachable. */
  down = false

  attach(source: FakeEventSource): void {
    queueMicrotask(() => {
      if (source.readyState === 2) return
      if (this.down) {
        source.drop("closed")
        return
      }
      source.open()
      const since = source.lastEventIdParam
      if (since !== null) this.replay(source, Number(since))
      else
        source.emit(
          "hello",
          { serverTime: Date.now(), version: "mock" },
          this.nextId - 1 || 0
        )
    })
  }

  private replay(source: FakeEventSource, since: number): void {
    const oldest = this.ring[0]?.id ?? this.nextId
    source.emit("hello", { serverTime: Date.now(), version: "mock" })
    if (since + 1 < oldest) {
      source.emit("resync", {}, this.nextId - 1)
      return
    }
    for (const frame of this.ring) {
      if (frame.id > since && this.visible(source, frame))
        source.emit(frame.type, frame.data, frame.id)
    }
  }

  private visible(source: FakeEventSource, frame: Frame): boolean {
    return frame.workspaceId === null || source.workspace === frame.workspaceId
  }

  /** `workspaceId` null = a global event every stream receives. */
  emit(type: string, data: unknown, workspaceId: string | null = null): void {
    const frame: Frame = { id: this.nextId++, type, data, workspaceId }
    this.ring.push(frame)
    if (this.ring.length > RING) this.ring.shift()
    if (this.down) return
    for (const source of FakeEventSource.instances) {
      if (source.readyState === 1 && this.visible(source, frame))
        source.emit(type, data, frame.id)
    }
  }

  /** Simulate the server vanishing: open streams error and new ones fail until `restore()`. */
  drop(): void {
    this.down = true
    for (const source of FakeEventSource.instances) {
      if (source.readyState === 1) source.drop("closed")
    }
  }

  restore(): void {
    this.down = false
  }

  reset(): void {
    this.nextId = 1
    this.ring = []
    this.down = false
  }
}

export const bus = new MockEventBus()
