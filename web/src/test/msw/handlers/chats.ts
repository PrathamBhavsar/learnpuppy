import { HttpResponse, http } from "msw"

import type {
  AnswerInput,
  ClassroomView,
  CreateChatBody,
  TurnBody,
} from "@/api/pending-types"

import { bus } from "../../mock/bus"
import { nextId, state } from "../db"
import { ARTIFACTS, summarize } from "../fixtures/chats"
import { apiError, latency, notFound, once } from "../respond"

const API = "*/api/v1/workspaces/:ws"

const chatsOf = (ws: unknown) => state.db.chats.get(String(ws))

function find(ws: unknown, id: unknown): ClassroomView | undefined {
  return chatsOf(ws)?.find((c) => c.id === id)
}

const REPLY =
  "Good. A **waker** is how a future asks to be polled again: when `poll` returns `Pending`, it must have stored the waker from its `Context` so the I/O driver can wake the task later."

/** Mock-mode only: stream a reply as deltas, then settle the turn (chat.rest). */
function streamReply(ws: string, chat: ClassroomView, turnId: string): void {
  const words = REPLY.split(" ")
  let seq = 0
  let index = 0
  const timer = setInterval(() => {
    if (index >= words.length) {
      clearInterval(timer)
      chat.messages.push({
        id: nextId("m"),
        role: "assistant",
        text: REPLY,
        calls: [],
        createdAt: Date.now(),
      })
      chat.turn = { state: "idle" }
      chat.viewVersion += 1
      bus.emit(
        "chat.rest",
        { chatId: chat.id, viewVersion: chat.viewVersion },
        ws
      )
      return
    }
    seq += 1
    const text = (index === 0 ? "" : " ") + words[index]
    index += 1
    bus.emit(
      "chat.delta",
      { chatId: chat.id, turnId, seq, kind: "text", text },
      ws
    )
  }, 60)
}

/** How an answer reads in the transcript (the server renders it from the question's options). */
function renderAnswer(chat: ClassroomView, answer: AnswerInput): string {
  const options = chat.pending?.options ?? []
  const picked = (answer.choice ?? []).map((key) => {
    if (key === chat.pending?.idkKey) return "I don't know"
    const index = options.findIndex((o) => o.key === key)
    const option = options[index]
    return option ? `${String.fromCharCode(65 + index)}. ${option.text}` : key
  })
  return [picked.join(", "), answer.text ?? ""].filter(Boolean).join("; ")
}

