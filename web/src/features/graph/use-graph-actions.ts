// The screen's user intents: selecting, hopping, practising, deleting. They update the selection store
// and the URL together and tell the engine where to look, so the screen component stays declarative.
import { useCallback, useEffect, useRef, useState } from "react"

import { usePracticeFromSelection } from "@/api/queries/graph"
import { useGoto } from "@/hooks/use-goto"
import { toastError } from "@/lib/toast"
import { useGraphView } from "@/stores/graphView"

import type { GraphEngine, Insets } from "./canvas/engine"
import { copy } from "./copy"
import {
  EMPTY_SELECTION,
  type Selection,
  pruneSelection,
  selectMany,
  selectOnly,
  toggleSelected,
} from "./model/selection"
import { type Scene, neighbours } from "./model/scene"
import type { GraphUrl } from "./use-graph-url"

type Params = {
  ws: string
  scene: Scene | null
  engine: GraphEngine | null
  url: GraphUrl
  /** Insets for the engine with or without the drawer showing. */
  insetsFor: (drawerOpen: boolean) => Insets
}

type Fly = "none" | "node" | "group"

export function useGraphActions({ ws, scene, engine, url, insetsFor }: Params) {
  const goto = useGoto()
  const selection = useGraphView((s) => s.selection)
  const setSelection = useGraphView((s) => s.setSelection)
  const practice = usePracticeFromSelection(ws)
  const [drawerDismissed, setDrawerDismissed] = useState(false)
  const [focusDrawer, setFocusDrawer] = useState(false)
  const [pulse, setPulse] = useState<number[]>([])
  const [deleteIds, setDeleteIds] = useState<string[] | null>(null)
  const selectionRef = useRef(selection)
  useEffect(() => {
    selectionRef.current = selection
  }, [selection])
  // The id we just wrote to the URL; ignore the URL until our own navigation has landed.
  const echo = useRef<string | null | undefined>(undefined)

  const commit = useCallback(
    (next: Selection, opts: { fly?: Fly; open?: boolean } = {}) => {
      const prev = selectionRef.current
      setSelection(next)
      if (next.active !== url.node) {
        echo.current = next.active
        url.setNode(next.active)
      }
      if (opts.open) setDrawerDismissed(false)
      setFocusDrawer(false)
      if (scene) {
        const fresh = next.selected.filter((id) => !prev.selected.includes(id))
        const added = fresh
          .map((id) => scene.index.get(id))
          .filter((i): i is number => i !== undefined)
        setPulse(added)
        if (added.length > 0) window.setTimeout(() => setPulse([]), 900)
        const idx = next.active ? scene.index.get(next.active) : undefined
        if (idx !== undefined && engine && opts.fly && opts.fly !== "none") {
          engine.setInsets(
            insetsFor(
              next.active !== null && (opts.open === true || !drawerDismissed)
            )
          )
          engine.flyTo(
            opts.fly === "group" ? [idx, ...neighbours(scene, idx)] : [idx]
          )
        }
      }
    },
    [setSelection, url, scene, engine, insetsFor, drawerDismissed]
  )

  // Browser Back/Forward and palette links change ?node=; follow them without echoing back.
  const urlNode = url.node
  useEffect(() => {
    if (!scene) return
    if (echo.current !== undefined) {
      if (urlNode === echo.current) echo.current = undefined
      return
    }
    const current = selectionRef.current
    if (urlNode === current.active) return
    if (urlNode === null) {
      setSelection(
        current.selected.length > 1
          ? { ...current, active: null }
          : EMPTY_SELECTION
      )
      return
    }
    if (!scene.index.has(urlNode)) {
      url.setNode(null)
      return
    }
    // The URL (Back/Forward, palette links) is the external system this effect follows. `commit`
    // echoes the id into the URL only when it differs, which it no longer does here.
    // eslint-disable-next-line react-hooks/set-state-in-effect
    commit(selectOnly(urlNode), { fly: "node", open: true })
    // Only a changed URL node or scene re-syncs; `commit` and `url` change identity every render.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [urlNode, scene])

  // After a refetch, forget selected ids that no longer exist.
  useEffect(() => {
    if (!scene) return
    const pruned = pruneSelection(selectionRef.current, (id) =>
      scene.index.has(id)
    )
    if (pruned !== selectionRef.current) setSelection(pruned)
  }, [scene, setSelection])

  const pickIdx = useCallback(
    (
      idx: number,
      mode: "replace" | "toggle" = "replace",
      opts: { fly?: Fly; open?: boolean } = {}
    ) => {
      const id = scene?.ids[idx]
      if (id === undefined) return
      const next =
        mode === "toggle"
          ? toggleSelected(selectionRef.current, id)
          : selectOnly(id)
      commit(next, { fly: opts.fly ?? "node", open: opts.open ?? true })
    },
    [scene, commit]
  )

  const pickIds = useCallback(
    (ids: readonly string[], opts: { fly?: Fly } = {}) => {
      if (ids.length === 0) return
      const known = ids.filter((id) => scene?.index.has(id))
      const next =
        known.length === 1
          ? selectOnly(known[0] as string)
          : selectMany(EMPTY_SELECTION, known, false)
      commit(known.length === 1 ? next : { ...next, active: null }, {
        fly: opts.fly ?? "none",
        open: true,
      })
      if (known.length > 1 && scene && engine) {
        engine.flyTo(known.map((id) => scene.index.get(id) as number))
      }
    },
    [scene, commit, engine]
  )

  const marquee = useCallback(
    (idx: number[]) => {
      if (!scene || idx.length === 0) return
      const ids = idx.map((i) => scene.ids[i] as string)
      commit(selectMany(selectionRef.current, ids, true), {
        fly: "none",
        open: true,
      })
    },
    [scene, commit]
  )

  const clear = useCallback(() => {
    commit(EMPTY_SELECTION)
  }, [commit])

  const closeDrawer = useCallback(() => setDrawerDismissed(true), [])
  const openDrawer = useCallback(() => {
    setDrawerDismissed(false)
    setFocusDrawer(true)
  }, [])

  const startPractice = useCallback(
    (ids: readonly string[]) => {
      if (ids.length === 0 || practice.isPending) return
      practice.mutate([...ids], {
        onSuccess: ({ chatId }) => goto(`/w/${ws}/learn/${chatId}`),
        onError: (error) => toastError(error, copy.selection.practiceFailed),
      })
    },
    [practice, goto, ws]
  )

  const requestDelete = useCallback((ids: readonly string[]) => {
    if (ids.length > 0) setDeleteIds([...ids])
  }, [])

  const animateOut = useCallback(
    async (ids: readonly string[]) => {
      if (!scene || !engine) return
      const idx = ids
        .map((id) => scene.index.get(id))
        .filter((i): i is number => i !== undefined)
      await engine.markDying(idx)
    },
    [scene, engine]
  )

  const finishDelete = useCallback(() => {
    setDeleteIds(null)
    commit(EMPTY_SELECTION)
  }, [commit])

  return {
    selection,
    drawerOpen: selection.active !== null && !drawerDismissed,
    focusDrawer,
    pulse,
    pickIdx,
    pickIds,
    marquee,
    clear,
    closeDrawer,
    openDrawer,
    startPractice,
    practicePending: practice.isPending,
    deleteIds,
    requestDelete,
    cancelDelete: useCallback(() => setDeleteIds(null), []),
    animateOut,
    finishDelete,
  }
}
