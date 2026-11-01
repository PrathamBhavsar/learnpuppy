// Filtering and search highlighting over a Scene. Both produce flag arrays the renderer, hit-test and
// keyboard navigation share, so "what is visible" is decided in exactly one place.
import type { NodeSource } from "@/api/pending-types"

import { SOURCES, type Scene } from "./scene"

export type GraphFilters = {
  /** Empty = all sources. */
  sources: readonly NodeSource[]
  /** Cluster id, or null for all. */
  cluster: string | null
  minDegree: number
}

export const NO_FILTERS: GraphFilters = {
  sources: [],
  cluster: null,
  minDegree: 0,
}

export function filtersActive(f: GraphFilters): boolean {
  return f.sources.length > 0 || f.cluster !== null || f.minDegree > 0
}

/** 1 = node passes every filter. Null means "no filter", so callers can skip the mask entirely. */
export function visibilityMask(
  scene: Scene,
  f: GraphFilters
): Uint8Array | null {
  if (!filtersActive(f)) return null
  const allowed = new Uint8Array(SOURCES.length)
  if (f.sources.length === 0) allowed.fill(1)
  else for (const s of f.sources) allowed[SOURCES.indexOf(s)] = 1
  const clusterIdx =
    f.cluster === null
      ? -1
      : scene.clusters.findIndex((c) => c.id === f.cluster)
  const mask = new Uint8Array(scene.count)
  for (let i = 0; i < scene.count; i++) {
    if (!allowed[scene.source[i] as number]) continue
    // An unknown cluster id (stale URL) matches nothing; unclustered nodes also carry index -1.
    if (
      f.cluster !== null &&
      (clusterIdx < 0 || scene.cluster[i] !== clusterIdx)
    )
      continue
    if ((scene.degree[i] as number) < f.minDegree) continue
    mask[i] = 1
  }
  return mask
}

export function countVisible(
  scene: Scene,
  mask: Uint8Array | null
): { nodes: number; chains: number } {
  if (!mask) return { nodes: scene.count, chains: scene.edgeCount }
  let nodes = 0
  for (let i = 0; i < scene.count; i++) nodes += mask[i] as number
  let chains = 0
  for (let e = 0; e < scene.edgeCount; e++)
    if (mask[scene.edgeFrom[e] as number] && mask[scene.edgeTo[e] as number])
      chains++
  return { nodes, chains }
}

/** Flags for the node ids a search returned; null when there is no active search. */
export function matchMask(
  scene: Scene,
  ids: readonly string[] | null
): Uint8Array | null {
  if (ids === null) return null
  const mask = new Uint8Array(scene.count)
  for (const id of ids) {
    const i = scene.index.get(id)
    if (i !== undefined) mask[i] = 1
  }
  return mask
}

/** Parses `?src=claude,codex` keeping only known sources, in canonical order. */
export function parseSources(raw: string | undefined): NodeSource[] {
  if (!raw) return []
  const wanted = new Set(raw.split(","))
  return SOURCES.filter((s) => wanted.has(s))
}

export function serializeSources(
  sources: readonly NodeSource[]
): string | undefined {
  if (sources.length === 0) return undefined
  return SOURCES.filter((s) => sources.includes(s)).join(",")
}
