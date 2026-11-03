import { act, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, delay, http } from "msw"
import { beforeEach, describe, expect, it } from "vitest"

import { FakeEventSource } from "@/test/fake-event-source"
import { listItem, state } from "@/test/msw/db"
import type { MockWorkspace } from "@/test/msw/fixtures/workspaces"
import {
  WS_DAMAGED,
  damagedWorkspace,
  trashFixtures,
} from "@/test/msw/fixtures/workspaces-extra"
import { apiError } from "@/test/msw/respond"
import { server } from "@/test/msw/server"
import { renderApp, resetUi } from "@/test/render"

import { validateImportFile } from "./ImportDialog"
import {
  isNameTaken,
  lastSection,
  relativeTime,
  rememberSection,
} from "./model"

beforeEach(resetUi)

const RUST = "ws_rustasync"
const API = "*/api/v1"

async function openHome(path = "/") {
  const view = renderApp(path)
  await screen.findByTestId("home")
  return view
}

async function openMenu(user: ReturnType<typeof userEvent.setup>, id: string) {
  await user.click(await screen.findByTestId(`workspace-menu-${id}`))
}

describe("home: five states", () => {
  it("loading: a skeleton sized like the card grid", async () => {
    server.use(
      http.get(`${API}/workspaces`, async () => {
        await delay(150)
        return HttpResponse.json({ items: [], damaged: [] })
      })
    )
    await openHome()
    expect(screen.getByTestId("home-loading")).toBeInTheDocument()
    expect(screen.getByTestId("home")).toHaveAttribute("aria-busy", "true")
    await waitFor(() =>
      expect(screen.queryByTestId("home-loading")).not.toBeInTheDocument()
    )
  })

  it("populated: cards show stats, due badge, activity and the due-all link", async () => {
    await openHome()
    const card = await screen.findByTestId(`workspace-item-${RUST}`)
    expect(within(card).getByText("Rust async")).toBeInTheDocument()
    expect(within(card).getByText(/Futures, executors/)).toBeInTheDocument()
    expect(card).toHaveTextContent("150 nodes")
    expect(card).toHaveTextContent("201 chains")
    expect(card).toHaveTextContent("11 chats")
    expect(within(card).getByTestId(`workspace-due-${RUST}`)).toHaveTextContent(
      "7 due"
    )
    expect(
      within(card).getByTestId(`workspace-activity-${RUST}`)
    ).toHaveTextContent(/hours? ago/)
    expect(screen.getByTestId(`workspace-card-${RUST}`)).toHaveAttribute(
      "href",
      `/w/${RUST}/graph`
    )
    expect(screen.getByTestId("due-all-link")).toHaveTextContent(
      "Due today: 19 across 2"
    )
    expect(screen.getByTestId("due-all-link")).toHaveAttribute(
      "href",
      "/review-all"
    )
  })

  it("opens a workspace at the last section visited there", async () => {
    rememberSection(RUST, "review")
    await openHome()
    expect(await screen.findByTestId(`workspace-card-${RUST}`)).toHaveAttribute(
      "href",
      `/w/${RUST}/review`
    )
  })

  it("empty: first run shows onboarding with detected accounts", async () => {
    state.db.workspaces.clear()
    await openHome()
    expect(await screen.findByTestId("onboarding")).toBeInTheDocument()
    expect(
      (
        await within(
          await screen.findByTestId("onboarding-accounts")
        ).findAllByText(/dev@example.com/)
      )[0]
    ).toBeInTheDocument()
    expect(screen.getByTestId("onboarding-rescan")).toBeEnabled()
    expect(screen.queryByTestId("new-workspace")).not.toBeInTheDocument()
  })

  it("error: message, request id and a working retry", async () => {
    let fail = true
    server.use(
      http.get(`${API}/workspaces`, ({ request }) => {
        if (
          fail &&
          new URL(request.url).searchParams.get("archived") !== "true"
        )
          return apiError("internal", "Disk exploded")
        return HttpResponse.json({ items: [], damaged: [] })
      })
    )
    await openHome()
    const error = await screen.findByTestId("error-state", undefined, {
      timeout: 4000,
    })
    expect(error).toHaveTextContent("Disk exploded")
    expect(error).toHaveTextContent(/req_mock_/)
    fail = false
    await userEvent.setup().click(screen.getByTestId("error-retry"))
    await waitFor(() =>
      expect(screen.queryByTestId("error-state")).not.toBeInTheDocument()
    )
  })

  it("degraded: damaged workspace offers Repair and Reveal, and no account raises a banner", async () => {
    const damaged = damagedWorkspace(state.db.now)
    state.db.workspaces.set(damaged.id, damaged)
    state.db.accounts = []
    let repaired = false
    server.use(
      http.post(`${API}/workspaces/${WS_DAMAGED}/repair`, () => {
        repaired = true
        const ws: MockWorkspace = {
          ...damaged,
          damaged: false,
          name: "Recovered",
        }
        state.db.workspaces.set(ws.id, ws)
        return HttpResponse.json(listItem(ws))
      })
    )
    const user = userEvent.setup()
    await openHome()
    const alert = await screen.findByTestId("home-degraded")
    expect(alert).toHaveTextContent("1 workspace is damaged")
    expect(alert).toHaveTextContent("No model account is ready")
    const card = await screen.findByTestId(`workspace-item-${WS_DAMAGED}`)
    expect(card).toHaveAttribute("data-state", "damaged")
    expect(
      within(card).getByTestId(`workspace-card-reveal-${WS_DAMAGED}`)
    ).toBeInTheDocument()
    await user.click(
      within(card).getByTestId(`workspace-card-repair-${WS_DAMAGED}`)
    )
    await waitFor(() => expect(repaired).toBe(true))
    expect(await screen.findByText("Recovered")).toBeInTheDocument()
  })
})

