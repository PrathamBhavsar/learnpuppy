import { act, fireEvent, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, http } from "msw"
import { beforeEach, describe, expect, it } from "vitest"

import { useLive } from "@/stores/live"
import { FakeEventSource } from "@/test/fake-event-source"
import { state } from "@/test/msw/db"
import { apiError } from "@/test/msw/respond"
import { server } from "@/test/msw/server"
import { renderApp, resetUi } from "@/test/render"

const WS = "ws_rustasync"
const TURN = "*/api/v1/workspaces/:ws/chats/:id/turn"

beforeEach(resetUi)

async function open(chat: string) {
  const view = renderApp(`/w/${WS}/learn/${chat}`)
  await screen.findByTestId("classroom")
  return { ...view, user: userEvent.setup() }
}

/** The open workspace event stream; returns a function that delivers numbered server frames. */
async function stream() {
  const source = await waitFor(() => {
    const found = [...FakeEventSource.instances]
      .reverse()
      .find((s) => s.workspace === WS)
    if (!found) throw new Error("no workspace stream yet")
    return found
  })
  source.open()
  let id = 1
  return (type: string, data: unknown) => source.emit(type, data, id++)
}

/** Records turn bodies and answers 202, leaving the view untouched. */
function captureTurns() {
  const bodies: Record<string, unknown>[] = []
  server.use(
    http.post(TURN, async ({ request }) => {
      bodies.push((await request.json()) as Record<string, unknown>)
      return HttpResponse.json(
        { turnId: "turn_x", status: "started" },
        { status: 202 }
      )
    })
  )
  return bodies
}

describe("Classroom states", () => {
  it("shows a skeleton while loading, then the conversation", async () => {
    renderApp(`/w/${WS}/learn/c_teach_quiz`)
    expect(await screen.findByTestId("classroom-loading")).toBeInTheDocument()
    expect(await screen.findByTestId("classroom")).toBeInTheDocument()
    expect(screen.getByTestId("classroom-title")).toHaveTextContent(
      "Futures, polling and wakers"
    )
  })

  it("says so when the chat does not exist", async () => {
    renderApp(`/w/${WS}/learn/nope`)
    expect(await screen.findByText("Chat not found")).toBeInTheDocument()
    expect(
      screen.getByRole("link", { name: "Back to chats" })
    ).toBeInTheDocument()
  })

  it("shows the request id on a load error and retries", async () => {
    let calls = 0
    server.use(
      http.get("*/api/v1/workspaces/:ws/chats/:id", () => {
        calls += 1
        return calls === 1
          ? apiError("internal", "Disk read failed")
          : HttpResponse.json(state.db.chats.get(WS)?.[2])
      })
    )
    const { user } = await (async () => {
      renderApp(`/w/${WS}/learn/c_teach_quiz`)
      await screen.findByTestId("error-state")
      return { user: userEvent.setup() }
    })()
    expect(screen.getByText("Disk read failed")).toBeInTheDocument()
    expect(screen.getByText(/req_mock_/)).toBeInTheDocument()
    await user.click(screen.getByTestId("error-retry"))
    expect(await screen.findByTestId("classroom")).toBeInTheDocument()
  })

  it("invites the first message in an empty chat", async () => {
    state.db.chats.get(WS)?.push({
      id: "c_blank",
      kind: "teach",
      title: "Blank",
      phase: "probe",
      viewVersion: 1,
      messages: [],
      pending: null,
      plan: null,
      probeProgress: { asked: 0, max: 8 },
      verdicts: [],
      answers: [],
      jobs: [],
      turn: { state: "idle" },
    })
    await open("c_blank")
    expect(screen.getByTestId("classroom-empty")).toBeInTheDocument()
  })
})

