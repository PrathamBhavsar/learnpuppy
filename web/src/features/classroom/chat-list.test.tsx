import { screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, http } from "msw"
import { beforeEach, describe, expect, it } from "vitest"

import { state } from "@/test/msw/db"
import { apiError } from "@/test/msw/respond"
import { server } from "@/test/msw/server"
import { renderApp, resetUi } from "@/test/render"

const WS = "ws_rustasync"

beforeEach(resetUi)

async function openList(path = `/w/${WS}/learn`) {
  const view = renderApp(path)
  await screen.findByTestId("chat-list")
  return { ...view, user: userEvent.setup() }
}

describe("ChatList", () => {
  it("shows a skeleton, then every chat with kind, phase and the pending dot", async () => {
    renderApp(`/w/${WS}/learn`)
    expect(await screen.findByTestId("chat-list-loading")).toBeInTheDocument()
    const item = await screen.findByTestId("chat-item-c_probe_pending")
    expect(within(item).getByText("Teach")).toBeInTheDocument()
    expect(within(item).getByTestId("chat-pending-dot")).toHaveAccessibleName(
      "Waiting for your answer"
    )
    expect(within(item).getByTestId("chat-phase")).toHaveAccessibleName(
      "Phase: Probe"
    )
    const practice = screen.getByTestId("chat-item-c_practice_problem")
    expect(within(practice).getByText("Practice")).toBeInTheDocument()
    expect(
      within(screen.getByTestId("chat-item-c_done")).queryByTestId(
        "chat-pending-dot"
      )
    ).not.toBeInTheDocument()
  })

  it("orders chats by recency and marks the open one", async () => {
    await openList(`/w/${WS}/learn/c_done`)
    const items = await screen.findAllByTestId(/^chat-item-/)
    const ids = items.map((li) => li.dataset["testid"])
    expect(ids.indexOf("chat-item-c_queued")).toBeLessThan(
      ids.indexOf("chat-item-c_done")
    )
    expect(screen.getByTestId("chat-item-c_done")).toHaveAttribute(
      "data-active",
      "true"
    )
  })

  it("has an empty state", async () => {
    state.db.chats.set(WS, [])
    await openList()
    expect(await screen.findByTestId("chat-list-empty")).toBeInTheDocument()
    // The classroom pane invites a first chat in one step.
    expect(screen.getByTestId("start-learning")).toBeInTheDocument()
  })

  it("has an error state with retry", async () => {
    let calls = 0
    server.use(
      http.get("*/api/v1/workspaces/:ws/chats", () => {
        calls += 1
        return calls === 1
          ? apiError("internal", "Chats index is unreadable")
          : HttpResponse.json({ items: [], nextCursor: null, total: 0 })
      })
    )
    const { user } = await (async () => {
      renderApp(`/w/${WS}/learn`)
      await screen.findByText("Chats index is unreadable")
      return { user: userEvent.setup() }
    })()
    await user.click(screen.getByTestId("error-retry"))
    expect(await screen.findByTestId("chat-list-empty")).toBeInTheDocument()
  })

  it("renames a chat and maps server field errors onto the form", async () => {
    const { user } = await openList()
    await user.click(await screen.findByTestId("chat-menu-c_done"))
    await user.click(await screen.findByRole("menuitem", { name: /Rename/ }))
    const input = await screen.findByTestId("rename-input")
    await user.clear(input)
    await user.click(screen.getByTestId("rename-submit"))
    expect(await screen.findByText("Give the chat a name")).toBeInTheDocument()
    await user.type(input, "Backpressure notes")
    await user.click(screen.getByTestId("rename-submit"))
    await waitFor(() =>
      expect(screen.queryByTestId("rename-dialog")).not.toBeInTheDocument()
    )
    expect(await screen.findByText("Backpressure notes")).toBeInTheDocument()
    expect(state.db.chats.get(WS)?.find((c) => c.id === "c_done")?.title).toBe(
      "Backpressure notes"
    )
  })

  it("confirms before deleting, states the consequence, and leaves the deleted chat", async () => {
    const { user, router } = await openList(`/w/${WS}/learn/c_done`)
    await screen.findByTestId("classroom")
    await user.click(screen.getByTestId("chat-menu-c_done"))
    await user.click(await screen.findByRole("menuitem", { name: /Delete/ }))
    const dialog = await screen.findByTestId("delete-dialog")
    expect(dialog).toHaveTextContent("moves to this workspace")
    await user.click(screen.getByTestId("delete-confirm"))
    await waitFor(() =>
      expect(screen.queryByTestId("chat-item-c_done")).not.toBeInTheDocument()
    )
    expect(router.state.location.pathname).toBe(`/w/${WS}/learn`)
  })

  it("exports a chat as Markdown from the context menu", async () => {
    let downloaded = ""
    const create = URL.createObjectURL
    URL.createObjectURL = (blob: Blob | MediaSource) => {
      void (blob as Blob).text().then((t) => (downloaded = t))
      return "blob:chat"
    }
    URL.revokeObjectURL = () => undefined
    try {
      const { user } = await openList()
      await user.click(await screen.findByTestId("chat-menu-c_done"))
      await user.click(
        await screen.findByRole("menuitem", { name: /Export as Markdown/ })
      )
      await waitFor(() =>
        expect(downloaded).toContain("# Channels and backpressure")
      )
    } finally {
      URL.createObjectURL = create
    }
  })
})

