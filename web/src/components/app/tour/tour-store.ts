import { create } from "zustand"

import { storage } from "@/lib/storage"

/** localStorage key; any value means the first-run tour was finished or skipped. */
export const TOUR_KEY = "lp-tour"

type TourPhase = "idle" | "splash" | "tour"

type TourState = {
  phase: TourPhase
  /** Welcome splash, then the spotlight tour (first visit). */
  startFirstRun: () => void
  /** Spotlight tour only (Help menu, Settings). */
  replay: () => void
  startTour: () => void
  /** Finish or skip: remembered so it never reappears on its own. */
  finish: () => void
}

export const useTour = create<TourState>((set) => ({
  phase: "idle",
  startFirstRun: () => set({ phase: "splash" }),
  replay: () => set({ phase: "tour" }),
  startTour: () => set({ phase: "tour" }),
  finish: () => {
    storage.set(TOUR_KEY, "done")
    set({ phase: "idle" })
  },
}))

/** True when the tour should open by itself: never seen, and not disabled with `?notour=1`. */
export function shouldAutoStart(search: string): boolean {
  if (new URLSearchParams(search).get("notour") === "1") return false
  return storage.get(TOUR_KEY) === null
}