describe("create and edit", () => {
  it("validates the name live: required, unique case-insensitively, then enables submit", async () => {
    const user = userEvent.setup()
    await openHome()
    await user.click(await screen.findByTestId("new-workspace"))
    const input = await screen.findByTestId("ws-name-input")
    const submit = screen.getByTestId("ws-submit")
    await user.type(input, "x")
    await user.clear(input)
    expect(await screen.findByTestId("ws-name-error")).toHaveTextContent(
      "Give the workspace a name"
    )
    expect(submit).toBeDisabled()
    await user.type(input, "RUST ASYNC")
    expect(await screen.findByTestId("ws-name-error")).toHaveTextContent(
      "already exists"
    )
    expect(submit).toBeDisabled()
    await user.clear(input)
    await user.type(input, "Type theory")
    await waitFor(() => expect(submit).toBeEnabled())
    expect(screen.queryByTestId("ws-name-error")).not.toBeInTheDocument()
  })

  it("creates a workspace with the chosen icon and hue, and its card appears", async () => {
    const user = userEvent.setup()
    await openHome()
    await user.click(await screen.findByTestId("new-workspace"))
    await user.type(await screen.findByTestId("ws-name-input"), "Type theory")
    await user.click(screen.getByLabelText("brain"))
    await user.click(screen.getByLabelText("rose"))
    await user.type(
      screen.getByTestId("ws-description-input"),
      "Dependent types"
    )
    await waitFor(() => expect(screen.getByTestId("ws-submit")).toBeEnabled())
    await user.click(screen.getByTestId("ws-submit"))
    expect(await screen.findByText("Type theory")).toBeInTheDocument()
    const created = [...state.db.workspaces.values()].find(
      (w) => w.name === "Type theory"
    )
    expect(created).toMatchObject({
      icon: "brain",
      hue: "rose",
      description: "Dependent types",
    })
    await waitFor(() =>
      expect(screen.queryByTestId("workspace-dialog")).not.toBeInTheDocument()
    )
  })

  it("maps server field errors onto the form", async () => {
    server.use(
      http.post(`${API}/workspaces`, () =>
        apiError("validation", "Invalid workspace", {
          details: {
            fields: [{ path: "name", message: "Reserved name" }],
          },
        })
      )
    )
    const user = userEvent.setup()
    await openHome()
    await user.click(await screen.findByTestId("new-workspace"))
    await user.type(await screen.findByTestId("ws-name-input"), "Admin")
    await waitFor(() => expect(screen.getByTestId("ws-submit")).toBeEnabled())
    await user.click(screen.getByTestId("ws-submit"))
    expect(await screen.findByTestId("ws-name-error")).toHaveTextContent(
      "Reserved name"
    )
    expect(screen.getByTestId("workspace-dialog")).toBeInTheDocument()
  })

  it("renames from the card menu", async () => {
    const user = userEvent.setup()
    await openHome()
    await openMenu(user, RUST)
    await user.click(await screen.findByTestId(`workspace-rename-${RUST}`))
    const input = await screen.findByTestId("ws-name-input")
    expect(input).toHaveValue("Rust async")
    // Its own name is not a collision when editing.
    await waitFor(() => expect(screen.getByTestId("ws-submit")).toBeEnabled())
    await user.clear(input)
    await user.type(input, "Async Rust")
    await user.click(screen.getByTestId("ws-submit"))
    expect(await screen.findByText("Async Rust")).toBeInTheDocument()
    expect(state.db.workspaces.get(RUST)?.name).toBe("Async Rust")
  })

  it("opens the create dialog from /?new=1", async () => {
    await openHome("/?new=1")
    expect(await screen.findByTestId("workspace-dialog")).toBeInTheDocument()
  })
})

