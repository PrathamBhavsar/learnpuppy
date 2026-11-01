// Tiny snapshot builder for unit tests: nodes by id with positions, chains by [from, to].
import type { ChainKind, NodeKind, NodeSource } from "@/api/pending-types"

import { type Scene, type SnapshotInput, buildScene } from "./scene"

export type TestNode = {
  id: string
  x?: number
  y?: number
  source?: NodeSource
  kind?: NodeKind
  cluster?: string
  text?: string
}

export function snapshotOf(
  nodes: TestNode[],
  chains: [string, string, ChainKind?][] = [],
  opts: {
    layoutVersion?: number
    version?: number
    clusters?: { id: string; label: string }[]
  } = {}
): SnapshotInput {
  const positions: Record<string, [number, number]> = {}
  for (const n of nodes)
    if (n.x !== undefined && n.y !== undefined) positions[n.id] = [n.x, n.y]
  const clusterIds =
    opts.clusters ??
    [
      ...new Set(nodes.map((n) => n.cluster).filter((c): c is string => !!c)),
    ].map((id) => ({ id, label: id }))
  return {
    version: opts.version ?? 1,
    layoutVersion: opts.layoutVersion ?? 1,
    nodes: nodes.map((n) => ({
      id: n.id,
      text: n.text ?? `text ${n.id}`,
      kind: n.kind ?? "claim",
      sessionId: "s1",
      provenance: n.source ?? "claude",
      createdAt: 0,
      degree: 0,
    })),
    chains: chains.map(([from, to, kind], i) => ({
      id: `c${i + 1}`,
      from,
      to,
      kind: kind ?? "supports",
    })),
    clusters: clusterIds.map((c) => ({
      id: c.id,
      label: c.label,
      nodeIds: nodes.filter((n) => n.cluster === c.id).map((n) => n.id),
    })),
    positions,
  }
}

export function sceneOf(...args: Parameters<typeof snapshotOf>): Scene {
  return buildScene(snapshotOf(...args))
}
