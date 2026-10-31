import { create } from "zustand"

import type { components } from "@/api/schema"

type Schemas = components["schemas"]
type ToolCall = Schemas["ToolCall"]
type TurnError = Schemas["TurnError"]
type IngestProgress = Schemas["IngestProgressData"]

/**
 * Transient streaming state (docs/web/03 §SSE client rule). Server data stays in TanStack Query; this
 * only holds the in-flight turn text between `chat.delta` and `chat.rest`.
 */
export type TurnBuffer = {
  chatId: string
  turnId: string
  text: string
  thinking: string
  calls: ToolCall[]
  lastSeq: number
  /** A seq gap was seen: text is dropped and further deltas ignored until `chat.rest`. */
  discarded: boolean
  error: TurnError | null
}

type DeltaInput = {
  chatId: string
  turnId: string
  seq: number
  kind: "text" | "thinking"
  text: string
}

type DeltaResult = "applied" | "ignored" | "gap"

type LiveState = {
  turns: Record<string, TurnBuffer>
  ingest: Record<string, IngestProgress>
  applyDelta: (delta: DeltaInput) => DeltaResult
  addToolCall: (chatId: string, turnId: string, call: ToolCall) => void
  setTurnError: (chatId: string, turnId: string, error: TurnError) => void
  /** `chat.rest`: the server view is authoritative again. */
  clearTurn: (chatId: string) => void
  setIngest: (progress: IngestProgress) => void
  clearIngest: (runId: string) => void
  reset: () => void
}

function blank(chatId: string, turnId: string): TurnBuffer {
  return {
    chatId,
    turnId,
    text: "",
    thinking: "",
    calls: [],
    lastSeq: 0,
    discarded: false,
    error: null,
  }
}

/** The buffer for `turnId`, starting a fresh one when a new turn begins. */
function bufferFor(
  turns: Record<string, TurnBuffer>,
  chatId: string,
  turnId: string
): TurnBuffer {
  const existing = turns[chatId]
  return existing && existing.turnId === turnId
    ? existing
    : blank(chatId, turnId)
}

export const useLive = create<LiveState>((set, get) => ({
  turns: {},
  ingest: {},
  applyDelta: (delta) => {
    const buffer = bufferFor(get().turns, delta.chatId, delta.turnId)
    if (buffer.discarded || delta.seq <= buffer.lastSeq) return "ignored"
    if (delta.seq !== buffer.lastSeq + 1) {
      set((s) => ({
        turns: {
          ...s.turns,
          [delta.chatId]: {
            ...buffer,
            text: "",
            thinking: "",
            discarded: true,
          },
        },
      }))
      return "gap"
    }
    set((s) => ({
      turns: {
        ...s.turns,
        [delta.chatId]: {
          ...buffer,
          lastSeq: delta.seq,
          text: delta.kind === "text" ? buffer.text + delta.text : buffer.text,
          thinking:
            delta.kind === "thinking"
              ? buffer.thinking + delta.text
              : buffer.thinking,
        },
      },
    }))
    return "applied"
  },
  addToolCall: (chatId, turnId, call) =>
    set((s) => {
      const buffer = bufferFor(s.turns, chatId, turnId)
      if (buffer.calls.some((c) => c.id === call.id)) return s
      return {
        turns: {
          ...s.turns,
          [chatId]: { ...buffer, calls: [...buffer.calls, call] },
        },
      }
    }),
  setTurnError: (chatId, turnId, error) =>
    set((s) => ({
      turns: {
        ...s.turns,
        [chatId]: { ...bufferFor(s.turns, chatId, turnId), error },
      },
    })),
  clearTurn: (chatId) =>
    set((s) => {
      if (!(chatId in s.turns)) return s
      const turns = { ...s.turns }
      delete turns[chatId]
      return { turns }
    }),
  setIngest: (progress) =>
    set((s) => ({ ingest: { ...s.ingest, [progress.runId]: progress } })),
  clearIngest: (runId) =>
    set((s) => {
      const ingest = { ...s.ingest }
      delete ingest[runId]
      return { ingest }
    }),
  reset: () => set({ turns: {}, ingest: {} }),
}))
