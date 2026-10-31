import { HttpResponse, http } from "msw"

import type {
  CreateWorkspaceBody,
  PatchWorkspaceBody,
  WorkspaceDetail,
} from "@/api/pending-types"

import { bus } from "../../mock/bus"
import { listItem, nextId, state } from "../db"
import { apiError, latency, notFound } from "../respond"

const API = "*/api/v1"

function nameTaken(name: string, exceptId?: string): boolean {
  return [...state.db.workspaces.values()].some(
    (w) =>
      !w.archived &&
      w.id !== exceptId &&
      w.name.toLowerCase() === name.trim().toLowerCase()
  )
}

function validateName(name: string | undefined, exceptId?: string) {
  const trimmed = (name ?? "").trim()
  if (trimmed.length < 1 || trimmed.length > 60)
    return apiError("validation", "Invalid workspace", {
      details: {
        fields: [{ path: "name", message: "Use 1 to 60 characters" }],
      },
    })
  if (nameTaken(trimmed, exceptId))
    return apiError("validation", "Invalid workspace", {
      details: {
        fields: [
          {
            path: "name",
            message: "A workspace with this name already exists",
          },
        ],
      },
    })
  return null
}

const slugify = (name: string) =>
  name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "")

export const workspaceHandlers = [
  http.get(`${API}/workspaces`, async ({ request }) => {
    await latency()
    const archived =
      new URL(request.url).searchParams.get("archived") === "true"
    const items = [...state.db.workspaces.values()]
      .filter((w) => w.archived === archived)
      .map(listItem)
    return HttpResponse.json(items)
  }),

  http.post(`${API}/workspaces`, async ({ request }) => {
    await latency()
    const body = (await request.json()) as CreateWorkspaceBody
    const invalid = validateName(body.name)
    if (invalid) return invalid
    const id = nextId("ws")
    const workspace: WorkspaceDetail = {
      id,
      name: body.name.trim(),
      slug: slugify(body.name),
      icon: body.icon,
      hue: body.hue,
      description: body.description ?? "",
      archived: false,
      damaged: false,
      createdAt: state.db.now,
      lastOpenedAt: state.db.now,
      sources: {
        claude: true,
        codex: true,
        agy: true,
        git: true,
        allow: [],
        sinceDays: 30,
        ...body.sources,
      },
      overrides: { profiles: {} },
      stats: {
        nodes: 0,
        chains: 0,
        chats: 0,
        dueCards: 0,
        lastActivityAt: null,
      },
    }
    state.db.workspaces.set(id, workspace)
    state.db.chats.set(id, [])
    state.db.cards.set(id, [])
    bus.emit("workspace.created", { workspaceId: id })
    return HttpResponse.json(workspace)
  }),

  http.post(`${API}/workspaces/import`, async () => {
    await latency()
    return apiError("validation", "Not a LearnPuppy export", {
      details: {
        fields: [{ path: "file", message: "The archive has no manifest" }],
      },
    })
  }),

  http.get(`${API}/workspaces/:ws`, async ({ params }) => {
    await latency()
    const ws = state.db.workspaces.get(String(params["ws"]))
    if (!ws) return notFound("Workspace")
    return HttpResponse.json({ ...ws, ...listItem(ws) })
  }),

  http.patch(`${API}/workspaces/:ws`, async ({ params, request }) => {
    await latency()
    const ws = state.db.workspaces.get(String(params["ws"]))
    if (!ws) return notFound("Workspace")
    if (ws.archived)
      return apiError("workspace_archived", "Archived workspaces are read-only")
    const body = (await request.json()) as PatchWorkspaceBody
    if (body.name !== undefined) {
      const invalid = validateName(body.name, ws.id)
      if (invalid) return invalid
      ws.name = body.name.trim()
      ws.slug = slugify(ws.name)
    }
    if (body.icon !== undefined) ws.icon = body.icon
    if (body.hue !== undefined) ws.hue = body.hue
    if (body.description !== undefined) ws.description = body.description
    if (body.sources) ws.sources = { ...ws.sources, ...body.sources }
    if (body.overrides) ws.overrides = body.overrides
    bus.emit("workspace.updated", { workspaceId: ws.id })
    return HttpResponse.json({ ...ws, ...listItem(ws) })
  }),

  http.post(`${API}/workspaces/:ws/archive`, ({ params }) => {
    const ws = state.db.workspaces.get(String(params["ws"]))
    if (!ws) return notFound("Workspace")
    ws.archived = true
    bus.emit("workspace.updated", { workspaceId: ws.id })
    return HttpResponse.json({ ...ws, ...listItem(ws) })
  }),
  http.post(`${API}/workspaces/:ws/restore`, ({ params }) => {
    const ws = state.db.workspaces.get(String(params["ws"]))
    if (!ws) return notFound("Workspace")
    ws.archived = false
    bus.emit("workspace.updated", { workspaceId: ws.id })
    return HttpResponse.json({ ...ws, ...listItem(ws) })
  }),

  http.delete(`${API}/workspaces/:ws`, async ({ params, request }) => {
    await latency()
    const id = String(params["ws"])
    const ws = state.db.workspaces.get(id)
    if (!ws) return notFound("Workspace")
    const { confirmName } = (await request.json()) as { confirmName: string }
    if (confirmName !== ws.name)
      return apiError("validation", "Confirmation does not match", {
        details: {
          fields: [
            { path: "confirmName", message: "Type the workspace name exactly" },
          ],
        },
      })
    state.db.workspaces.delete(id)
    state.db.trash.push({
      id: `trash_${id}`,
      workspaceId: id,
      name: ws.name,
      deletedAt: state.db.now,
      sizeBytes: 3_400_000,
    })
    bus.emit("workspace.deleted", { workspaceId: id })
    return new HttpResponse(null, { status: 204 })
  }),

  http.post(`${API}/workspaces/:ws/export`, ({ params }) => {
    const id = String(params["ws"])
    if (!state.db.workspaces.has(id)) return notFound("Workspace")
    const jobId = nextId("export")
    setTimeout(
      () => bus.emit("export.ready", { jobId, token: `tok_${jobId}` }, id),
      300
    )
    return HttpResponse.json({ jobId })
  }),

  http.get(`${API}/trash`, () => HttpResponse.json(state.db.trash)),
  http.post(`${API}/trash/:id/restore`, ({ params }) => {
    const item = state.db.trash.find((t) => t.id === params["id"])
    if (!item) return notFound("Trash item")
    return apiError(
      "conflict",
      "Restoring from trash is not available in mock mode"
    )
  }),
  http.delete(`${API}/trash/:id`, ({ params }) => {
    const index = state.db.trash.findIndex((t) => t.id === params["id"])
    if (index < 0) return notFound("Trash item")
    state.db.trash.splice(index, 1)
    return new HttpResponse(null, { status: 204 })
  }),
]
