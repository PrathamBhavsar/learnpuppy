import { HttpResponse, http } from "msw"

import type {
  CheckinRequest,
  CheckinResult,
  OverrideRequest,
  SetProfileRequest,
  SetScheduleRequest,
  StartBoutRequest,
  SuggestSlotsRequest,
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

  http.get(`${API}/schedule`, ({ params }) =>
    HttpResponse.json(entry(params["ws"]).schedule)
  ),

  http.post(`${API}/schedule`, async ({ params, request }) => {
    const e = entry(params["ws"])
    const body = (await request.json()) as SetScheduleRequest
    e.schedule = { wakeTime: body.wakeTime, slots: [...body.slots].sort() }
    return HttpResponse.json(e.schedule)
  }),

  http.post(`${API}/schedule/suggest`, async ({ request }) => {
    const { wakeTime } = (await request.json()) as SuggestSlotsRequest
    const [h = 0, m = 0] = wakeTime.split(":").map(Number)
    const first = h * 60 + m + 180
    const slots = [0, 1, 2].map((i) => {
      const min = (first + i * 210) % (24 * 60)
      return `${String(Math.floor(min / 60)).padStart(2, "0")}:${String(min % 60).padStart(2, "0")}`
    })
    return HttpResponse.json({ slots })
  }),

  http.get(`${API}/schedule/track`, ({ params }) =>
    HttpResponse.json({ slots: entry(params["ws"]).slots })
  ),

  http.get(`${API}/settings`, ({ params }) =>
    HttpResponse.json({ settings: entry(params["ws"]).settings })
  ),

  http.post(`${API}/settings`, async ({ params, request }) => {
    const e = entry(params["ws"])
    const body = (await request.json()) as { name: string; value: number }
    const row = e.settings.find((s) => s.name === body.name)
    if (!row || body.value < row.min || body.value > row.max)
      return apiError("validation", `${body.name} is out of range`)
    row.value = body.value
    return HttpResponse.json({ settings: e.settings })
  }),

  http.get(`${API}/profile`, ({ params }) =>
    HttpResponse.json({ entries: entry(params["ws"]).profile })
  ),

  http.post(`${API}/profile`, async ({ params, request }) => {
    const e = entry(params["ws"])
    const body = (await request.json()) as SetProfileRequest
    e.profile = [
      ...e.profile.filter((p) => p.key !== body.key),
      { ts: Date.now(), key: body.key, value: body.value },
    ]
    return HttpResponse.json({ entries: e.profile })
  }),
]
