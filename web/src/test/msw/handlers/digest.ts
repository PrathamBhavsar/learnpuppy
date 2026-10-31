import { HttpResponse, http } from "msw"

import { state } from "../db"
import { apiError, latency, notFound } from "../respond"

const API = "*/api/v1/workspaces/:ws"

export const digestHandlers = [
  http.get(`${API}/digest`, async ({ params, request }) => {
    await latency()
    if (!state.db.workspaces.has(String(params["ws"])))
      return notFound("Workspace")
    const url = new URL(request.url)
    const from = url.searchParams.get("from")
    const to = url.searchParams.get("to")
    const source = url.searchParams.get("source")
    const days = state.db.digest.days
      .filter((d) => (!from || d.day >= from) && (!to || d.day <= to))
      .map((d) => ({
        ...d,
        sessions: d.sessions.filter(
          (s) => !source || source.split(",").includes(s.source)
        ),
      }))
      .filter((d) => d.sessions.length > 0)
    return HttpResponse.json(days)
  }),

  http.get(`${API}/digest/decisions/:id`, async ({ params }) => {
    await latency()
    const detail = state.db.digest.details.get(String(params["id"]))
    return detail ? HttpResponse.json(detail) : notFound("Decision")
  }),

  http.post(`${API}/digest/decisions/:id/cards`, async ({ params }) => {
    await latency()
    const detail = state.db.digest.details.get(String(params["id"]))
    if (!detail) return notFound("Decision")
    if (detail.alternatives.length === 0)
      return apiError("validation", "Card shape is invalid", {
        details: {
          fields: [
            {
              path: "alternatives",
              message: "A card needs at least three distractors",
            },
          ],
        },
      })
    return HttpResponse.json({ cardIds: detail.cardIds })
  }),
]
