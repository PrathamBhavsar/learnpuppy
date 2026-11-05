import { HttpResponse, http } from "msw"

import type {
  CheckinRequest,
  CheckinResult,
  OverrideRequest,
  StartBoutRequest,
} from "@/api/learning-types"

import { state } from "../db"
import { learningFixture } from "../fixtures/learning"
import { apiError } from "../respond"

const API = "*/api/v1/workspaces/:ws/learning"

function entry(ws: unknown) {
  const key = String(ws)
  if (!state.db.learning.has(key)) state.db.learning.set(key, learningFixture())
  return state.db.learning.get(key)!
}

export const learningHandlers = [
  http.get(`${API}/clock`, ({ params }) =>
    HttpResponse.json(entry(params["ws"]).clock)
  ),

  http.post(`${API}/bout/start`, async ({ params, request }) => {
    const e = entry(params["ws"])
    const body = (await request.json()) as StartBoutRequest
    if (
      !body.ready.phoneAway ||
      !body.ready.tabsClosed ||
      body.ready.focusSeconds < 30
    ) {
      return apiError("conflict", "Not ready yet", {
        details: { gate: "not_ready" },
      })
    }
    if (e.clock.status.status === "gap") {
      return apiError("conflict", "Still inside the gap", {
        details: { gate: "gap" },
      })
    }
    if (e.clock.status.status === "daily_limit_reached") {
      return apiError("conflict", "Today's minutes are spent", {
        details: { gate: "daily_limit" },
      })
    }
    e.clock = {
      ...e.clock,
      bout: "b_mock",
      status: { status: "active", activeMinutes: 0 },
    }
    return HttpResponse.json(e.clock)
  }),

  http.post(`${API}/bout/heartbeat`, ({ params }) => {
    const e = entry(params["ws"])
    if (e.clock.status.status === "active") {
      const next = e.clock.status.activeMinutes + 1
      e.clock = {
        ...e.clock,
        status:
          next >= e.clock.boutLimitMinutes
            ? { status: "bout_limit_reached" }
            : { status: "active", activeMinutes: next },
        minutesToday: e.clock.minutesToday + 1,
      }
    }
    return HttpResponse.json(e.clock)
  }),

  http.post(`${API}/bout/end`, ({ params }) => {
    const e = entry(params["ws"])
    e.clock = {
      ...e.clock,
      bout: null,
      status: e.clock.overrideUsedToday
        ? { status: "active", activeMinutes: 0 }
        : { status: "gap", remainingMinutes: 120 },
    }
    return HttpResponse.json(e.clock)
  }),

  http.post(
    `${API}/bout/focus-lost`,
    () => new HttpResponse(null, { status: 204 })
  ),

  http.post(`${API}/bout/override`, async ({ params, request }) => {
    const e = entry(params["ws"])
    ;(await request.json()) as OverrideRequest
    e.clock = {
      ...e.clock,
      overrideUsedToday: true,
      status: { status: "active", activeMinutes: 0 },
    }
    return new HttpResponse(null, { status: 204 })
  }),

  http.get(`${API}/checkin/needed`, ({ params }) =>
    HttpResponse.json({ needed: entry(params["ws"]).checkinNeeded })
  ),

  http.post(`${API}/checkin`, async ({ params, request }) => {
    const e = entry(params["ws"])
    const body = (await request.json()) as CheckinRequest
    e.checkinNeeded = false
    const result: CheckinResult = { reviewOnlySuggested: body.sleepHours < 6 }
    return HttpResponse.json(result)
  }),
]
