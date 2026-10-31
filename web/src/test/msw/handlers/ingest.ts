import { HttpResponse, http } from "msw"

import type { IngestRun, IngestSources } from "@/api/pending-types"

import { bus } from "../../mock/bus"
import { nextId, state } from "../db"
import { apiError, latency, notFound } from "../respond"

const API = "*/api/v1/workspaces/:ws/ingest"

export const ingestHandlers = [
  http.get(`${API}/sources`, async ({ params }) => {
    await latency()
    const entry = state.db.ingest.get(String(params["ws"]))
    return entry ? HttpResponse.json(entry.sources) : notFound("Workspace")
  }),

  http.put(`${API}/sources`, async ({ params, request }) => {
    await latency()
    const entry = state.db.ingest.get(String(params["ws"]))
    if (!entry) return notFound("Workspace")
    const body = (await request.json()) as Omit<
      IngestSources,
      "candidates" | "overlaps"
    >
    const bad = body.allow.filter((p) => !p.startsWith("/"))
    if (bad.length)
      return apiError("validation", "Invalid sources", {
        details: {
          fields: bad.map((p) => ({
            path: `allow[${body.allow.indexOf(p)}]`,
            message: "Path must be absolute",
          })),
        },
      })
    const overlaps = body.allow
      .filter(
        (p) => p.includes("learnpuppy") && params["ws"] !== "ws_rustasync"
      )
      .map((path) => ({
        path,
        workspaceId: "ws_rustasync",
        workspaceName: "Rust async",
      }))
    entry.sources = { ...entry.sources, ...body, overlaps }
    return HttpResponse.json(entry.sources)
  }),

  http.get(`${API}/runs`, async ({ params }) => {
    await latency()
    const entry = state.db.ingest.get(String(params["ws"]))
    return entry ? HttpResponse.json(entry.runs) : notFound("Workspace")
  }),

  http.post(`${API}/run`, async ({ params }) => {
    await latency()
    const wsId = String(params["ws"])
    const entry = state.db.ingest.get(wsId)
    if (!entry) return notFound("Workspace")
    if (entry.sources.allow.length === 0)
      return apiError(
        "validation",
        "Add at least one source directory before ingesting",
        {
          details: {
            fields: [{ path: "allow", message: "The allow-list is empty" }],
          },
        }
      )
    if (entry.runs.some((r) => r.finishedAt === null))
      return apiError("conflict", "An ingest run is already in progress")
    const runId = nextId("run")
    const run: IngestRun = {
      id: runId,
      startedAt: Date.now(),
      finishedAt: null,
      report: null,
    }
    entry.runs.unshift(run)
    const phases = ["scanning", "extracting", "auditing", "cards"]
    phases.forEach((phase, i) => {
      setTimeout(
        () =>
          bus.emit(
            "ingest.progress",
            { runId, done: (i + 1) * 3, total: 12, phase },
            wsId
          ),
        250 * (i + 1)
      )
    })
    setTimeout(() => {
      const report = {
        sessions: 12,
        newDecisions: 4,
        newCards: 11,
        skipped: 0,
        errors: [] as string[],
      }
      run.finishedAt = Date.now()
      run.report = report
      bus.emit("ingest.done", { runId, report }, wsId)
    }, 1300)
    return HttpResponse.json({ runId })
  }),
]
