import { graphlib, layout } from "@dagrejs/dagre"

import type { ClassroomView } from "@/api/pending-types"

type Plan = NonNullable<ClassroomView["plan"]>

export const NODE_W = 148
export const NODE_H = 44

type PlacedNode = Plan["nodes"][number] & {
  x: number
  y: number
  /** 0-based position in topological order; drives the staggered reveal. */
  order: number
}
type PlacedEdge = { from: string; to: string; d: string; order: number }
export type PlanLayout = {
  nodes: PlacedNode[]
  edges: PlacedEdge[]
  width: number
  height: number
}

/** Layered dagre layout. `LR` suits a wide panel, `TB` a narrow one. */
export function layoutPlan(plan: Plan, rankdir: "TB" | "LR"): PlanLayout {
  const g = new graphlib.Graph()
  g.setGraph({ rankdir, nodesep: 24, ranksep: 36, marginx: 16, marginy: 16 })
  g.setDefaultEdgeLabel(() => ({}))
  const ids = new Set(plan.nodes.map((n) => n.id))
  for (const node of plan.nodes)
    g.setNode(node.id, { width: NODE_W, height: NODE_H })
  for (const edge of plan.edges)
    if (ids.has(edge.from) && ids.has(edge.to)) g.setEdge(edge.from, edge.to)
  layout(g)

  const rank = (n: { x: number; y: number }) => (rankdir === "TB" ? n.y : n.x)
  const placed = plan.nodes
    .map((node) => {
      const at = g.node(node.id)
      return { ...node, x: at.x - NODE_W / 2, y: at.y - NODE_H / 2, order: 0 }
    })
    .sort((a, b) => rank(a) - rank(b) || a.x - b.x || a.y - b.y)
    .map((node, order) => ({ ...node, order }))
  const orderOf = new Map(placed.map((n) => [n.id, n.order]))

  const edges = g.edges().map((e) => {
    const points = g.edge(e).points as { x: number; y: number }[]
    const d = points
      .map(
        (p, i) => `${i === 0 ? "M" : "L"}${p.x.toFixed(1)} ${p.y.toFixed(1)}`
      )
      .join(" ")
    return { from: e.v, to: e.w, d, order: orderOf.get(e.v) ?? 0 }
  })
  const graph = g.graph()
  return {
    nodes: placed,
    edges,
    width: graph.width ?? NODE_W,
    height: graph.height ?? NODE_H,
  }
}
