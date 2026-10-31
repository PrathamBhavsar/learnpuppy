import { create } from "zustand"

import { storage } from "@/lib/storage"
import {
  DENSITY_KEY,
  DIR_KEY,
  REDUCE_MOTION_KEY,
  THEME_KEY,
  type Density,
  type Direction,
  type ResolvedTheme,
  type Theme,
  applyDensity,
  applyDirection,
  applyReduceMotion,
  applyResolvedTheme,
  readDensity,
  readDirection,
  readTheme,
  resolveTheme,
  withViewTransition,
} from "@/lib/theme"

const SIDEBAR_KEY = "lp-sidebar-collapsed"
const LAST_WS_KEY = "lp-last-workspace"

type UiState = {
  theme: Theme
  resolvedTheme: ResolvedTheme
  direction: Direction
  reduceMotion: boolean
  density: Density
  sidebarCollapsed: boolean
  lastWorkspace: string | null
  setLastWorkspace: (id: string) => void
  setTheme: (theme: Theme) => void
  /** Re-resolve after the OS color scheme changed (only matters for `system`). */
  syncSystemTheme: () => void
  setDirection: (direction: Direction) => void
  setReduceMotion: (on: boolean) => void
  setDensity: (density: Density) => void
  setSidebarCollapsed: (collapsed: boolean) => void
  toggleSidebar: () => void
}

export const useUi = create<UiState>((set, get) => {
  const theme = readTheme()
  return {
    theme,
    resolvedTheme: resolveTheme(theme),
    direction: readDirection(),
    reduceMotion: storage.get(REDUCE_MOTION_KEY) === "1",
    density: readDensity(),
    sidebarCollapsed: storage.get(SIDEBAR_KEY) === "1",
    lastWorkspace: storage.get(LAST_WS_KEY),
    setLastWorkspace: (id) => {
      if (get().lastWorkspace === id) return
      storage.set(LAST_WS_KEY, id)
      set({ lastWorkspace: id })
    },
    setTheme: (next) => {
      storage.set(THEME_KEY, next)
      const resolvedTheme = resolveTheme(next)
      withViewTransition(() => applyResolvedTheme(resolvedTheme))
      set({ theme: next, resolvedTheme })
    },
    syncSystemTheme: () => {
      if (get().theme !== "system") return
      const resolvedTheme = resolveTheme("system")
      withViewTransition(() => applyResolvedTheme(resolvedTheme))
      set({ resolvedTheme })
    },
    setDirection: (direction) => {
      storage.set(DIR_KEY, direction)
      withViewTransition(() => applyDirection(direction))
      set({ direction })
    },
    setReduceMotion: (on) => {
      storage.set(REDUCE_MOTION_KEY, on ? "1" : "0")
      applyReduceMotion(on)
      set({ reduceMotion: on })
    },
    setDensity: (density) => {
      storage.set(DENSITY_KEY, density)
      applyDensity(density)
      set({ density })
    },
    setSidebarCollapsed: (collapsed) => {
      storage.set(SIDEBAR_KEY, collapsed ? "1" : "0")
      set({ sidebarCollapsed: collapsed })
    },
    toggleSidebar: () => get().setSidebarCollapsed(!get().sidebarCollapsed),
  }
})
