// Keyboard control of the canvas (docs/web/06): arrows, Tab, Enter, Esc on the focused canvas, plus
// registry hotkeys (listed in the Shortcuts sheet) for fit, zoom and delete.
import { useCallback } from "react"

import { useHotkeys } from "@/hooks/use-hotkey"

import { screenToWorld } from "./canvas/camera"
import type { GraphEngine } from "./canvas/engine"
import {
  type Direction,
  nearestInDirection,
  nearestToPoint,
  stepTab,
  tabOrder,
} from "./model/nav"
import type { Scene } from "./model/scene"

const ARROWS: Record<string, Direction> = {
  ArrowLeft: "left",
  ArrowRight: "right",
  ArrowUp: "up",
  ArrowDown: "down",
}

/** True while a dialog, menu or listbox popup owns the keyboard, so single-letter hotkeys stay quiet. */
function overlayOpen(): boolean {
  return (
    document.querySelector(
      "[role='dialog'], [role='alertdialog'], [role='menu'], [data-radix-popper-content-wrapper]"
    ) !== null
  )
}

type Params = {
  scene: Scene | null
  engine: GraphEngine | null
  visible: Uint8Array | null
  activeIdx: number
  selectedCount: number
  pick: (
    idx: number,
    mode?: "replace" | "toggle",
    opts?: { open?: boolean }
  ) => void
  clear: () => void
  openDrawer: () => void
  requestDelete: () => void
}

export function useGraphKeys({
  scene,
  engine,
  visible,
  activeIdx,
  selectedCount,
  pick,
  clear,
  openDrawer,
  requestDelete,
}: Params) {
  const startIdx = useCallback((): number => {
    if (!scene || !engine) return -1
    const [wx, wy] = screenToWorld(
      engine.camera.cam,
      engine.size.w / 2,
      engine.size.h / 2
    )
    return nearestToPoint(scene, visible, wx, wy)
  }, [scene, engine, visible])

  const onKeyDown = useCallback(
    (e: React.KeyboardEvent<HTMLElement>) => {
      if (!scene || !engine || e.target !== e.currentTarget) return
      if (e.metaKey || e.ctrlKey || e.altKey) return
      const dir = ARROWS[e.key]
      if (dir) {
        const from = activeIdx >= 0 ? activeIdx : -1
        const to =
          from < 0 ? startIdx() : nearestInDirection(scene, visible, from, dir)
        if (to >= 0)
          pick(to, e.shiftKey ? "toggle" : "replace", { open: false })
        e.preventDefault()
        return
      }
      switch (e.key) {
        case "Tab": {
          const next = stepTab(tabOrder(scene, visible), activeIdx, e.shiftKey)
          // At either end Tab leaves the canvas instead of trapping focus.
          if (next === null) return
          pick(next, "replace", { open: false })
          e.preventDefault()
          return
        }
        case "Enter":
          if (activeIdx >= 0) {
            openDrawer()
            e.preventDefault()
          }
          return
        case "Escape":
          if (!engine.cancelMarquee() && selectedCount > 0) clear()
          e.preventDefault()
          return
        case "+":
          engine.zoomBy(1.4)
          e.preventDefault()
          return
        default:
      }
    },
    [
      scene,
      engine,
      visible,
      activeIdx,
      selectedCount,
      pick,
      clear,
      openDrawer,
      startIdx,
    ]
  )

  useHotkeys([
    {
      id: "graph-fit",
      keys: "f",
      label: "Fit graph to view",
      group: "Graph",
      enabled: scene !== null,
      handler: () => {
        if (!overlayOpen()) engine?.fit()
      },
    },
    {
      id: "graph-zoom-in",
      keys: "=",
      label: "Zoom in",
      group: "Graph",
      enabled: scene !== null,
      handler: () => {
        if (!overlayOpen()) engine?.zoomBy(1.4)
      },
    },
    {
      id: "graph-zoom-out",
      keys: "-",
      label: "Zoom out",
      group: "Graph",
      enabled: scene !== null,
      handler: () => {
        if (!overlayOpen()) engine?.zoomBy(1 / 1.4)
      },
    },
    {
      id: "graph-delete",
      keys: "delete",
      label: "Delete selected propositions",
      group: "Graph",
      enabled: selectedCount > 0,
      handler: () => {
        if (!overlayOpen()) requestDelete()
      },
    },
    {
      id: "graph-delete-backspace",
      keys: "backspace",
      enabled: selectedCount > 0,
      handler: () => {
        if (!overlayOpen()) requestDelete()
      },
    },
  ])

  return onKeyDown
}
