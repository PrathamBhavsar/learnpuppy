// Scene: the typed-array form of a GraphSnapshot that the renderer, hit-test and keyboard navigation
// read. Built once per snapshot; everything per-frame indexes into these arrays, never into objects.
import type {
  ChainKind,
  GraphSnapshot,
  NodeKind,
  NodeSource,
} from "@/api/pending-types"
import type { Bounds } from "../canvas/camera"

import { type Point, centroid, paddedHull } from "./hull"

export const SOURCES: readonly NodeSource[] = [
  "claude",
  "codex",
  "agy",
  "git",
  "chat",
]
export const NODE_KINDS: readonly NodeKind[] = [
  "claim",
  "concept",
  "procedure",
  "decision",
]
const CHAIN_KINDS: readonly ChainKind[] = [
  "supports",
  "implies",
  "contrasts",
  "part_of",
]
/** `--cluster-1..8` repeat for graphs with more clusters than hues. */
const CLUSTER_HUES = 8

type SceneCluster = {
  id: string
  label: string
  /** 1..8, picks `--cluster-N`. */
  slot: number
  nodes: Uint32Array
  centroid: Point
  /** Padded convex hull in world space; empty for clusters with no placed nodes. */
  hull: Point[]
}

export type Scene = {
  version: number
  layoutVersion: number
  count: number
  ids: string[]
  texts: string[]
  sessionIds: string[]
  index: Map<string, number>
  x: Float32Array
  y: Float32Array
  /** Size multiplier from degree (about 0.8..2.2). */
  weight: Float32Array
  source: Uint8Array
  kind: Uint8Array
  /** Cluster index into `clusters`, or -1. */
  cluster: Int16Array
  degree: Uint32Array
  createdAt: Float64Array
  edgeCount: number
  edgeIds: string[]
  edgeFrom: Uint32Array
  edgeTo: Uint32Array
  edgeKind: Uint8Array
  /** CSR adjacency (undirected): neighbours of i are adjList[adjStart[i]..adjStart[i+1]]. */
  adjStart: Uint32Array
  adjList: Uint32Array
  clusters: SceneCluster[]
  /** Node indices by degree (desc), then id: label priority, Tab order and GraphList order. */
  order: Uint32Array
  bounds: Bounds
}

/** FNV-1a so unplaced nodes land in the same spot on every load. */
function hash01(text: string, salt: number): number {
  let h = 2166136261 ^ salt
  for (let i = 0; i < text.length; i++) {
    h ^= text.charCodeAt(i)
    h = Math.imul(h, 16777619)
  }
  return ((h >>> 0) % 100000) / 100000
}

/** Share of the farthest points left out of a hull so a few strays do not balloon the silhouette. */
const HULL_TRIM = 0.06
const HULL_TRIM_MIN_POINTS = 40

function trimOutliers(points: Point[], center: Point): Point[] {
  if (points.length < HULL_TRIM_MIN_POINTS) return points
  const keep = Math.ceil(points.length * (1 - HULL_TRIM))
  return points
    .map((p) => ({ p, d: Math.hypot(p[0] - center[0], p[1] - center[1]) }))
    .sort((a, b) => a.d - b.d)
    .slice(0, keep)
    .map((e) => e.p)
}

export function weightForDegree(degree: number): number {
  return Math.min(2.2, 0.8 + 0.3 * Math.sqrt(degree))
}

/** Compares node ids so `order` is stable and total. */
const byDegreeThenId =
  (degree: Uint32Array, ids: string[]) =>
  (a: number, b: number): number =>
    (degree[b] as number) - (degree[a] as number) ||
    ((ids[a] as string) < (ids[b] as string) ? -1 : 1)

/**
 * Builds the scene. Nodes the server did not position (new since the last layout) are placed at their
 * cluster's centroid with a deterministic offset so they appear in a sensible place until relayout.
 */
/** The snapshot as the API client types it (tuples arrive as `number[]`). */
export type SnapshotInput = Omit<GraphSnapshot, "positions"> & {
  positions: Record<string, readonly number[]> | null
}

