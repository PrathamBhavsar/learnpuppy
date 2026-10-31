import { QueryClient } from "@tanstack/react-query"
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest"

import { useConnection } from "@/stores/connection"
import { useLive } from "@/stores/live"
import { FakeEventSource } from "@/test/fake-event-source"

import {
  BACKOFF_MAX_MS,
  BACKOFF_MIN_MS,
  NATIVE_RETRY_GRACE_MS,
  SseManager,
  WATCHDOG_MS,
  backoffDelay,
  eventsUrl,
} from "./events"
import { keys } from "./keys"

let qc: QueryClient
let manager: SseManager
let probe: ReturnType<typeof vi.fn<() => Promise<boolean>>>

beforeEach(() => {
  vi.useFakeTimers()
  qc = new QueryClient()
  probe = vi.fn(async () => true)
  manager = new SseManager({
    queryClient: qc,
    factory: (url) => new FakeEventSource(url),
    random: () => 0.5,
    probe,
  })
})

afterEach(() => {
  manager.disconnect()
  vi.useRealTimers()
})

const seed = (key: readonly unknown[]) => qc.setQueryData(key, { seeded: true })
const stale = (key: readonly unknown[]) =>
  qc.getQueryState(key)?.isInvalidated === true

describe("backoffDelay", () => {
  it("doubles from 0.5 s up to the 8 s cap with ±20% jitter", () => {
    expect(backoffDelay(0, () => 0.5)).toBe(BACKOFF_MIN_MS)
    expect(backoffDelay(1, () => 0.5)).toBe(1000)
    expect(backoffDelay(3, () => 0.5)).toBe(4000)
    expect(backoffDelay(10, () => 0.5)).toBe(BACKOFF_MAX_MS)
    expect(backoffDelay(0, () => 0)).toBe(400)
    expect(backoffDelay(0, () => 1)).toBe(600)
    expect(backoffDelay(10, () => 1)).toBe(BACKOFF_MAX_MS)
  })
})

describe("eventsUrl", () => {
  it("scopes by workspace and carries the resume id", () => {
    expect(eventsUrl(null)).toBe("/api/v1/events")
    expect(eventsUrl("ws_a")).toBe("/api/v1/events?workspace=ws_a")
    expect(eventsUrl("ws_a", "42")).toBe(
      "/api/v1/events?workspace=ws_a&lastEventId=42"
    )
  })
})