describe("delete confirmation", () => {
  it("keeps the destructive button disabled until the exact name is typed, then collapses the card", async () => {
    const user = userEvent.setup()
    await openHome()
    await openMenu(user, "ws_designnotes")
    await user.click(
      await screen.findByTestId("workspace-delete-ws_designnotes")
    )
    const confirm = await screen.findByTestId("delete-confirm")
    const input = screen.getByTestId("delete-confirm-input")
    expect(confirm).toBeDisabled()
    await user.type(input, "design notes")
    expect(confirm).toBeDisabled()
    await user.clear(input)
    await user.type(input, "Design notes")
    expect(confirm).toBeEnabled()
    await user.click(confirm)
    await waitFor(() =>
      expect(
        screen.queryByTestId("workspace-item-ws_designnotes")
      ).not.toBeInTheDocument()
    )
    expect(state.db.trash.map((t) => t.name)).toContain("Design notes")
  })

  it("Cancel leaves the workspace in place", async () => {
    const user = userEvent.setup()
    await openHome()
    await openMenu(user, RUST)
    await user.click(await screen.findByTestId(`workspace-delete-${RUST}`))
    await user.click(await screen.findByTestId("delete-cancel"))
    await waitFor(() =>
      expect(screen.queryByTestId("delete-dialog")).not.toBeInTheDocument()
    )
    expect(screen.getByTestId(`workspace-item-${RUST}`)).toBeInTheDocument()
  })
})

describe("archive and restore", () => {
  it("archives, shows it under the archived toggle, and restores it", async () => {
    const user = userEvent.setup()
    await openHome()
    await openMenu(user, "ws_distsys")
    await user.click(await screen.findByTestId("workspace-archive-ws_distsys"))
    await waitFor(() =>
      expect(
        screen.queryByTestId("workspace-item-ws_distsys")
      ).not.toBeInTheDocument()
    )
    await user.click(await screen.findByTestId("archived-toggle"))
    const archivedCard = await screen.findByTestId("workspace-item-ws_distsys")
    expect(archivedCard).toHaveAttribute("data-state", "archived")
    await openMenu(user, "ws_distsys")
    await user.click(await screen.findByTestId("workspace-archive-ws_distsys"))
    await waitFor(() =>
      expect(state.db.workspaces.get("ws_distsys")?.archived).toBe(false)
    )
    await waitFor(() =>
      expect(
        within(screen.getByTestId("workspace-grid")).getByTestId(
          "workspace-item-ws_distsys"
        )
      ).toBeInTheDocument()
    )
  })
})