export function buildScene(snapshot: SnapshotInput): Scene {
  const { nodes, chains, clusters: rawClusters, positions } = snapshot
  const count = nodes.length
  const ids = new Array<string>(count)
  const texts = new Array<string>(count)
  const sessionIds = new Array<string>(count)
  const index = new Map<string, number>()
  const x = new Float32Array(count)
  const y = new Float32Array(count)
  const source = new Uint8Array(count)
  const kind = new Uint8Array(count)
  const cluster = new Int16Array(count).fill(-1)
  const createdAt = new Float64Array(count)
  const placed = new Uint8Array(count)

  nodes.forEach((node, i) => {
    ids[i] = node.id
    texts[i] = node.text
    sessionIds[i] = node.sessionId
    index.set(node.id, i)
    source[i] = Math.max(0, SOURCES.indexOf(node.provenance))
    kind[i] = Math.max(0, NODE_KINDS.indexOf(node.kind))
    createdAt[i] = node.createdAt
    const pos = positions?.[node.id]
    if (pos && pos.length >= 2) {
      x[i] = pos[0] as number
      y[i] = pos[1] as number
      placed[i] = 1
    }
  })

  rawClusters.forEach((c, ci) => {
    for (const id of c.nodeIds) {
      const i = index.get(id)
      if (i !== undefined) cluster[i] = ci
    }
  })

  // Edges (skip chains with a missing endpoint, e.g. a node deleted between fetches).
  const edgeFrom: number[] = []
  const edgeTo: number[] = []
  const edgeKind: number[] = []
  const edgeIds: string[] = []
  const degree = new Uint32Array(count)
  for (const chain of chains) {
    const a = index.get(chain.from)
    const b = index.get(chain.to)
    if (a === undefined || b === undefined || a === b) continue
    edgeFrom.push(a)
    edgeTo.push(b)
    edgeKind.push(Math.max(0, CHAIN_KINDS.indexOf(chain.kind)))
    edgeIds.push(chain.id)
    degree[a] = (degree[a] as number) + 1
    degree[b] = (degree[b] as number) + 1
  }
  const edgeCount = edgeFrom.length

  const adjStart = new Uint32Array(count + 1)
  for (let e = 0; e < edgeCount; e++) {
    adjStart[(edgeFrom[e] as number) + 1] = (adjStart[(edgeFrom[e] as number) + 1] as number) + 1
    adjStart[(edgeTo[e] as number) + 1] = (adjStart[(edgeTo[e] as number) + 1] as number) + 1
  }
  for (let i = 0; i < count; i++)
    adjStart[i + 1] = (adjStart[i + 1] as number) + (adjStart[i] as number)
  const cursor = adjStart.slice(0, count)
  const adjList = new Uint32Array(edgeCount * 2)
  for (let e = 0; e < edgeCount; e++) {
    const a = edgeFrom[e] as number
    const b = edgeTo[e] as number
    adjList[cursor[a] as number] = b
    cursor[a] = (cursor[a] as number) + 1
    adjList[cursor[b] as number] = a
    cursor[b] = (cursor[b] as number) + 1
  }

  // Cluster centroids from placed nodes, then place the stragglers.
  const sceneClusters: SceneCluster[] = rawClusters.map((c, ci) => {
    const members: number[] = []
    for (let i = 0; i < count; i++) if (cluster[i] === ci) members.push(i)
    const placedPts: Point[] = members
      .filter((i) => placed[i] === 1)
      .map((i) => [x[i] as number, y[i] as number])
    return {
      id: c.id,
      label: c.label,
      slot: (ci % CLUSTER_HUES) + 1,
      nodes: Uint32Array.from(members),
      centroid: centroid(placedPts),
      hull: [],
    }
  })
  const allPlaced: Point[] = []
  for (let i = 0; i < count; i++) if (placed[i] === 1) allPlaced.push([x[i] as number, y[i] as number])
  const origin = centroid(allPlaced)
  const clusterPlaced = sceneClusters.map((c) =>
    c.nodes.some((n) => placed[n] === 1)
  )
  for (let i = 0; i < count; i++) {
    if (placed[i] === 1) continue
    const ci = cluster[i] as number
    const base = ci >= 0 && clusterPlaced[ci] ? (sceneClusters[ci] as SceneCluster).centroid : origin
    const angle = hash01(ids[i] as string, 1) * Math.PI * 2
    const radius = 30 + hash01(ids[i] as string, 2) * 120
    x[i] = base[0] + Math.cos(angle) * radius
    y[i] = base[1] + Math.sin(angle) * radius
  }

  let minX = Infinity
  let minY = Infinity
  let maxX = -Infinity
  let maxY = -Infinity
  for (let i = 0; i < count; i++) {
    const px = x[i] as number
    const py = y[i] as number
    if (px < minX) minX = px
    if (px > maxX) maxX = px
    if (py < minY) minY = py
    if (py > maxY) maxY = py
  }
  if (count === 0) {
    minX = minY = -1
    maxX = maxY = 1
  }
  const bounds: Bounds = { minX, minY, maxX, maxY }

  const diag = Math.hypot(maxX - minX, maxY - minY)
  const pad = Math.max(diag * 0.018, 8)
  for (const c of sceneClusters) {
    const pts: Point[] = Array.from(c.nodes, (i) => [x[i] as number, y[i] as number])
    c.centroid = centroid(pts)
    c.hull = paddedHull(trimOutliers(pts, c.centroid), pad)
  }

  const weight = new Float32Array(count)
  for (let i = 0; i < count; i++) weight[i] = weightForDegree(degree[i] as number)
  const order = Uint32Array.from({ length: count }, (_, i) => i).sort(
    byDegreeThenId(degree, ids)
  )

  return {
    version: snapshot.version,
    layoutVersion: snapshot.layoutVersion,
    count,
    ids,
    texts,
    sessionIds,
    index,
    x,
    y,
    weight,
    source,
    kind,
    cluster,
    degree,
    createdAt,
    edgeCount,
    edgeIds,
    edgeFrom: Uint32Array.from(edgeFrom),
    edgeTo: Uint32Array.from(edgeTo),
    edgeKind: Uint8Array.from(edgeKind),
    adjStart,
    adjList,
    clusters: sceneClusters,
    order,
    bounds,
  }
}

/** Neighbours of node `i` as a view into the CSR list. */
export function neighbours(scene: Scene, i: number): Uint32Array {
  return scene.adjList.subarray(
    scene.adjStart[i] as number,
    scene.adjStart[i + 1] as number
  )
}