describe("SseManager connection", () => {
  it("opens one stream for the workspace and reports state", () => {
    manager.connect("ws_a")
    expect(FakeEventSource.instances).toHaveLength(1)
    expect(FakeEventSource.last.url).toContain("workspace=ws_a")
    expect(useConnection.getState().status).toBe("connecting")
    FakeEventSource.last.open()
    expect(useConnection.getState().status).toBe("open")
  })

  it("does not reopen for the same workspace but moves the stream on a change", () => {
    manager.connect("ws_a")
    manager.connect("ws_a")
    expect(FakeEventSource.instances).toHaveLength(1)
    const first = FakeEventSource.last
    manager.connect("ws_b")
    expect(first.readyState).toBe(2)
    expect(FakeEventSource.instances).toHaveLength(2)
    expect(FakeEventSource.last.url).toContain("workspace=ws_b")
  })

  it("lets the browser retry first, then takes over after the grace period", () => {
    manager.connect("ws_a")
    const first = FakeEventSource.last
    first.open()
    first.emit("hello", {}, 7)
    first.drop("native")
    expect(useConnection.getState()).toMatchObject({
      status: "reconnecting",
      attempt: 1,
    })
    expect(FakeEventSource.instances).toHaveLength(1)
    vi.advanceTimersByTime(BACKOFF_MIN_MS + NATIVE_RETRY_GRACE_MS - 1)
    expect(FakeEventSource.instances).toHaveLength(1)
    vi.advanceTimersByTime(1)
    expect(FakeEventSource.instances).toHaveLength(2)
    expect(FakeEventSource.last.url).toContain("lastEventId=7")
  })

  it("reconnects a closed stream with exponential backoff and resets after success", () => {
    manager.connect("ws_a")
    FakeEventSource.last.open()
    FakeEventSource.last.drop("closed")
    vi.advanceTimersByTime(BACKOFF_MIN_MS)
    expect(FakeEventSource.instances).toHaveLength(2)
    FakeEventSource.last.drop("closed")
    expect(useConnection.getState().attempt).toBe(2)
    vi.advanceTimersByTime(999)
    expect(FakeEventSource.instances).toHaveLength(2)
    vi.advanceTimersByTime(1)
    expect(FakeEventSource.instances).toHaveLength(3)

    FakeEventSource.last.open()
    const state = useConnection.getState()
    expect(state).toMatchObject({ status: "open", attempt: 0, downSince: null })
    expect(state.recoveredAt).not.toBeNull()
  })

  it("remembers when the stream went down across attempts", () => {
    vi.setSystemTime(10_000)
    manager.connect("ws_a")
    FakeEventSource.last.open()
    FakeEventSource.last.drop("closed")
    vi.advanceTimersByTime(BACKOFF_MIN_MS)
    FakeEventSource.last.drop("closed")
    expect(useConnection.getState().downSince).toBe(10_000)
  })

  it("stops retrying after disconnect", () => {
    manager.connect("ws_a")
    FakeEventSource.last.open()
    FakeEventSource.last.drop("closed")
    manager.disconnect()
    vi.advanceTimersByTime(60_000)
    expect(FakeEventSource.instances).toHaveLength(1)
    expect(useConnection.getState().status).toBe("idle")
  })

  it("probes the server when no frame arrives for 35 s and re-opens if it is unreachable", async () => {
    manager.connect("ws_a")
    FakeEventSource.last.open()
    await vi.advanceTimersByTimeAsync(WATCHDOG_MS)
    expect(probe).toHaveBeenCalledTimes(1)
    expect(FakeEventSource.instances).toHaveLength(1)

    probe.mockResolvedValue(false)
    await vi.advanceTimersByTimeAsync(WATCHDOG_MS)
    expect(useConnection.getState().status).toBe("reconnecting")
    await vi.advanceTimersByTimeAsync(BACKOFF_MIN_MS)
    expect(FakeEventSource.instances).toHaveLength(2)
  })

  it("any frame feeds the watchdog", async () => {
    manager.connect("ws_a")
    FakeEventSource.last.open()
    await vi.advanceTimersByTimeAsync(WATCHDOG_MS - 1000)
    FakeEventSource.last.emit("hello", {}, 1)
    await vi.advanceTimersByTimeAsync(WATCHDOG_MS - 1000)
    expect(probe).not.toHaveBeenCalled()
  })
})

describe("replay and resync", () => {
  it("keeps caches when the server replays the missed events", () => {
    manager.connect("ws_a")
    FakeEventSource.last.open()
    FakeEventSource.last.emit("hello", {}, 5)
    seed(keys.ws("ws_a").graph())
    FakeEventSource.last.drop("closed")
    vi.advanceTimersByTime(BACKOFF_MIN_MS)
    const next = FakeEventSource.last
    next.open()
    // A replaying server answers hello without an id, then the missed frames.
    next.emit("hello", {})
    next.emit("graph.changed", { version: 2 }, 6)
    expect(stale(keys.ws("ws_a").graph())).toBe(true)
    expect(stale(keys.accounts())).toBe(false)
  })

  it("refetches everything when the new stream starts a fresh lineage", () => {
    manager.connect("ws_a")
    FakeEventSource.last.open()
    FakeEventSource.last.emit("hello", {}, 5)
    seed(keys.accounts())
    FakeEventSource.last.drop("closed")
    vi.advanceTimersByTime(BACKOFF_MIN_MS)
    FakeEventSource.last.open()
    FakeEventSource.last.emit("hello", {}, 1)
    expect(stale(keys.accounts())).toBe(true)
  })

  it("invalidates every query and clears streaming buffers on resync", () => {
    manager.connect("ws_a")
    FakeEventSource.last.open()
    seed(keys.accounts())
    seed(keys.ws("ws_a").chats())
    useLive.getState().applyDelta({
      chatId: "c",
      turnId: "t",
      seq: 1,
      kind: "text",
      text: "hi",
    })
    FakeEventSource.last.emit("resync", {}, 9)
    expect(stale(keys.accounts())).toBe(true)
    expect(stale(keys.ws("ws_a").chats())).toBe(true)
    expect(useLive.getState().turns).toEqual({})
  })
})

