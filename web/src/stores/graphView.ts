import { create } from "zustand"

import { storage } from "@/lib/storage"

import {
  EMPTY_SELECTION,
  type Selection,
} from "@/features/graph/model/selection"

export type LabelMode = "auto" | "always" | "never"
type PerformancePref = "auto" | "on" | "off"

/** Where the camera was when the user left the screen; restored when the same layout is shown again. */
type SavedCamera = {
  x: number
  y: number
  k: number
  w: number
  h: number
  layoutVersion: number
}

type Prefs = {
  hulls: boolean
  labels: LabelMode
  minimap: boolean
  followNew: boolean
  table: boolean
  tableHeight: number
  performance: PerformancePref
}

const PREFS_KEY = "lp-graph-prefs"
export const TABLE_MIN = 160
export const TABLE_MAX = 640

const DEFAULT_PREFS: Prefs = {
  hulls: true,
  labels: "auto",
  minimap: true,
  followNew: false,
  table: false,
  tableHeight: 300,
  performance: "auto",
}

function readPrefs(): Prefs {
  try {
    const raw: unknown = JSON.parse(storage.get(PREFS_KEY) ?? "{}")
    if (typeof raw !== "object" || raw === null) return DEFAULT_PREFS
    const r = raw as Record<string, unknown>
    return {
      hulls: typeof r["hulls"] === "boolean" ? r["hulls"] : DEFAULT_PREFS.hulls,
      labels:
        r["labels"] === "always" ||
        r["labels"] === "never" ||
        r["labels"] === "auto"
          ? r["labels"]
          : DEFAULT_PREFS.labels,
      minimap:
        typeof r["minimap"] === "boolean"
          ? r["minimap"]
          : DEFAULT_PREFS.minimap,
      followNew:
        typeof r["followNew"] === "boolean"
          ? r["followNew"]
          : DEFAULT_PREFS.followNew,
      table: typeof r["table"] === "boolean" ? r["table"] : DEFAULT_PREFS.table,
      tableHeight:
        typeof r["tableHeight"] === "number"
          ? Math.min(TABLE_MAX, Math.max(TABLE_MIN, r["tableHeight"]))
          : DEFAULT_PREFS.tableHeight,
      performance:
        r["performance"] === "on" ||
        r["performance"] === "off" ||
        r["performance"] === "auto"
          ? r["performance"]
          : DEFAULT_PREFS.performance,
    }
  } catch {
    return DEFAULT_PREFS
  }
}

type GraphViewState = Prefs & {
  /** Workspace the transient state below belongs to; switching resets it. */
  wsId: string | null
  selection: Selection
  minDegree: number
  cameras: Record<string, SavedCamera>
  bind: (wsId: string) => void
  setSelection: (selection: Selection) => void
  clearSelection: () => void
  setMinDegree: (value: number) => void
  setPref: <K extends keyof Prefs>(key: K, value: Prefs[K]) => void
  saveCamera: (wsId: string, camera: SavedCamera) => void
}

export const useGraphView = create<GraphViewState>((set, get) => ({
  ...readPrefs(),
  wsId: null,
  selection: EMPTY_SELECTION,
  minDegree: 0,
  cameras: {},
  bind: (wsId) => {
    if (get().wsId === wsId) return
    set({ wsId, selection: EMPTY_SELECTION, minDegree: 0 })
  },
  setSelection: (selection) => set({ selection }),
  clearSelection: () => {
    if (get().selection.selected.length > 0) set({ selection: EMPTY_SELECTION })
  },
  setMinDegree: (minDegree) => set({ minDegree }),
  setPref: (key, value) => {
    set({ [key]: value } as Partial<GraphViewState>)
    const s = get()
    storage.set(
      PREFS_KEY,
      JSON.stringify({
        hulls: s.hulls,
        labels: s.labels,
        minimap: s.minimap,
        followNew: s.followNew,
        table: s.table,
        tableHeight: s.tableHeight,
        performance: s.performance,
      } satisfies Prefs)
    )
  },
  saveCamera: (wsId, camera) =>
    set((s) => ({ cameras: { ...s.cameras, [wsId]: camera } })),
}))
