import { create } from "zustand"

type ConnectionStatus =
  /** No stream wanted (before mount or after teardown). */
  "idle" | "connecting" | "open" | "reconnecting"

type ConnectionState = {
  status: ConnectionStatus
  /** Epoch ms the stream was last healthy; the banner waits 2 s from here. */
  downSince: number | null
  attempt: number
  /** Flips true when a dropped stream recovers, so the banner can flash the success tint. */
  recoveredAt: number | null
  set: (patch: Partial<Omit<ConnectionState, "set" | "reset">>) => void
  reset: () => void
}

const INITIAL = {
  status: "idle",
  downSince: null,
  attempt: 0,
  recoveredAt: null,
} satisfies Partial<ConnectionState>

export const useConnection = create<ConnectionState>((set) => ({
  ...INITIAL,
  set: (patch) => set(patch),
  reset: () => set(INITIAL),
}))