describe("MessageList", () => {
  it("renders roles, tool chips, verdicts and the question dock", async () => {
    await open("c_teach_quiz")
    const list = await screen.findByTestId("message-list")
    expect(within(list).getAllByTestId("message-user")).toHaveLength(2)
    expect(within(list).getAllByTestId("message-assistant")).toHaveLength(3)
    expect(within(list).getAllByTestId("call-chip-quiz")[0]).toHaveTextContent(
      "Quiz asked"
    )
    const verdict = within(list).getByTestId("verdict-call_quiz_1")
    expect(verdict).toHaveAttribute("data-correct", "true")
    expect(verdict).toHaveTextContent("Right: futures are lazy.")
    // The answered question stays: prompt, the pick and the right option, not just the verdict.
    const recap = within(list).getByTestId("recap-call_quiz_1")
    expect(recap).toHaveTextContent("Is a future eager or lazy?")
    expect(within(recap).getByTestId("recap-option-A")).toHaveAttribute(
      "data-right",
      "false"
    )
    expect(within(recap).getByTestId("recap-option-B")).toHaveAttribute(
      "data-picked",
      "true"
    )
    expect(within(recap).getByTestId("recap-option-B")).toHaveAttribute(
      "data-right",
      "true"
    )
    expect(recap).toHaveTextContent("Confidence 5/5")
    expect(screen.getByTestId("question-dock")).toHaveAttribute(
      "data-tool",
      "quiz"
    )
    // The composer is replaced by the dock while a question is pending.
    expect(screen.queryByTestId("composer")).not.toBeInTheDocument()
  })

  it("renders the compaction divider, interrupted marker, tool notices and problem result", async () => {
    const view = await open("c_done")
    expect(await screen.findByTestId("message-compaction")).toHaveTextContent(
      "Earlier conversation summarized"
    )
    view.unmount()
    await open("c_interrupted")
    expect(await screen.findByTestId("message-interrupted")).toHaveTextContent(
      "Interrupted"
    )
    view.unmount()
    await open("c_practice_done")
    expect(await screen.findByTestId("message-tool")).toHaveTextContent(
      "Model solution"
    )
    expect(screen.getByTestId("problem-result")).toHaveAttribute(
      "data-outcome",
      "solved"
    )
  })

  it("copies a message", async () => {
    const { user } = await open("c_done")
    await screen.findAllByTestId("message-assistant")
    const assistant = screen.getAllByTestId("message-assistant")[0]
    if (!assistant) throw new Error("no assistant message")
    await user.click(within(assistant).getByTestId("message-copy"))
    expect(await navigator.clipboard.readText()).toContain("Bounded channels")
  })

  it("shows Jump to latest after scrolling up and returns to the bottom", async () => {
    await open("c_done")
    const list = await screen.findByTestId("message-list")
    Object.defineProperty(list, "scrollHeight", {
      value: 2000,
      configurable: true,
    })
    Object.defineProperty(list, "clientHeight", {
      value: 400,
      configurable: true,
    })
    list.scrollTop = 100
    fireEvent.scroll(list)
    const jump = await screen.findByTestId("jump-to-latest")
    fireEvent.click(jump)
    expect(list.scrollTop).toBe(2000)
    await waitFor(() =>
      expect(screen.queryByTestId("jump-to-latest")).not.toBeInTheDocument()
    )
  })

  it("streams the reply from chat.delta events and settles on chat.rest", async () => {
    await open("c_running")
    const emit = await stream()
    expect(await screen.findByTestId("message-streaming")).toBeInTheDocument()
    expect(screen.getByTestId("thinking-block")).toHaveTextContent("Thinking")
    act(() => {
      emit("chat.delta", {
        chatId: "c_running",
        turnId: "turn_live_1",
        seq: 1,
        kind: "text",
        text: "A future is ",
      })
      emit("chat.delta", {
        chatId: "c_running",
        turnId: "turn_live_1",
        seq: 2,
        kind: "text",
        text: "cancel safe when",
      })
    })
    await waitFor(() =>
      expect(screen.getByTestId("streaming-text")).toHaveTextContent(
        "A future is cancel safe when"
      )
    )
    // The server finishes the turn: view says idle with the settled assistant message.
    const chat = state.db.chats.get(WS)?.find((c) => c.id === "c_running")
    if (!chat) throw new Error("fixture missing")
    chat.messages.push({
      id: "m9",
      role: "assistant",
      text: "A future is cancel safe when dropping it loses no data.",
      calls: [],
      createdAt: Date.now(),
    })
    chat.turn = { state: "idle" }
    act(() => {
      emit("chat.rest", { chatId: "c_running", viewVersion: 9 })
    })
    await screen.findByText(/dropping it loses no data/)
    expect(screen.queryByTestId("streaming-text")).not.toBeInTheDocument()
    expect(useLive.getState().turns["c_running"]).toBeUndefined()
  })
})

