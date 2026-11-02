import { HttpResponse, http } from "msw"

import type { AccountDetail } from "@/api/pending-types"

import { bus } from "../../mock/bus"
import { state } from "../db"
import { reauthHintFor } from "../fixtures/accounts"
import { apiError, latency, notFound } from "../respond"

const API = "*/api/v1/accounts"

export const accountHandlers = [
  http.get(API, async () => {
    await latency()
    return HttpResponse.json({ items: state.db.accounts })
  }),

  http.post(`${API}/rescan`, async () => {
    await latency()
    return HttpResponse.json({ items: state.db.accounts })
  }),

  http.post(`${API}/claude`, async ({ request }) => {
    await latency()
    const { path } = (await request.json()) as { path: string }
    const fail = (message: string) =>
      apiError("validation", "That directory is not a Claude profile", {
        details: { fields: [{ path: "path", message }] },
      })
    if (!path.startsWith("/")) return fail("Use an absolute directory path")
    if (path.endsWith("/nope"))
      return fail("No .claude.json found in that directory")
    if (state.db.accounts.some((a) => a.id === `claude:${path}`))
      return apiError("conflict", "That profile is already in the pool")
    const account: AccountDetail = {
      id: `claude:${path}`,
      provider: "claude",
      email: "added@example.com",
      tier: "pro",
      status: "ready",
      cooldownUntil: null,
      cooldownReason: null,
      requests: 0,
      tokensIn: 0,
      tokensOut: 0,
      reauthHint: null,
      howToConnect: "claude login",
      models: [],
    }
    state.db.accounts.push(account)
    bus.emit("accounts.changed", {})
    return HttpResponse.json(account)
  }),

  http.patch(`${API}/:id`, async ({ params, request }) => {
    await latency()
    const { disabled } = (await request.json()) as { disabled: boolean }
    const account = state.db.accounts.find((a) => a.id === params["id"])
    if (!account) return notFound("Account")
    account.status = disabled ? "disabled" : "ready"
    account.cooldownUntil = null
    account.cooldownReason = null
    account.reauthHint = reauthHintFor(account.provider, account.id)
    bus.emit("accounts.changed", {})
    return HttpResponse.json(account)
  }),
]