describe("StartLearning and NewChatDialog", () => {
  it("creates a chat from the empty state in one step and opens it", async () => {
    const { user, router } = await openList()
    await user.type(await screen.findByTestId("start-topic"), "Rust lifetimes")
    await user.click(screen.getByTestId("start-submit"))
    await waitFor(() =>
      expect(router.state.location.pathname).toMatch(
        new RegExp(`^/w/${WS}/learn/c_\\d+`)
      )
    )
    expect(await screen.findByTestId("classroom")).toBeInTheDocument()
  })

  it("will not start without a topic", async () => {
    const { user } = await openList()
    await user.click(await screen.findByTestId("start-submit"))
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Say what you want to learn"
    )
  })

  it("New chat dialog previews roles, picks graph nodes and starts a practice chat", async () => {
    let body: unknown
    server.use(
      http.post("*/api/v1/workspaces/:ws/chats", async ({ request }) => {
        body = await request.json()
        return HttpResponse.json(
          { chatId: "c_new", turnId: "turn_new" },
          { status: 202 }
        )
      })
    )
    const { user } = await openList()
    await user.click(await screen.findByTestId("new-chat"))
    const roles = await screen.findByTestId("new-chat-roles")
    await waitFor(() =>
      expect(roles).toHaveTextContent("claude / claude-opus-5-5")
    )
    expect(roles).toHaveTextContent("prober")
    await user.click(screen.getByTestId("kind-practice"))
    expect(roles).toHaveTextContent("practice")
    expect(roles).not.toHaveTextContent("teacher")
    await user.click(screen.getByTestId("new-chat-use-nodes"))
    const picker = await screen.findByTestId("node-picker")
    const [first] = await within(picker).findAllByRole("checkbox")
    if (!first) throw new Error("no nodes")
    await user.click(first)
    await user.click(screen.getByTestId("new-chat-submit"))
    await waitFor(() => expect(body).toBeDefined())
    expect(body).toMatchObject({ kind: "practice", topic: "" })
    expect((body as { seedNodeIds: string[] }).seedNodeIds).toHaveLength(1)
  })

  it("warns when no model account is ready", async () => {
    for (const account of state.db.accounts) account.status = "disabled"
    const { user } = await openList()
    await user.click(await screen.findByTestId("new-chat"))
    expect(await screen.findByTestId("new-chat-no-account")).toBeInTheDocument()
  })

  it("shows server validation on the topic field", async () => {
    server.use(
      http.post("*/api/v1/workspaces/:ws/chats", () =>
        apiError("validation", "Invalid topic", {
          details: {
            fields: [{ path: "topic", message: "Topic is too vague" }],
          },
        })
      )
    )
    const { user } = await openList()
    await user.click(await screen.findByTestId("new-chat"))
    await user.type(await screen.findByTestId("new-chat-topic"), "stuff")
    await user.click(screen.getByTestId("new-chat-submit"))
    expect(await screen.findByText("Topic is too vague")).toBeInTheDocument()
  })
})
