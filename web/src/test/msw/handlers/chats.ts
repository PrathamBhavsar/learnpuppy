import { HttpResponse, http } from "msw"

import type {
  ClassroomView,
  CreateChatBody,
  TurnBody,
} from "@/api/pending-types"

import { bus } from "../../mock/bus"
import { nextId, state } from "../db"
import { summarize } from "../fixtures/chats"
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

export const chatHandlers = [
  http.get(`${API}/chats`, async ({ params }) => {
    await latency()
    const chats = chatsOf(params["ws"])
    if (!chats) return notFound("Workspace")
    const items = chats.map(summarize).sort((a, b) => b.updatedAt - a.updatedAt)
    return HttpResponse.json(items)
  }),

  http.post(`${API}/chats`, async ({ params, request }) => {
    await latency()
    const wsId = String(params["ws"])
    const chats = chatsOf(wsId)
    if (!chats) return notFound("Workspace")
    const body = (await request.json()) as CreateChatBody
    if (body.kind === "teach" && body.topic.trim() === "")
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
      title: body.topic.trim().slice(0, 60) || "Practice",
      phase: body.kind === "teach" ? "probe" : "practice",
      viewVersion: 1,
      messages: [
        {
          id: nextId("m"),
          role: "user",
          text: body.topic,
          calls: [],
          createdAt: Date.now(),
        },
      ],
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
    return HttpResponse.json({ chatId: id })
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

  http.post(`${API}/chats/:id/turn`, async ({ params, request }) => {
    await latency()
    const wsId = String(params["ws"])
    const chat = find(wsId, params["id"])
    if (!chat) return notFound("Chat")
    const body = (await request.json()) as TurnBody
    const replay = state.db.idempotent.get(`once:${body.clientId}`)
    if (replay) return HttpResponse.json(replay, { status: 202 })
    if (chat.turn.state === "running" && body.type !== "steer")
      return apiError(
        "turn_in_progress",
        "A turn is already running for this chat"
      )
    if (body.type === "answer" && chat.pending?.callId !== body.callId)
      return apiError("stale_question", "That question is no longer pending", {
        details: { view: chat },
      })
    const result = once(body.clientId, () => ({ turnId: nextId("turn") }))
    const text = body.type === "answer" ? body.answer : body.text
    chat.messages.push({
      id: nextId("m"),
      role: body.type === "steer" ? "steer" : "user",
      text,
      calls: [],
      createdAt: Date.now(),
    })
    if (body.type === "answer") chat.pending = null
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
    chat.turn = { state: "idle" }
    const last = chat.messages[chat.messages.length - 1]
    if (last?.role === "assistant") last.interrupted = true
    chat.viewVersion += 1
    bus.emit(
      "chat.rest",
      { chatId: chat.id, viewVersion: chat.viewVersion },
      String(params["ws"])
    )
    return HttpResponse.json({})
  }),

  http.get(`${API}/jobs`, ({ params }) => {
    const jobs = (chatsOf(params["ws"]) ?? []).flatMap((c) => c.jobs)
    return HttpResponse.json(jobs)
  }),
  http.post(`${API}/jobs/:id/abort`, ({ params }) => {
    for (const chat of chatsOf(params["ws"]) ?? []) {
      const job = chat.jobs.find((j) => j.id === params["id"])
      if (job) {
        job.status = "aborted"
        bus.emit("job.updated", { job }, String(params["ws"]))
        return HttpResponse.json(job)
      }
    }
    return notFound("Job")
  }),
  http.post(`${API}/sources/pdf`, ({ params, request }) => {
    const chatId = new URL(request.url).searchParams.get("chat")
    const chat = chatId ? find(params["ws"], chatId) : undefined
    const job = {
      id: nextId("job"),
      kind: "pdf",
      status: "running",
      title: "Reading uploaded PDF",
      artifactId: null,
    }
    chat?.jobs.push(job)
    bus.emit("job.updated", { job }, String(params["ws"]))
    return HttpResponse.json(job)
  }),
]