export const chatHandlers = [
  http.get(`${API}/chats`, async ({ params }) => {
    await latency()
    const chats = chatsOf(params["ws"])
    if (!chats) return notFound("Workspace")
    const items = chats.map(summarize).sort((a, b) => b.updatedAt - a.updatedAt)
    return HttpResponse.json({ items, nextCursor: null, total: items.length })
  }),

  http.post(`${API}/chats`, async ({ params, request }) => {
    await latency()
    const wsId = String(params["ws"])
    const chats = chatsOf(wsId)
    if (!chats) return notFound("Workspace")
    const body = (await request.json()) as CreateChatBody
    const topic = (body.topic ?? "").trim()
    if (body.kind === "teach" && topic === "")
      return apiError("validation", "Add a topic", {
        details: {
          fields: [{ path: "topic", message: "Say what you want to learn" }],
        },
      })
    const id = nextId("c")
    const turnId = nextId("turn")
    const chat: ClassroomView = {
      id,
      kind: body.kind,
      title: topic.slice(0, 60) || "Practice",
      phase: body.kind === "teach" ? "probe" : "practice",
      viewVersion: 1,
      answers: [],
      messages: topic
        ? [
            {
              id: nextId("m"),
              role: "user",
              text: topic,
              calls: [],
              createdAt: Date.now(),
            },
          ]
        : [],
      pending: null,
      plan: null,
      probeProgress: { asked: 0, max: 8 },
      verdicts: [],
      jobs: [],
      turn: { state: "running", turnId, startedAt: Date.now() },
    }
    chats.unshift(chat)
    bus.emit("chat.created", { chatId: id }, wsId)
    if (state.db.simulateTurns) streamReply(wsId, chat, turnId)
    return HttpResponse.json({ chatId: id, turnId }, { status: 202 })
  }),

  http.get(`${API}/chats/:id`, async ({ params }) => {
    await latency()
    const chat = find(params["ws"], params["id"])
    return chat ? HttpResponse.json(chat) : notFound("Chat")
  }),

  http.patch(`${API}/chats/:id`, async ({ params, request }) => {
    const chat = find(params["ws"], params["id"])
    if (!chat) return notFound("Chat")
    const { title } = (await request.json()) as { title: string }
    if (title.trim() === "")
      return apiError("validation", "Invalid title", {
        details: {
          fields: [{ path: "title", message: "Give the chat a name" }],
        },
      })
    chat.title = title.trim()
    bus.emit("chat.updated", { chatId: chat.id }, String(params["ws"]))
    return HttpResponse.json(summarize(chat))
  }),

  http.delete(`${API}/chats/:id`, ({ params }) => {
    const chats = chatsOf(params["ws"])
    const index = chats?.findIndex((c) => c.id === params["id"]) ?? -1
    if (!chats || index < 0) return notFound("Chat")
    chats.splice(index, 1)
    bus.emit(
      "chat.updated",
      { chatId: String(params["id"]) },
      String(params["ws"])
    )
    return new HttpResponse(null, { status: 204 })
  }),

  http.get(`${API}/chats/:id/export.md`, ({ params }) => {
    const chat = find(params["ws"], params["id"])
    if (!chat) return notFound("Chat")
    const body = [
      `# ${chat.title}`,
      ...chat.messages.map((m) => `**${m.role}:** ${m.text}`),
    ].join("\n\n")
    return new HttpResponse(body, {
      headers: { "content-type": "text/markdown" },
    })
  }),

  http.post(`${API}/chats/:id/turn`, async ({ params, request }) => {
    await latency()
    const wsId = String(params["ws"])
    const chat = find(wsId, params["id"])
    if (!chat) return notFound("Chat")
    const body = (await request.json()) as TurnBody
    const replay = state.db.idempotent.get(`once:${body.clientId}`)
    if (replay && body.type !== "resume")
      return HttpResponse.json(replay, { status: 202 })
    if (chat.turn.state === "running" && body.type !== "steer")
      return apiError(
        "turn_in_progress",
        "A turn is already running for this chat"
      )
    if (body.type === "answer" && chat.pending?.callId !== body.callId)
      return apiError("stale_question", "That question is no longer pending", {
        details: { view: chat },
      })
    if (body.type === "user" && chat.pending)
      return apiError("conflict", "Answer the open question first")
    const result = once(body.clientId, () => ({
      turnId: nextId("turn"),
      status: "started" as const,
    }))
    if (body.type === "user" || body.type === "steer") {
      chat.messages.push({
        id: nextId("m"),
        role: body.type === "steer" ? "steer" : "user",
        text: body.text,
        calls: [],
        createdAt: Date.now(),
      })
    }
    if (body.type === "answer") {
      const pending = chat.pending
      chat.messages.push({
        id: nextId("m"),
        role: "user",
        text: renderAnswer(chat, body.answer),
        answersCallId: body.callId,
        calls: [],
        createdAt: Date.now(),
      })
      if (pending && (pending.tool === "quiz" || pending.tool === "probe")) {
        chat.verdicts.push({
          callId: body.callId,
          correct: body.answer.choice?.[0] === pending.options?.[0]?.key,
          explanation: "Poll returns Pending after registering the waker.",
        })
      }
      chat.pending = null
    }
    chat.turn = {
      state: "running",
      turnId: result.turnId,
      startedAt: Date.now(),
    }
    chat.viewVersion += 1
    bus.emit("chat.updated", { chatId: chat.id }, wsId)
    if (state.db.simulateTurns) streamReply(wsId, chat, result.turnId)
    return HttpResponse.json(result, { status: 202 })
  }),

  http.post(`${API}/chats/:id/cancel`, ({ params }) => {
    const chat = find(params["ws"], params["id"])
    if (!chat) return notFound("Chat")
    const wasRunning = chat.turn.state === "running"
    const turnId = chat.turn.turnId ?? null
    chat.turn = { state: "idle" }
    const last = chat.messages[chat.messages.length - 1]
    if (last?.role === "assistant") last.interrupted = true
    chat.viewVersion += 1
    bus.emit(
      "chat.rest",
      { chatId: chat.id, viewVersion: chat.viewVersion },
      String(params["ws"])
    )
    return HttpResponse.json({ cancelled: wasRunning, turnId })
  }),

  http.get(`${API}/jobs`, ({ params }) => {
    const items = (chatsOf(params["ws"]) ?? []).flatMap((c) =>
      c.jobs.map((job) => ({ ...job, chatId: c.id, createdAt: state.db.now }))
    )
    // Textbook imports are workspace jobs with no chat (chatId "").
    const books = (state.db.textbooks.get(String(params["ws"])) ?? []).flatMap(
      (b) => (b.job ? [b.job] : [])
    )
    return HttpResponse.json({ items: [...books, ...items] })
  }),
  http.post(`${API}/jobs/:id/abort`, ({ params }) => {
    for (const chat of chatsOf(params["ws"]) ?? []) {
      const job = chat.jobs.find((j) => j.id === params["id"])
      if (job) {
        job.status = "failed"
        bus.emit("job.updated", { job }, String(params["ws"]))
        return HttpResponse.json({
          ...job,
          chatId: chat.id,
          createdAt: state.db.now,
        })
      }
    }
    for (const b of state.db.textbooks.get(String(params["ws"])) ?? []) {
      const job = b.job
      if (!job || job.id !== params["id"]) continue
      job.status = "failed"
      job.error = "cancelled"
      b.book.jobId = null
      // Chapters not finished stay queued, as on the server.
      for (const c of b.book.chapters)
        if (c.state === "importing") c.state = "queued"
      bus.emit("job.updated", { job }, String(params["ws"]))
      return HttpResponse.json(job)
    }
    return notFound("Job")
  }),
  http.post(`${API}/sources/pdf`, ({ params, request }) => {
    const chatId = new URL(request.url).searchParams.get("chat")
    const chat = chatId ? find(params["ws"], chatId) : undefined
    if (!chat) return notFound("Chat")
    const job = {
      id: nextId("job"),
      kind: "pdf",
      status: "running" as const,
      title: "Reading uploaded PDF",
      artifactId: null,
    }
    chat.jobs.push(job)
    bus.emit("job.updated", { job }, String(params["ws"]))
    return HttpResponse.json(
      {
        jobId: job.id,
        artifactId: nextId("art"),
        chatId: chat.id,
        bytes: 1024,
      },
      { status: 202 }
    )
  }),
  http.get(`${API}/artifacts/:id`, ({ params }) => {
    const artifact = ARTIFACTS[String(params["id"])]
    if (!artifact) return notFound("Artifact")
    return new HttpResponse(artifact.body, {
      headers: { "content-type": artifact.type },
    })
  }),
]
