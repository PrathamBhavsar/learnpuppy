// Derives everything the screen renders from the graph snapshot: the scene, the filter and search
// masks, counts, and the selection mapped to scene indices.
import { useMemo } from "react"

import { useGraph, useGraphSearch } from "@/api/queries/graph"
import { useGraphView } from "@/stores/graphView"

import {
  type GraphFilters,
  countVisible,
  filtersActive,
  matchMask,
  visibilityMask,
} from "./model/filters"
import { type Scene, buildScene } from "./model/scene"
import type { GraphUrl } from "./use-graph-url"

/** Above this many nodes the screen switches to performance mode on its own. */
export const PERFORMANCE_NODE_LIMIT = 10_000
const MAX_DEGREE_CAP = 60

export function useGraphModel(ws: string, url: GraphUrl) {
  const query = useGraph(ws)
  const minDegree = useGraphView((s) => s.minDegree)
  const selection = useGraphView((s) => s.selection)
  const performancePref = useGraphView((s) => s.performance)
  const snapshot = query.data

  const scene: Scene | null = useMemo(
    () => (snapshot && snapshot.positions !== null ? buildScene(snapshot) : null),
    [snapshot]
  )

  const filters: GraphFilters = useMemo(
    () => ({ sources: url.sources, cluster: url.cluster, minDegree }),
    [url.sources, url.cluster, minDegree]
  )
  const visible = useMemo(() => (scene ? visibilityMask(scene, filters) : null), [scene, filters])
  const counts = useMemo(() => (scene ? countVisible(scene, visible) : { nodes: 0, chains: 0 }), [scene, visible])

  const term = url.q.trim()
  const search = useGraphSearch(ws, term)
  const hits = term && search.data ? search.data : null
  const match = useMemo(
    () => (scene && hits ? matchMask(scene, hits.map((h) => h.id)) : null),
    [scene, hits]
  )

  const maxDegree = useMemo(() => {
    if (!scene) return 0
    let max = 0
    for (let i = 0; i < scene.count; i++) if ((scene.degree[i] as number) > max) max = scene.degree[i] as number
    return Math.min(max, MAX_DEGREE_CAP)
  }, [scene])

  const selectedIdx = useMemo(() => {
    if (!scene) return []
    const out: number[] = []
    for (const id of selection.selected) {
      const i = scene.index.get(id)
      if (i !== undefined) out.push(i)
    }
    return out
  }, [scene, selection.selected])
  const activeIdx = scene && selection.active ? (scene.index.get(selection.active) ?? -1) : -1

  const nodeCount = snapshot?.nodes.length ?? 0
  const overLimit = nodeCount > PERFORMANCE_NODE_LIMIT
  const performance = overLimit && performancePref !== "off" ? true : performancePref === "on"

  return {
    query,
    snapshot,
    scene,
    visible,
    match,
    matchCount: hits ? hits.length : null,
    searching: term.length > 0,
    counts,
    totalNodes: scene?.count ?? 0,
    maxDegree,
    selectedIdx,
    activeIdx,
    filters,
    filtersActive: filtersActive(filters),
    overLimit,
    performance,
    layoutPending: Boolean(snapshot) && snapshot?.positions === null,
  }
}
