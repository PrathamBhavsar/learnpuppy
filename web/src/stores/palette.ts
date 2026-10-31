import { create } from "zustand"

import { storage } from "@/lib/storage"

const RECENTS_KEY = "lp-palette-recents"
const MAX_RECENTS = 5

type PaletteScope = "workspace" | "all"

type PaletteState = {
  open: boolean
  query: string
  /** Node search scope; `Tab` widens it to every workspace. */
  scope: PaletteScope
  /** Ids of recently run commands, newest first. */
  recents: string[]
  setOpen: (open: boolean) => void
  toggle: () => void
  setQuery: (query: string) => void
  setScope: (scope: PaletteScope) => void
  remember: (commandId: string) => void
}

function readRecents(): string[] {
  try {
    const parsed: unknown = JSON.parse(storage.get(RECENTS_KEY) ?? "[]")
    return Array.isArray(parsed)
      ? parsed
          .filter((x): x is string => typeof x === "string")
          .slice(0, MAX_RECENTS)
      : []
  } catch {
    return []
  }
}

export const usePalette = create<PaletteState>((set, get) => ({
  open: false,
  query: "",
  scope: "workspace",
  recents: readRecents(),
  // Closing clears the query so the next open starts fresh.
  setOpen: (open) =>
    set(open ? { open } : { open, query: "", scope: "workspace" }),
  toggle: () => get().setOpen(!get().open),
  setQuery: (query) => set({ query }),
  setScope: (scope) => set({ scope }),
  remember: (commandId) => {
    const recents = [
      commandId,
      ...get().recents.filter((id) => id !== commandId),
    ].slice(0, MAX_RECENTS)
    storage.set(RECENTS_KEY, JSON.stringify(recents))
    set({ recents })
  },
}))
