import { create } from "zustand"

/** App-wide failure flags raised by the query/mutation caches (docs/web/04 §Data layer, Errors). */
type StatusState = {
  /** The server rejected the session: show the full-screen "open from terminal" state. */
  authRequired: boolean
  /** A request failed with `engine_unavailable`; cleared once an account is ready again. */
  engineUnavailable: boolean
  setAuthRequired: (on: boolean) => void
  setEngineUnavailable: (on: boolean) => void
}

export const useStatus = create<StatusState>((set) => ({
  authRequired: false,
  engineUnavailable: false,
  setAuthRequired: (authRequired) => set({ authRequired }),
  setEngineUnavailable: (engineUnavailable) => set({ engineUnavailable }),
}))
