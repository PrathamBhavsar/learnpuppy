import { HttpResponse, http } from "msw"

import type { ChainTableRow, DeletePreview } from "@/api/pending-types"

import { bus } from "../../mock/bus"
import { nextId, state } from "../db"
import { apiError, latency, notFound } from "../respond"

const API = "*/api/v1/workspaces/:ws/graph"

const graphOf = (ws: unknown) => state.db.graphs.get(String(ws))

export const graphHandlers = [
  http.get(API, async ({ params }) => {
    await latency()
    const graph = graphOf(params["ws"])
    if (!graph) return notFound("Graph")
    return HttpResponse.json(graph.snapshot)
  }),

  http.get(`${API}/nodes/:id`, async ({ params }) => {
    await latency()
    const detail = graphOf(params["ws"])?.details.get(String(params["id"]))
    return detail ? HttpResponse.json(detail) : notFound("Node")
  }),

  http.get(`${API}/search`, async ({ params, request }) => {
    await latency()
    const graph = graphOf(params["ws"])
    if (!graph) return notFound("Graph")
    const q = (new URL(request.url).searchParams.get("q") ?? "").toLowerCase()
    const hits = graph.snapshot.nodes
      .filter((n) => q && n.text.toLowerCase().includes(q))
      .slice(0, 50)
      .map((n) => ({ id: n.id, text: n.text, kind: n.kind, score: 1 }))
    return HttpResponse.json(hits)
  }),

  http.post(`${API}/relayout`, ({ params }) => {
    const graph = graphOf(params["ws"])
    if (!graph) return notFound("Graph")
    graph.snapshot.layoutVersion += 1
    graph.snapshot.version += 1
    bus.emit(
      "graph.changed",
      { version: graph.snapshot.version },
      String(params["ws"])
    )
    return HttpResponse.json({ layoutVersion: graph.snapshot.layoutVersion })
  }),

  http.post(`${API}/delete-preview`, async ({ params, request }) => {
    await latency()
    const graph = graphOf(params["ws"])
    if (!graph) return notFound("Graph")
    const { nodeIds } = (await request.json()) as { nodeIds: string[] }
    const ids = new Set(nodeIds)
    const orphaned = graph.snapshot.chains.filter(
      (c) => ids.has(c.from) || ids.has(c.to)
    )
    const preview: DeletePreview = {
      deleted: ids.size,
      orphanedChains: orphaned.length,
      blastRadius: orphaned.slice(0, 8).map((c) => ({
        id: c.id,
        text: `${c.kind}: ${c.from} → ${c.to}`,
        reason: "Chain loses an endpoint",
      })),
      previewToken: `prev_${[...ids].sort().join("_")}`,
    }
    return HttpResponse.json(preview)
  }),

  http.delete(`${API}/nodes`, async ({ params, request }) => {
    await latency()
    const wsId = String(params["ws"])
    const graph = graphOf(wsId)
    if (!graph) return notFound("Graph")
    const { nodeIds, previewToken } = (await request.json()) as {
      nodeIds: string[]
      previewToken: string
    }
    if (previewToken !== `prev_${[...nodeIds].sort().join("_")}`)
      return apiError(
        "conflict",
        "The preview is out of date; review the deletion again"
      )
    const ids = new Set(nodeIds)
    graph.snapshot.nodes = graph.snapshot.nodes.filter((n) => !ids.has(n.id))
    graph.snapshot.chains = graph.snapshot.chains.filter(
      (c) => !ids.has(c.from) && !ids.has(c.to)
    )
    for (const cluster of graph.snapshot.clusters)
      cluster.nodeIds = cluster.nodeIds.filter((id) => !ids.has(id))
    graph.snapshot.version += 1
    bus.emit("graph.changed", { version: graph.snapshot.version }, wsId)
    return HttpResponse.json({ deleted: ids.size })
  }),

  http.get(`${API}/chains`, async ({ params, request }) => {
    await latency()
    const graph = graphOf(params["ws"])
    if (!graph) return notFound("Graph")
    const selection = (new URL(request.url).searchParams.get("selection") ?? "")
      .split(",")
      .filter(Boolean)
    const picked = new Set(selection)
    const rows: ChainTableRow[] = graph.snapshot.chains
      .filter(
        (c) => picked.size === 0 || picked.has(c.from) || picked.has(c.to)
      )
      .map((c) => {
        const from = graph.snapshot.nodes.find((n) => n.id === c.from)
        return {
          groupId: from?.sessionId ?? "unknown",
          groupLabel: from?.sessionId ?? "Unknown session",
          chainId: c.id,
          from: c.from,
          to: c.to,
          kind: c.kind,
        }
      })
    return HttpResponse.json(rows)
  }),

  http.post(`${API}/practice`, async ({ params, request }) => {
    const wsId = String(params["ws"])
    const { nodeIds } = (await request.json()) as { nodeIds: string[] }
    if (nodeIds.length === 0)
      return apiError("validation", "Select at least one node", {
        details: {
          fields: [{ path: "nodeIds", message: "Select at least one node" }],
        },
      })
    const chatId = nextId("c_practice")
    const chats = state.db.chats.get(wsId) ?? []
    chats.unshift({
      id: chatId,
      kind: "practice",
      title: `Practice: ${nodeIds.length} selected`,
      phase: "practice",
      viewVersion: 1,
      messages: [],
      pending: null,
      plan: null,
      probeProgress: { asked: 0, max: 8 },
      verdicts: [],
      jobs: [],
      turn: {
        state: "running",
        turnId: nextId("turn"),
        startedAt: state.db.now,
      },
    })
    state.db.chats.set(wsId, chats)
    bus.emit("chat.created", { chatId }, wsId)
    return HttpResponse.json({ chatId })
  }),
]