describe("event to query table", () => {
  beforeEach(() => {
    manager.connect("ws_a")
    FakeEventSource.last.open()
  })

  it("invalidates only what the event touches", () => {
    seed(keys.ws("ws_a").graph())
    seed(keys.ws("ws_a").chats())
    seed(keys.accounts())
    FakeEventSource.last.emit("graph.changed", { version: 3 }, 1)
    expect(stale(keys.ws("ws_a").graph())).toBe(true)
    expect(stale(keys.ws("ws_a").chats())).toBe(false)
    expect(stale(keys.accounts())).toBe(false)
  })

  it("refreshes chat views and the chat list on chat.rest and clears the buffer", () => {
    seed(keys.ws("ws_a").chat("c1"))
    seed(keys.ws("ws_a").chats())
    useLive.getState().applyDelta({
      chatId: "c1",
      turnId: "t",
      seq: 1,
      kind: "text",
      text: "x",
    })
    FakeEventSource.last.emit("chat.rest", { chatId: "c1", viewVersion: 3 }, 2)
    expect(stale(keys.ws("ws_a").chat("c1"))).toBe(true)
    expect(stale(keys.ws("ws_a").chats())).toBe(true)
    expect(useLive.getState().turns["c1"]).toBeUndefined()
  })

  it("invalidates accounts on accounts.changed", () => {
    seed(keys.accounts())
    FakeEventSource.last.emit("accounts.changed", {}, 3)
    expect(stale(keys.accounts())).toBe(true)
  })

  it("ignores malformed frames", () => {
    seed(keys.ws("ws_a").graph())
    expect(() =>
      FakeEventSource.last.emitRaw("graph.changed", "{not json")
    ).not.toThrow()
    expect(stale(keys.ws("ws_a").graph())).toBe(false)
  })

  it("routes workspace.deleted to the UI hook and drops that workspace's cache", () => {
    const workspaceDeleted = vi.fn()
    manager.setUiHooks({ workspaceDeleted })
    seed(keys.ws("ws_b").graph())
    FakeEventSource.last.emit("workspace.deleted", { workspaceId: "ws_b" }, 4)
    expect(workspaceDeleted).toHaveBeenCalledWith("ws_b")
    expect(qc.getQueryData(keys.ws("ws_b").graph())).toBeUndefined()
  })
})

describe("streaming buffers", () => {
  beforeEach(() => {
    manager.connect("ws_a")
    FakeEventSource.last.open()
  })

  const delta = (seq: number, text: string) => ({
    chatId: "c1",
    turnId: "t1",
    seq,
    kind: "text" as const,
    text,
  })

  it("accumulates deltas in order", () => {
    FakeEventSource.last.emit("chat.delta", delta(1, "Hel"), 1)
    FakeEventSource.last.emit("chat.delta", delta(2, "lo"), 2)
    expect(useLive.getState().turns["c1"]?.text).toBe("Hello")
  })

  it("discards the buffer on a seq gap, refetches the chat, and ignores later deltas until rest", () => {
    seed(keys.ws("ws_a").chat("c1"))
    FakeEventSource.last.emit("chat.delta", delta(1, "Hel"), 1)
    FakeEventSource.last.emit("chat.delta", delta(3, "!"), 2)
    const turn = useLive.getState().turns["c1"]
    expect(turn).toMatchObject({ text: "", discarded: true })
    expect(stale(keys.ws("ws_a").chat("c1"))).toBe(true)
    FakeEventSource.last.emit("chat.delta", delta(4, "?"), 3)
    expect(useLive.getState().turns["c1"]?.text).toBe("")
    FakeEventSource.last.emit("chat.rest", { chatId: "c1", viewVersion: 2 }, 4)
    expect(useLive.getState().turns["c1"]).toBeUndefined()
  })

  it("ignores duplicate (replayed) deltas", () => {
    FakeEventSource.last.emit("chat.delta", delta(1, "A"), 1)
    FakeEventSource.last.emit("chat.delta", delta(1, "A"), 2)
    expect(useLive.getState().turns["c1"]?.text).toBe("A")
  })

  it("records tool calls once and keeps ingest progress until done", () => {
    const call = { id: "k1", name: "quiz" }
    FakeEventSource.last.emit(
      "chat.toolCall",
      { chatId: "c1", turnId: "t1", call },
      1
    )
    FakeEventSource.last.emit(
      "chat.toolCall",
      { chatId: "c1", turnId: "t1", call },
      2
    )
    expect(useLive.getState().turns["c1"]?.calls).toHaveLength(1)
    FakeEventSource.last.emit(
      "ingest.progress",
      { runId: "r1", phase: "scan", done: 1, total: 4 },
      3
    )
    expect(useLive.getState().ingest["r1"]?.done).toBe(1)
    FakeEventSource.last.emit(
      "ingest.done",
      {
        runId: "r1",
        report: {
          sessions: 1,
          newDecisions: 0,
          newCards: 0,
          skipped: 0,
          errors: [],
        },
      },
      4
    )
    expect(useLive.getState().ingest["r1"]).toBeUndefined()
  })
})