describe("Composer", () => {
  it("sends on Enter, keeps Shift+Enter as a newline, and echoes the message", async () => {
    const bodies = captureTurns()
    const { user } = await open("c_done")
    const input = await screen.findByTestId("composer-input")
    await user.type(input, "line one{Shift>}{Enter}{/Shift}line two")
    expect(input).toHaveValue("line one\nline two")
    await user.keyboard("{Enter}")
    await waitFor(() => expect(bodies).toHaveLength(1))
    expect(bodies[0]).toMatchObject({
      type: "user",
      text: "line one\nline two",
    })
    expect(typeof bodies[0]?.["clientId"]).toBe("string")
    expect(input).toHaveValue("")
  })

  it("also sends with Cmd/Ctrl+Enter and never sends empty text", async () => {
    const bodies = captureTurns()
    const { user } = await open("c_done")
    const input = await screen.findByTestId("composer-input")
    expect(screen.getByTestId("composer-send")).toBeDisabled()
    await user.type(input, "hi")
    await user.keyboard("{Control>}{Enter}{/Control}")
    await waitFor(() => expect(bodies).toHaveLength(1))
  })

  it("lets you steer a running turn and stop it with Esc", async () => {
    const bodies = captureTurns()
    let cancelled = 0
    server.use(
      http.post("*/api/v1/workspaces/:ws/chats/:id/cancel", () => {
        cancelled += 1
        return HttpResponse.json({ cancelled: true, turnId: "turn_live_1" })
      })
    )
    const { user } = await open("c_running")
    const input = await screen.findByTestId("composer-input")
    expect(input).toBeDisabled()
    expect(screen.queryByTestId("composer-send")).not.toBeInTheDocument()
    await user.click(screen.getByTestId("composer-steer"))
    await user.type(input, "go slower")
    await user.keyboard("{Enter}")
    await waitFor(() => expect(bodies).toHaveLength(1))
    expect(bodies[0]).toMatchObject({ type: "steer", text: "go slower" })
    await user.keyboard("{Escape}")
    await waitFor(() => expect(cancelled).toBe(1))
    await user.click(screen.getByTestId("composer-stop"))
    await waitFor(() => expect(cancelled).toBe(2))
  })

  it("imports a Markdown note into the current chat", async () => {
    let query = ""
    let hasFile = false
    server.use(
      http.post("*/api/v1/workspaces/:ws/sources/note", async ({ request }) => {
        query = new URL(request.url).search
        const form = await request.formData()
        hasFile = form.get("file") !== null
        return HttpResponse.json(
          { artifactId: "source_note", chatId: "c_done", bytes: 12 },
          { status: 201 }
        )
      })
    )
    const { user } = await open("c_done")
    await user.upload(
      await screen.findByTestId("composer-note-file"),
      new File(["# Notes\nOwnership"], "notes.md", { type: "text/markdown" })
    )
    expect(await screen.findByText(/Note imported/)).toBeInTheDocument()
    expect(query).toBe("?chat=c_done")
    expect(hasFile).toBe(true)
  })

  it("rejects notes above 1 MB before uploading", async () => {
    const { user } = await open("c_done")
    await user.upload(
      await screen.findByTestId("composer-note-file"),
      new File([new Uint8Array(1024 * 1024 + 1)], "large.txt", {
        type: "text/plain",
      })
    )
    expect(await screen.findByRole("alert")).toHaveTextContent("over 1 MB")
  })

  it("uploads a PDF to the chat", async () => {
    let query = ""
    server.use(
      http.post("*/api/v1/workspaces/:ws/sources/pdf", ({ request }) => {
        query = new URL(request.url).search
        return HttpResponse.json(
          { jobId: "job_p", artifactId: "art_p", chatId: "c_done", bytes: 12 },
          { status: 202 }
        )
      })
    )
    const { user } = await open("c_done")
    const file = new File(["%PDF-1.4"], "book.pdf", { type: "application/pdf" })
    await user.upload(await screen.findByTestId("composer-file"), file)
    await waitFor(() => expect(query).toBe("?chat=c_done"))
    expect(await screen.findByText(/PDF uploaded/)).toBeInTheDocument()
  })
})