describe("export", () => {
  it("shows progress, then a Download action when export.ready arrives over SSE", async () => {
    const user = userEvent.setup()
    await openHome()
    await openMenu(user, RUST)
    await user.click(await screen.findByTestId(`workspace-export-${RUST}`))
    expect(await screen.findByText("Exporting Rust async…")).toBeInTheDocument()
    await waitFor(() =>
      expect(FakeEventSource.instances.length).toBeGreaterThan(0)
    )
    const jobId = await waitFor(() => {
      const el = screen.getByText("Exporting Rust async…")
      expect(el).toBeInTheDocument()
      return "export_101"
    })
    FakeEventSource.last.open()
    await act(async () => {
      FakeEventSource.last.emit("export.ready", { jobId, token: "tok_1" }, 1)
    })
    expect(
      await screen.findByText("Rust async export is ready")
    ).toBeInTheDocument()
    expect(screen.getByRole("button", { name: "Download" })).toBeInTheDocument()
  })
})

describe("import", () => {
  it("rejects non-zip, empty and oversized files before uploading", () => {
    expect(validateImportFile(new File(["x"], "notes.txt"))).toMatch(/\.zip/)
    expect(validateImportFile(new File([], "a.zip"))).toMatch(/empty/)
    expect(validateImportFile(new File(["x"], "a.zip"))).toBeNull()
  })

  it("shows the server's validation error in the drop zone", async () => {
    const user = userEvent.setup()
    await openHome()
    await user.click(await screen.findByTestId("import-open"))
    const input = await screen.findByTestId("import-file-input")
    await user.upload(input, new File(["zip"], "export.zip"))
    await user.click(screen.getByTestId("import-submit"))
    expect(await screen.findByTestId("import-error")).toHaveTextContent(
      "The archive has no manifest"
    )
  })

  it("flags a wrong file type without calling the server", async () => {
    const user = userEvent.setup({ applyAccept: false })
    await openHome()
    await user.click(await screen.findByTestId("import-open"))
    await user.upload(
      await screen.findByTestId("import-file-input"),
      new File(["x"], "notes.txt")
    )
    expect(await screen.findByTestId("import-error")).toHaveTextContent(".zip")
    expect(screen.getByTestId("import-submit")).toBeDisabled()
  })
})

describe("onboarding", () => {
  it("creates the first workspace, offers detected folders and starts ingest", async () => {
    state.db.workspaces.clear()
    server.use(
      http.get(`${API}/workspaces/:ws/ingest/sources`, () =>
        HttpResponse.json({
          allow: [],
          claude: true,
          codex: true,
          agy: true,
          git: true,
          sinceDays: 30,
          candidates: [
            {
              path: "/Users/dev/code/raft-lab",
              sessions: 14,
              lastSeen: state.db.now,
              source: "claude",
            },
          ],
          overlaps: [],
        })
      ),
      http.put(`${API}/workspaces/:ws/ingest/sources`, () =>
        HttpResponse.json({})
      ),
      http.post(`${API}/workspaces/:ws/ingest/run`, () =>
        HttpResponse.json({ runId: "run_1" })
      )
    )
    const user = userEvent.setup()
    const { router } = renderApp("/")
    await screen.findByTestId("onboarding")
    await user.type(await screen.findByTestId("ws-name-input"), "First one")
    await waitFor(() => expect(screen.getByTestId("ws-submit")).toBeEnabled())
    await user.click(screen.getByTestId("ws-submit"))
    // Step 3 stays on screen after the list gains its first workspace.
    const candidate = await screen.findByTestId("onboarding-candidate-0")
    expect(screen.getByTestId("onboarding-start")).toBeDisabled()
    await user.click(candidate)
    await user.click(screen.getByTestId("onboarding-start"))
    await waitFor(() =>
      expect(router.state.location.pathname).toMatch(/^\/w\/ws_\d+\/ingest$/)
    )
  })

  it("step 3 can be skipped", async () => {
    state.db.workspaces.clear()
    const user = userEvent.setup()
    const { router } = renderApp("/")
    await screen.findByTestId("onboarding")
    await user.type(await screen.findByTestId("ws-name-input"), "Skippy")
    await waitFor(() => expect(screen.getByTestId("ws-submit")).toBeEnabled())
    await user.click(screen.getByTestId("ws-submit"))
    await user.click(await screen.findByTestId("onboarding-skip"))
    await waitFor(() =>
      expect(router.state.location.pathname).toMatch(/^\/w\/ws_\d+\/graph$/)
    )
  })
})

