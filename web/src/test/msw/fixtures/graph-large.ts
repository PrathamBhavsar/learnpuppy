import type {
  ChainKind,
  GraphChain,
  GraphCluster,
  GraphNode,
  NodeDetail,
  NodeKind,
  NodeSource,
} from "@/api/pending-types"

import type { GraphFixture } from "./graph"
import { DAY, mulberry32, pick } from "./rng"

const SOURCES: NodeSource[] = ["claude", "codex", "agy", "git", "chat"]
const KINDS: NodeKind[] = ["claim", "concept", "procedure", "decision"]
const CHAIN_KINDS: ChainKind[] = ["supports", "implies", "contrasts", "part_of"]
const SUBJECTS = [
  "scheduler",
  "waker",
  "executor",
  "channel",
  "mutex",
  "future",
  "task",
  "reactor",
  "timer",
  "stream",
]
const VERBS = [
  "wakes",
  "parks",
  "polls",
  "drops",
  "spawns",
  "cancels",
  "buffers",
  "yields to",
  "borrows",
  "joins",
]

/** Node details are derived on demand so a 10k-node fixture does not build 10k objects up front. */
class LazyDetails extends Map<string, NodeDetail> {
  private readonly build: (id: string) => NodeDetail | undefined
  constructor(build: (id: string) => NodeDetail | undefined) {
    super()
    this.build = build
  }
  override get(id: string): NodeDetail | undefined {
    return this.build(id)
  }
}

/**
 * A deterministic graph of `count` nodes in ~`clusters` gaussian blobs with about 1.3 chains per
 * node (hubs included), for the 5k / 10k performance work and the performance-mode tests.
 */
export function buildLargeGraph(
  wsId: string,
  count: number,
  now: number,
  seed: number,
  clusterCount = 8
): GraphFixture {
  const rand = mulberry32(seed)
  const prefix = `n_${wsId.slice(3)}_`
  const sessions = Array.from({ length: 24 }, (_, i) => `sess_${wsId}_${i + 1}`)
  const clusters: GraphCluster[] = Array.from({ length: clusterCount }, (_, i) => ({
    id: `cl_${wsId}_${i + 1}`,
    label: `Topic ${i + 1}`,
    nodeIds: [],
  }))
  // Blob centres on a ring; the radius grows with sqrt(count) so density stays constant.
  const ring = Math.sqrt(count) * 18
  const spread = Math.sqrt(count / clusterCount) * 7
  const nodes: GraphNode[] = new Array<GraphNode>(count)
  const positions: Record<string, [number, number]> = {}
  const owner = new Int32Array(count)
  const gaussian = (): number => {
    let u = 0
    let v = 0
    while (u === 0) u = rand()
    while (v === 0) v = rand()
    return Math.sqrt(-2 * Math.log(u)) * Math.cos(2 * Math.PI * v)
  }
  for (let i = 0; i < count; i++) {
    const c = i % clusterCount
    const angle = (c / clusterCount) * Math.PI * 2
    const id = `${prefix}${String(i + 1).padStart(5, "0")}`
    positions[id] = [
      Math.round(Math.cos(angle) * ring + gaussian() * spread),
      Math.round(Math.sin(angle) * ring + gaussian() * spread),
    ]
    owner[i] = c
    nodes[i] = {
      id,
      text: `The ${pick(rand, SUBJECTS)} ${pick(rand, VERBS)} the ${pick(rand, SUBJECTS)} (${i + 1})`,
      kind: pick(rand, KINDS),
      sessionId: pick(rand, sessions),
      provenance: pick(rand, SOURCES),
      createdAt: now - Math.floor(rand() * 60 * DAY),
      degree: 0,
    }
    clusters[c]?.nodeIds.push(id)
  }

  const chains: GraphChain[] = []
  const seen = new Set<number>()
  const members = clusters.map((c) => c.nodeIds.map((id) => Number(id.slice(prefix.length)) - 1))
  const add = (a: number, b: number): void => {
    if (a === b) return
    const key = a < b ? a * count + b : b * count + a
    if (seen.has(key)) return
    seen.add(key)
    chains.push({
      id: `ch_${wsId.slice(3)}_${String(chains.length + 1).padStart(5, "0")}`,
      from: (nodes[a] as GraphNode).id,
      to: (nodes[b] as GraphNode).id,
      kind: pick(rand, CHAIN_KINDS),
    })
    ;(nodes[a] as GraphNode).degree += 1
    ;(nodes[b] as GraphNode).degree += 1
  }
  const target = Math.round(count * 1.3)
  for (let guard = 0; chains.length < target && guard < target * 6; guard++) {
    const a = Math.floor(rand() * count)
    const list = members[owner[a] as number] as number[]
    // 90% of chains stay inside a cluster; a few preferentially attach to low indices (hubs).
    const local = rand() < 0.9
    const pool = local ? list : null
    const b = pool
      ? (pool[Math.floor(Math.pow(rand(), 2) * pool.length)] as number)
      : Math.floor(rand() * count)
    add(a, b)
  }

  const byId = new Map(nodes.map((n) => [n.id, n]))
  const incident = new Map<string, GraphChain[]>()
  for (const chain of chains) {
    for (const id of [chain.from, chain.to]) {
      const list = incident.get(id)
      if (list) list.push(chain)
      else incident.set(id, [chain])
    }
  }
  const details = new LazyDetails((id) => {
    const node = byId.get(id)
    if (!node) return undefined
    const touching = incident.get(id) ?? []
    const incoming = touching.filter((c) => c.to === id)
    const outgoing = touching.filter((c) => c.from === id)
    return {
      ...node,
      provenanceDetail: {
        chatId: node.provenance === "chat" ? "c_teach_quiz" : null,
        messageIndex: node.provenance === "chat" ? 4 : null,
        snippet: `…noting that ${node.text.charAt(0).toLowerCase()}${node.text.slice(1)} in the trace…`,
      },
      incoming,
      outgoing,
      related: touching.slice(0, 6).map((c) => {
        const other = byId.get(c.from === id ? c.to : c.from) as GraphNode
        return { id: other.id, text: other.text, kind: other.kind }
      }),
      cards: [],
    }
  })

  return {
    snapshot: { version: 1, nodes, chains, clusters, positions, layoutVersion: 1 },
    details,
  }
}

/** Sizes the performance work uses (docs/web/07 §Fixtures: large = 5k, huge = 10k). */
export const LARGE_GRAPH_SIZES = { large: 5000, huge: 10_000 } as const
