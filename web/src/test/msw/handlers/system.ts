import { HttpResponse, http } from "msw"

import type { AccountDetail, SearchHit, SettingsDoc } from "@/api/pending-types"
import type { components } from "@/api/schema"

import { bus } from "../../mock/bus"
import { state } from "../db"
import { CATALOG, ROLES } from "../fixtures/system"
import { apiError, latency, notFound } from "../respond"

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
            node: { id: node.id, text: node.text, kind: node.kind },
            score: 1 / (1 + at),
          })
        }
      }
    }
    hits.sort((a, b) => b.score - a.score)
    return HttpResponse.json(hits.slice(0, limit))
  }),

  http.get(`${API}/accounts`, async () => {
    await latency()
    return HttpResponse.json(state.db.accounts)
  }),
  http.post(`${API}/accounts/rescan`, async () => {
    await latency()
    return HttpResponse.json(state.db.accounts)
  }),
  http.post(`${API}/accounts/claude`, async ({ request }) => {
    const { path } = (await request.json()) as { path: string }
    if (!path.startsWith("/")) {
      return apiError("validation", "Path must be absolute", {
        details: {
          fields: [{ path: "path", message: "Use an absolute directory path" }],
        },
      })
    }
    const account: AccountDetail = {
      id: `claude:added-${state.db.accounts.length + 1}`,
      provider: "claude",
      email: "added@example.com",
      tier: "pro",
      status: "ready",
      cooldownUntil: null,
      cooldownReason: null,
      requests: 0,
      tokensIn: 0,
      tokensOut: 0,
    }
    state.db.accounts.push(account)
    bus.emit("accounts.changed", {})
    return HttpResponse.json(account, { status: 200 })
  }),
  http.patch(`${API}/accounts/:id`, async ({ params, request }) => {
    const { disabled } = (await request.json()) as { disabled: boolean }
    const account = state.db.accounts.find((a) => a.id === params["id"])
    if (!account) return notFound("Account")
    account.status = disabled ? "disabled" : "ready"
    bus.emit("accounts.changed", {})
    return HttpResponse.json(account)
  }),

  http.get(`${API}/settings`, () => HttpResponse.json(state.db.settings)),
  http.put(`${API}/settings`, async ({ request }) => {
    const doc = (await request.json()) as SettingsDoc
    const fields: { path: string; message: string }[] = []
    for (const role of ROLES) {
      const profile = doc.profiles[role]
      const provider = CATALOG.providers.find((p) => p.id === profile?.provider)
      if (!provider || !profile)
        fields.push({
          path: `profiles.${role}.provider`,
          message: "Unknown provider",
        })
      else if (!provider.models.includes(profile.model))
        fields.push({
          path: `profiles.${role}.model`,
          message: "Unknown model",
        })
    }
    if (fields.length)
      return apiError("validation", "Invalid settings", { details: { fields } })
    state.db.settings = doc
    return HttpResponse.json(doc)
  }),
  http.get(`${API}/settings/catalog`, () => HttpResponse.json(CATALOG)),

  http.post(`${API}/diagnostics`, () =>
    HttpResponse.json({
      text: "LearnPuppy diagnostics\nversion: mock\nengine: faux\naccounts: 4 (2 ready)",
    })
  ),
]