describe("trash", () => {
  it("empty state", async () => {
    renderApp("/trash")
    expect(await screen.findByTestId("empty-state")).toHaveTextContent(
      "Trash is empty"
    )
  })

  it("lists items with size and age, restores one and purges one after confirmation", async () => {
    state.db.trash = trashFixtures(state.db.now)
    server.use(
      http.post(`${API}/trash/:id/restore`, ({ params }) => {
        const index = state.db.trash.findIndex((t) => t.id === params["id"])
        const [item] = state.db.trash.splice(index, 1)
        return HttpResponse.json(
          listItem({
            ...damagedWorkspace(state.db.now),
            damaged: false,
            name: item?.name ?? "",
          })
        )
      })
    )
    const user = userEvent.setup()
    renderApp("/trash")
    const first = await screen.findByTestId("trash-item-trash_ws_old1")
    expect(first).toHaveTextContent("Compilers course")
    expect(first).toHaveTextContent("4.6 MB")
    expect(first).toHaveTextContent(/2 days ago/)

    await user.click(screen.getByTestId("trash-restore-trash_ws_old1"))
    await waitFor(() =>
      expect(
        screen.queryByTestId("trash-item-trash_ws_old1")
      ).not.toBeInTheDocument()
    )

    await user.click(screen.getByTestId("trash-purge-trash_ws_old2"))
    expect(await screen.findByTestId("purge-dialog")).toHaveTextContent(
      /permanently/
    )
    await user.click(screen.getByTestId("purge-confirm"))
    await waitFor(() => expect(state.db.trash).toHaveLength(0))
    expect(await screen.findByTestId("empty-state")).toBeInTheDocument()
  })

  it("error state with retry", async () => {
    server.use(http.get(`${API}/trash`, () => apiError("internal", "Nope")))
    renderApp("/trash")
    expect(
      await screen.findByTestId("error-state", undefined, { timeout: 4000 })
    ).toHaveTextContent("Nope")
    expect(screen.getByTestId("error-retry")).toBeInTheDocument()
  })
})

describe("workspace not found", () => {
  it("shows a picker instead of a blank screen", async () => {
    renderApp("/w/ws_missing/graph")
    expect(await screen.findByTestId("workspace-not-found")).toBeInTheDocument()
    expect(
      await screen.findByTestId(`not-found-option-${RUST}`)
    ).toHaveAttribute("href", `/w/${RUST}/graph`)
    expect(screen.getByTestId("not-found-home")).toHaveAttribute("href", "/")
  })
})

describe("model helpers", () => {
  it("name uniqueness ignores archived workspaces, case and the workspace itself", () => {
    const list = [
      { id: "a", name: "Rust", archived: false },
      { id: "b", name: "Old", archived: true },
    ]
    expect(isNameTaken(" rust ", list)).toBe(true)
    expect(isNameTaken("rust", list, "a")).toBe(false)
    expect(isNameTaken("old", list)).toBe(false)
  })

  it("remembers a section per workspace and falls back to the graph", () => {
    expect(lastSection("ws_x")).toBe("graph")
    rememberSection("ws_x", "digest")
    rememberSection("ws_x", "settings") // not a remembered section
    expect(lastSection("ws_x")).toBe("digest")
  })

  it("formats relative times", () => {
    const now = Date.UTC(2026, 9, 31)
    expect(relativeTime(null, now)).toBe("No activity yet")
    expect(relativeTime(now - 3 * 86_400_000, now)).toMatch(/3 days ago/)
  })
})