describe("answering questions", () => {
  it("posts the answer with callId and a clientId, and the dock goes busy", async () => {
    const bodies = captureTurns()
    const { user } = await open("c_teach_quiz")
    await screen.findByTestId("question-dock")
    await user.click(screen.getByTestId("quiz-option-A"))
    await user.click(screen.getByTestId("quiz-submit"))
    await waitFor(() => expect(bodies).toHaveLength(1))
    expect(bodies[0]).toMatchObject({
      type: "answer",
      callId: "call_quiz_2",
      answer: { choice: ["pending"], confidence: 3 },
    })
    expect(typeof bodies[0]?.["clientId"]).toBe("string")
  })

  it("renders the verdict under the answer once the server has graded it", async () => {
    const { user } = await open("c_teach_quiz")
    await screen.findByTestId("question-dock")
    await user.click(screen.getByTestId("quiz-option-B"))
    await user.click(screen.getByTestId("quiz-submit"))
    const verdict = await screen.findByTestId("verdict-call_quiz_2")
    expect(verdict).toHaveAttribute("data-correct", "false")
    await waitFor(() =>
      expect(screen.queryByTestId("question-dock")).not.toBeInTheDocument()
    )
  })

  it("resyncs silently on a stale question and says Question updated", async () => {
    const fresh = structuredClone(
      state.db.chats.get(WS)?.find((c) => c.id === "c_probe_pending")
    )
    if (!fresh) throw new Error("fixture missing")
    fresh.pending = null
    fresh.messages.push({
      id: "m77",
      role: "assistant",
      text: "Moving on to the plan.",
      calls: [],
      createdAt: Date.now(),
    })
    server.use(
      http.post(TURN, () =>
        apiError("stale_question", "That question is no longer pending", {
          details: { view: fresh },
        })
      )
    )
    const { user } = await open("c_probe_pending")
    await user.type(await screen.findByTestId("probe-text"), "no idea")
    // The server's truth moved on, so the refetch after the 409 agrees with the embedded view.
    const chats = state.db.chats.get(WS) ?? []
    chats.splice(
      chats.findIndex((c) => c.id === "c_probe_pending"),
      1,
      fresh
    )
    await user.keyboard("{Enter}")
    await screen.findByText("Moving on to the plan.")
    expect(screen.queryByTestId("question-dock")).not.toBeInTheDocument()
    expect(await screen.findByText("Question updated")).toBeInTheDocument()
  })
})

describe("turn status", () => {
  it("counts down while queued behind a rate limit and cancels", async () => {
    let cancelled = 0
    server.use(
      http.post("*/api/v1/workspaces/:ws/chats/:id/cancel", () => {
        cancelled += 1
        return HttpResponse.json({ cancelled: true, turnId: "turn_q_1" })
      })
    )
    const { user } = await open("c_queued")
    const banner = await screen.findByTestId("rate-limit-banner")
    expect(banner).toHaveTextContent("Waiting for an available model account")
    expect(screen.getByTestId("rate-limit-countdown").textContent).toMatch(
      /^(4[0-2]) s$/
    )
    await user.click(screen.getByTestId("rate-limit-cancel"))
    await waitFor(() => expect(cancelled).toBe(1))
  })

  it("shows the engine error and retries with the same clientId", async () => {
    const bodies = captureTurns()
    const { user } = await open("c_error")
    const card = await screen.findByTestId("turn-error")
    expect(card).toHaveTextContent("No model account answered")
    expect(card).toHaveAttribute("data-code", "engine_unavailable")
    await user.click(screen.getByTestId("turn-retry"))
    await waitFor(() => expect(bodies).toHaveLength(1))
    expect(bodies[0]).toEqual({ type: "resume", clientId: "client_err_1" })
  })

  it("offers Resume after a restart interrupted the turn", async () => {
    const bodies = captureTurns()
    const { user } = await open("c_interrupted")
    expect(await screen.findByTestId("turn-error")).toHaveTextContent(
      "The lesson was interrupted"
    )
    await user.click(screen.getByTestId("turn-resume"))
    await waitFor(() => expect(bodies).toHaveLength(1))
    expect(bodies[0]).toMatchObject({
      type: "resume",
      clientId: "client_int_1",
    })
  })

  it("copies diagnostics", async () => {
    const { user } = await open("c_error")
    await user.click(await screen.findByTestId("turn-copy-diagnostics"))
    const text = await navigator.clipboard.readText()
    expect(text).toContain("chat: c_error")
    expect(text).toContain("code: engine_unavailable")
  })

  it("warns when the engine is unavailable", async () => {
    const { useStatus } = await import("@/stores/status")
    // With an account ready the shell clears the flag again, so none is.
    for (const account of state.db.accounts) account.status = "disabled"
    await open("c_done")
    act(() => useStatus.setState({ engineUnavailable: true }))
    expect(await screen.findByTestId("engine-down")).toBeInTheDocument()
  })
})

describe("plan panel layout", () => {
  it("collapses, remembers the choice, and resizes with the keyboard", async () => {
    const { user } = await open("c_teach_quiz")
    const handle = await screen.findByTestId("plan-resize")
    expect(handle).toHaveAttribute("aria-valuenow", "340")
    handle.focus()
    await user.keyboard("{ArrowLeft}{ArrowLeft}")
    expect(handle).toHaveAttribute("aria-valuenow", "372")
    expect(localStorage.getItem("lp-classroom-plan-width")).toBe("372")
    await user.keyboard("{End}")
    expect(handle).toHaveAttribute("aria-valuenow", "560")
    await user.click(screen.getByTestId("plan-toggle"))
    expect(screen.queryByTestId("plan-panel")).not.toBeInTheDocument()
    expect(localStorage.getItem("lp-classroom-plan-open")).toBe("0")
  })
})
