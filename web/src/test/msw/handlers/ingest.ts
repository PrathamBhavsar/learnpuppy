import { HttpResponse, http } from "msw"

import type { IngestRun, IngestSourcesBody } from "@/api/pending-types"

import { bus } from "../../mock/bus"
import { nextId, state } from "../db"
import { apiError, latency, notFound } from "../respond"

const API = "*/api/v1/workspaces/:ws/ingest"
const PHASES = ["scanning", "extracting", "auditing", "cards"] as const

/** Pending timers per run so a cancel stops the scripted progress. */
const timers = new Map<string, ReturnType<typeof setTimeout>[]>()

function warningsFor(ws: string, allow: string[]) {
  const out: { path: string; otherPath: string; workspaceId: string }[] = []
  for (const [other, entry] of state.db.ingest) {
    if (other === ws) continue
    for (const path of allow)
      for (const otherPath of entry.sources.sources.allow)
        if (
          path === otherPath ||
          path.startsWith(`${otherPath}/`) ||
          otherPath.startsWith(`${path}/`)
        )
          out.push({ path, otherPath, workspaceId: other })
  }
  return out
}

function finish(ws: string, run: IngestRun, errors: string[]) {
  const entry = state.db.ingest.get(ws)
  run.finishedAt = Date.now()
  run.report = {
    sessions: errors.length ? 5 : 12,
    newDecisions: errors.length ? 1 : 4,
    newCards: errors.length ? 3 : 11,
    skipped: 0,
    errors,
  }
  if (entry) entry.sources.runningId = null
  timers.delete(run.id)
  bus.emit("ingest.done", { runId: run.id, report: run.report }, ws)
}

export const ingestHandlers = [
  http.get(`${API}/sources`, async ({ params }) => {
    await latency()
    const entry = state.db.ingest.get(String(params["ws"]))
    return entry ? HttpResponse.json(entry.sources) : notFound("Workspace")
  }),

  http.put(`${API}/sources`, async ({ params, request }) => {
    await latency()
    const ws = String(params["ws"])
    const entry = state.db.ingest.get(ws)
    if (!entry) return notFound("Workspace")
    const body = (await request.json()) as IngestSourcesBody
    const bad = body.allow
      .map((path, index) => ({ path, index }))
      .filter(
        ({ path }) => !path.startsWith("/") || path.includes("/unreadable")
      )
    if (bad.length)
      return apiError("validation", "Some directories cannot be used", {
        details: {
          fields: bad.map(({ path, index }) => ({
            path: `allow[${index}]`,
            message: path.startsWith("/")
              ? "Directory is not readable"
              : "Path must be absolute",
          })),
        },
      })
    entry.sources = {
      ...entry.sources,
      sources: body,
      enabled: body.allow.length > 0,
      warnings: warningsFor(ws, body.allow),
    }
    return HttpResponse.json(entry.sources)
  }),

  http.get(`${API}/runs`, async ({ params }) => {
    await latency()
    const entry = state.db.ingest.get(String(params["ws"]))
    return entry
      ? HttpResponse.json({ items: entry.runs })
      : notFound("Workspace")
  }),

  http.post(`${API}/run`, async ({ params }) => {
    await latency()
    const ws = String(params["ws"])
    const entry = state.db.ingest.get(ws)
    if (!entry) return notFound("Workspace")
    if (entry.sources.sources.allow.length === 0)
      return apiError(
        "validation",
        "Add a project directory before ingesting",
        {
          details: {
            fields: [{ path: "allow", message: "The allow-list is empty" }],
          },
        }
      )
    if (entry.sources.runningId)
      return apiError("conflict", "An ingest run is already in progress")
    const runId = nextId("run")
    const run: IngestRun = {
      id: runId,
      startedAt: Date.now(),
      finishedAt: null,
      report: {
        sessions: 0,
        newDecisions: 0,
        newCards: 0,
        skipped: 0,
        errors: [],
      },
    }
    entry.runs.unshift(run)
    entry.sources.runningId = runId
    const pending = PHASES.map((phase, i) =>
      setTimeout(
        () =>
          bus.emit(
            "ingest.progress",
            { runId, done: (i + 1) * 3, total: 12, phase },
            ws
          ),
        250 * (i + 1)
      )
    )
    pending.push(
      setTimeout(
        () =>
          finish(
            ws,
            run,
            entry.sources.sources.allow.some((p) => p.includes("flaky"))
              ? [
                  "claude: /Users/dev/code/flaky is not readable (permission denied)",
                ]
              : []
          ),
        1300
      )
    )
    timers.set(runId, pending)
    return HttpResponse.json({ runId }, { status: 202 })
  }),

  http.post(`${API}/runs/:id/cancel`, async ({ params }) => {
    await latency()
    const ws = String(params["ws"])
    const runId = String(params["id"])
    const run = state.db.ingest.get(ws)?.runs.find((r) => r.id === runId)
    if (!run || run.finishedAt) return notFound("Running ingest")
    for (const timer of timers.get(runId) ?? []) clearTimeout(timer)
    finish(ws, run, ["Cancelled before the run finished"])
    return new HttpResponse(null, { status: 204 })
  }),
]
