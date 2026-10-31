import { HttpResponse, http } from "msw"

import type {
  ProfilePingBody,
  ProfilePingResult,
  SettingsDoc,
} from "@/api/pending-types"

import { state } from "../db"
import { CATALOG } from "../fixtures/settings"
import { apiError, latency } from "../respond"

const API = "*/api/v1"

/** Mirrors `PUT /settings` validation: every profile must name a catalog provider and model. */
function validate(doc: SettingsDoc): { path: string; message: string }[] {
  const fields: { path: string; message: string }[] = []
  for (const [role, profile] of Object.entries(doc.profiles)) {
    const provider = CATALOG.providers.find((p) => p.id === profile.provider)
    if (!provider)
      fields.push({
        path: `profiles.${role}.provider`,
        message: "Unknown provider",
      })
    else if (!provider.models.includes(profile.model))
      fields.push({
        path: `profiles.${role}.model`,
        message: `${profile.model} is not offered by ${provider.id}`,
      })
    if (profile.effort && !CATALOG.efforts.includes(profile.effort))
      fields.push({
        path: `profiles.${role}.effort`,
        message: "Unknown effort level",
      })
  }
  if (doc.concurrencyCap < 1 || doc.concurrencyCap > 6)
    fields.push({
      path: "concurrencyCap",
      message: "Must be between 1 and 6",
    })
  return fields
}

export const settingsHandlers = [
  http.get(`${API}/settings`, async () => {
    await latency()
    return HttpResponse.json(state.db.settings)
  }),

  http.put(`${API}/settings`, async ({ request }) => {
    await latency()
    const doc = (await request.json()) as SettingsDoc
    const fields = validate(doc)
    if (fields.length)
      return apiError("validation", "Invalid settings", { details: { fields } })
    state.db.settings = doc
    return HttpResponse.json(doc)
  }),

  http.get(`${API}/settings/catalog`, () => HttpResponse.json(CATALOG)),

  http.post(`${API}/settings/test`, async ({ request }) => {
    await latency()
    const body = (await request.json()) as ProfilePingBody
    const ready = state.db.accounts.some(
      (a) => a.provider === body.provider && a.status === "ready"
    )
    const known = CATALOG.providers
      .find((p) => p.id === body.provider)
      ?.models.includes(body.model)
    const result: ProfilePingResult = !known
      ? { ok: false, latencyMs: 0, message: `${body.model} is not available` }
      : !ready
        ? {
            ok: false,
            latencyMs: 0,
            message: `No ${body.provider} account is ready`,
          }
        : { ok: true, latencyMs: 412, message: "pong" }
    return HttpResponse.json(result)
  }),

  http.post(`${API}/diagnostics`, () =>
    HttpResponse.json({
      text: [
        "LearnPuppy diagnostics",
        "version: mock",
        "engine: faux",
        `accounts: ${state.db.accounts.length} (${state.db.accounts.filter((a) => a.status === "ready").length} ready)`,
      ].join("\n"),
    })
  ),
]
