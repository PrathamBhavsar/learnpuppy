import { HttpResponse, http } from "msw"

import type { components } from "@/api/schema"

import type { MockWorkspace } from "../fixtures/workspaces"
import { bus } from "../../mock/bus"
import { listItem, nextId, state } from "../db"
import { apiError, latency, notFound } from "../respond"

const API = "*/api/v1"

type Schemas = components["schemas"]
type WorkspaceDto = Schemas["WorkspaceDto"]

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
    const all = [...state.db.workspaces.values()].filter(
      (w) => w.archived === archived
    )
    const body: Schemas["WorkspaceListDto"] = {
      items: all.filter((w) => !w.damaged).map(listItem),
      damaged: all
        .filter((w) => w.damaged)
        .map((w) => ({
          id: w.id,
          path: `/mock/.learnpuppy/workspaces/${w.id}`,
          reason: "workspace.json is unreadable",
        })),
    }
    return HttpResponse.json(body)
  }),

  http.post(`${API}/workspaces`, async ({ request }) => {
    await latency()
    const body = (await request.json()) as Schemas["CreateWorkspace"]
    const invalid = validateName(body.name)
    if (invalid) return invalid
    const id = nextId("ws")
    const workspace: MockWorkspace = {
      id,
      name: body.name.trim(),
      slug: slugify(body.name),
      icon: body.icon ?? "book-open",
      hue: body.hue ?? "violet",
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
        ...(body.sources ?? {}),
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
    const created: WorkspaceDto = listItem(workspace)
    return HttpResponse.json(created, { status: 201 })
  }),

  http.post(`${API}/workspaces/:ws/repair`, async ({ params }) => {
    await latency()
    const ws = state.db.workspaces.get(String(params["ws"]))
    if (!ws) return notFound("Workspace")
    // The server rewrites minimal metadata from the folder name; mock the same thing on the id.
    ws.name = String(params["ws"])
      .replace(/^ws_/, "")
      .replace(/[-_]+/g, " ")
      .trim()
      .replace(/\b\w/g, (c) => c.toUpperCase())
    ws.slug = slugify(ws.name)
    ws.damaged = false
    bus.emit("workspace.updated", { workspaceId: ws.id })
    return HttpResponse.json(listItem(ws) satisfies WorkspaceDto)
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
    return HttpResponse.json(listItem(ws) satisfies WorkspaceDto)
  }),

  http.patch(`${API}/workspaces/:ws`, async ({ params, request }) => {
    await latency()
    const ws = state.db.workspaces.get(String(params["ws"]))
    if (!ws) return notFound("Workspace")
    if (ws.archived)
      return apiError("workspace_archived", "Archived workspaces are read-only")
    const body = (await request.json()) as Schemas["PatchWorkspace"]
    if (body.name != null) {
      const invalid = validateName(body.name, ws.id)
      if (invalid) return invalid
      ws.name = body.name.trim()
      ws.slug = slugify(ws.name)
    }
    if (body.icon != null) ws.icon = body.icon
    if (body.hue != null) ws.hue = body.hue
    if (body.description != null) ws.description = body.description
    if (body.sources) ws.sources = { ...ws.sources, ...body.sources }
    if (body.overrides) ws.overrides = body.overrides
    bus.emit("workspace.updated", { workspaceId: ws.id })
    return HttpResponse.json(listItem(ws) satisfies WorkspaceDto)
  }),

  http.post(`${API}/workspaces/:ws/archive`, ({ params }) => {
    const ws = state.db.workspaces.get(String(params["ws"]))
    if (!ws) return notFound("Workspace")
    ws.archived = true
    bus.emit("workspace.updated", { workspaceId: ws.id })
    return HttpResponse.json(listItem(ws) satisfies WorkspaceDto)
  }),
  http.post(`${API}/workspaces/:ws/restore`, ({ params }) => {
    const ws = state.db.workspaces.get(String(params["ws"]))
    if (!ws) return notFound("Workspace")
    ws.archived = false
    bus.emit("workspace.updated", { workspaceId: ws.id })
    return HttpResponse.json(listItem(ws) satisfies WorkspaceDto)
  }),

  http.delete(`${API}/workspaces/:ws`, async ({ params, request }) => {
    await latency()
    const id = String(params["ws"])
    const ws = state.db.workspaces.get(id)
    if (!ws) return notFound("Workspace")
    const { confirmName } = (await request.json()) as { confirmName: string }
    if (confirmName !== (ws.damaged ? ws.id : ws.name))
      return apiError("validation", "Confirmation does not match", {
        details: {
          fields: [
            { path: "confirmName", message: "Type the workspace name exactly" },
          ],
        },
      })
    state.db.workspaces.delete(id)
    const entry: Schemas["TrashEntryDto"] = {
      id: `trash_${id}`,
      workspaceId: id,
      name: ws.name,
      deletedAt: state.db.now,
      bytes: 3_400_000,
    }
    state.db.trash.push(entry)
    bus.emit("workspace.deleted", { workspaceId: id })
    return HttpResponse.json(entry)
  }),

  http.post(`${API}/workspaces/:ws/export`, ({ params }) => {
    const id = String(params["ws"])
    if (!state.db.workspaces.has(id)) return notFound("Workspace")
    const jobId = nextId("export")
    setTimeout(
      () => bus.emit("export.ready", { jobId, token: `tok_${jobId}` }, id),
      300
    )
    return HttpResponse.json({ jobId } satisfies Schemas["ExportStarted"], {
      status: 202,
    })
  }),

  http.get(`${API}/trash`, () =>
    HttpResponse.json({
      items: state.db.trash,
    } satisfies Schemas["TrashListDto"])
  ),
  http.post(`${API}/trash/:id/restore`, async ({ params }) => {
    await latency()
    const index = state.db.trash.findIndex((t) => t.id === params["id"])
    const item = state.db.trash[index]
    if (!item) return notFound("Trash item")
    if (state.db.workspaces.has(item.workspaceId))
      return apiError("conflict", "A workspace with this id already exists")
    const ws: MockWorkspace = {
      id: item.workspaceId,
      name: item.name,
      slug: slugify(item.name),
      icon: "book-open",
      hue: "violet",
      description: "",
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
    state.db.workspaces.set(ws.id, ws)
    state.db.chats.set(ws.id, [])
    state.db.cards.set(ws.id, [])
    state.db.trash.splice(index, 1)
    bus.emit("workspace.updated", { workspaceId: ws.id })
    return HttpResponse.json(listItem(ws) satisfies WorkspaceDto)
  }),
  http.delete(`${API}/trash/:id`, ({ params }) => {
    const index = state.db.trash.findIndex((t) => t.id === params["id"])
    if (index < 0) return notFound("Trash item")
    state.db.trash.splice(index, 1)
    return new HttpResponse(null, { status: 204 })
  }),
]
