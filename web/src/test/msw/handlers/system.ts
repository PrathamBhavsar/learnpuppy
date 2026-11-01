import { HttpResponse, http } from "msw"

import type { AccountDetail, SearchHit } from "@/api/pending-types"
import type { components } from "@/api/schema"

import { state } from "../db"
import { latency } from "../respond"

type Bootstrap = components["schemas"]["Bootstrap"]

const API = "*/api/v1"

function summary(a: AccountDetail) {
  return {
    id: a.id,
    provider: a.provider,
    email: a.email,
    tier: a.tier,
    status: a.status,
    cooldownUntil: a.cooldownUntil,
    cooldownReason: a.cooldownReason,
  }
}

export const systemHandlers = [
  http.get(`${API}/health`, () =>
    HttpResponse.json({
      ok: true,
      version: "mock",
      engine: "faux",
      dataDir: "/mock/.learnpuppy",
    })
  ),

  http.get(`${API}/bootstrap`, async () => {
    await latency()
    const db = state.db
    const body: Bootstrap = {
      version: "mock",
      serverTime: db.now,
      deviceId: "dev_mock",
      engine: "faux",
      dev: true,
      workspaces: [...db.workspaces.values()].map((w) => ({
        id: w.id,
        name: w.name,
        slug: w.slug,
        icon: w.icon,
        hue: w.hue,
        archived: w.archived,
        lastOpenedAt: w.lastOpenedAt,
      })),
      accounts: db.accounts.map(summary),
      banners: [],
      lastWorkspaceId: [...db.workspaces.values()][0]?.id ?? null,
    }
    return HttpResponse.json(body)
  }),

  http.get(`${API}/search`, async ({ request }) => {
    await latency()
    const url = new URL(request.url)
    const q = (url.searchParams.get("q") ?? "").toLowerCase()
    const limit = Number(url.searchParams.get("limit") ?? 30)
    const hits: SearchHit[] = []
    for (const [workspaceId, graph] of state.db.graphs) {
      const ws = state.db.workspaces.get(workspaceId)
      if (!ws || ws.archived) continue
      for (const node of graph.snapshot.nodes) {
        const at = node.text.toLowerCase().indexOf(q)
        if (q && at >= 0) {
          hits.push({
            workspaceId,
            node: {
              id: node.id,
              name: node.text.slice(0, 60),
              text: node.text,
              kind: node.kind,
            },
            score: 1 / (1 + at),
            workspaceName: ws.name,
          })
        }
      }
    }
    hits.sort((a, b) => b.score - a.score)
    return HttpResponse.json({
      items: hits.slice(0, limit),
      timedOut: [],
    } satisfies components["schemas"]["SearchDto"])
  }),
]
