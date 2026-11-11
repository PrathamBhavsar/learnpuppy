import { HttpResponse, http } from "msw"

import type { components } from "@/api/schema"

import { bus } from "../../mock/bus"
import { nextId, state } from "../db"
import {
  type MockBook,
  UPLOAD_CHAPTERS,
  chapterRow,
  newBook,
  summary,
} from "../fixtures/textbooks"
import { apiError, latency, notFound } from "../respond"

type Dto = components["schemas"]

const API = "*/api/v1/workspaces/:ws"
const MAX_BYTES = 50 * 1024 * 1024

const booksOf = (ws: unknown): MockBook[] => {
  const key = String(ws)
  const list = state.db.textbooks.get(key) ?? []
  state.db.textbooks.set(key, list)
  return list
}
const find = (ws: unknown, id: unknown) =>
  booksOf(ws).find((b) => b.book.id === id)

/** One import step per poll: the next chapter becomes ready, and the last one finishes the job. */
function advance(ws: string, b: MockBook) {
  const job = b.job
  if (job?.status !== "running") return
  const next = b.book.chapters.find((c) => c.state === "importing")
  if (next) {
    Object.assign(
      next,
      chapterRow(next.index, {
        ...UPLOAD_CHAPTERS[next.index]!,
        state: "ready",
      })
    )
    b.book.autoCalls += 1
    job.chaptersDone = (job.chaptersDone ?? 0) + 1
  }
  if (!b.book.chapters.some((c) => c.state === "importing")) {
    job.status = "done"
    b.book.jobId = null
  }
  bus.emit("job.updated", { job }, ws)
}

export const textbookHandlers = [
  http.get(`${API}/textbooks`, async ({ params }) => {
    await latency()
    const ws = String(params["ws"])
    for (const b of booksOf(ws)) advance(ws, b)
    const body: Dto["TextbookListDto"] = { items: booksOf(ws).map(summary) }
    return HttpResponse.json(body)
  }),
  http.get(`${API}/textbooks/:book`, async ({ params }) => {
    await latency()
    const b = find(params["ws"], params["book"])
    if (!b) return notFound("Book")
    advance(String(params["ws"]), b)
    return HttpResponse.json(b.book)
  }),
  http.get(`${API}/textbooks/:book/chapters/:n`, async ({ params }) => {
    const b = find(params["ws"], params["book"])
    const chapter = b?.book.chapters[Number(params["n"])]
    if (!b || !chapter) return notFound("Chapter")
    const body: Dto["ChapterDetailDto"] = {
      chapter,
      objectives: (b.items.get(chapter.index) ?? []).map((i, n) => ({
        id: i.objectiveId,
        text: `Objective ${n + 1}: ${i.q}`,
        startPage: chapter.startPage,
        endPage: chapter.endPage,
        items: 1,
      })),
    }
    return HttpResponse.json(body)
  }),
  http.post(`${API}/sources/textbook`, async ({ params, request }) => {
    await latency()
    const ws = String(params["ws"])
    const form = await request.formData().catch(() => null)
    const file = form?.get("file")
    if (!(file instanceof File) || !file.name.toLowerCase().endsWith(".pdf"))
      return apiError("validation", "Upload a PDF file")
    if (file.size > MAX_BYTES)
      return apiError("validation", "PDF is too large", {
        details: { maxBytes: MAX_BYTES },
      })
    const id = `bk_${file.name.replace(/\W/g, "").toLowerCase()}`
    const existing = find(ws, id)
    if (existing) {
      const body: Dto["TextbookUploadDto"] = {
        bookId: id,
        bytes: file.size,
        existing: true,
        jobId: existing.book.jobId ?? null,
      }
      return HttpResponse.json(body)
    }
    const job: Dto["JobItemDto"] = {
      id: nextId("tbj"),
      kind: "textbook",
      status: "running",
      title: `Import ${file.name}`,
      chatId: "",
      createdAt: state.db.now,
      bookId: id,
      chaptersDone: 0,
      chaptersTotal: UPLOAD_CHAPTERS.length,
      pagesRead: 0,
      totalPages: 52,
    }
    const b = newBook(
      id,
      file.name.replace(/\.pdf$/i, ""),
      UPLOAD_CHAPTERS.map((c) => ({ ...c, state: "importing" as const })),
      state.db.now,
      job
    )
    booksOf(ws).unshift(b)
    bus.emit("job.updated", { job }, ws)
    const body: Dto["TextbookUploadDto"] = {
      bookId: id,
      bytes: file.size,
      existing: false,
      jobId: job.id,
    }
    return HttpResponse.json(body, { status: 202 })
  }),
  http.post(`${API}/textbooks/:book/import`, ({ params }) => {
    const b = find(params["ws"], params["book"])
    if (!b) return notFound("Book")
    if (b.job?.status === "running")
      return apiError("conflict", "An import is already running", {
        details: { reason: "import_running" },
      })
    const pending = b.book.chapters.filter(
      (c) => c.state === "failed" || c.state === "queued"
    )
    if (pending.length === 0)
      return apiError("conflict", "Nothing to import", {
        details: { reason: "nothing_to_import" },
      })
    for (const c of pending) c.state = "importing"
    b.job = {
      id: nextId("tbj"),
      kind: "textbook",
      status: "running",
      title: `Import ${b.book.title}`,
      chatId: "",
      createdAt: state.db.now,
      bookId: b.book.id,
      chaptersDone: 0,
      chaptersTotal: pending.length,
    }
    b.book.jobId = b.job.id
    return HttpResponse.json({ jobId: b.job.id }, { status: 202 })
  }),
  http.post(`${API}/textbooks/:book/chapters/:n/check`, ({ params }) => {
    const b = find(params["ws"], params["book"])
    const n = Number(params["n"])
    const chapter = b?.book.chapters[n]
    if (!b || !chapter) return notFound("Chapter")
    if (chapter.state !== "ready")
      return apiError("conflict", "Chapter is not imported yet", {
        details: { reason: "chapter_not_ready" },
      })
    const checkId = `rc_${nextId("c").replace(/\W/g, "").padEnd(32, "0")}`
    b.checks.set(checkId, { chapter: n, answered: new Set() })
    const body: Dto["ReadingCheckDto"] = {
      checkId,
      chapter: n,
      items: (b.items.get(n) ?? []).map(
        ({ answer: _a, why: _w, ...item }) => item
      ),
    }
    return HttpResponse.json(body)
  }),
  http.post(
    `${API}/textbooks/:book/checks/:check/answer`,
    async ({ params, request }) => {
      const b = find(params["ws"], params["book"])
      const check = b?.checks.get(String(params["check"]))
      if (!b || !check) return notFound("Check")
      const body = (await request.json()) as Dto["CheckAnswerRequest"]
      const item = b.items.get(check.chapter)?.find((i) => i.id === body.item)
      if (!item) return notFound("Item")
      if (check.answered.has(item.id))
        return apiError("conflict", "Already answered", {
          details: { reason: "already_answered" },
        })
      check.answered.add(item.id)
      const correct =
        body.text.trim().toLowerCase() === item.answer.toLowerCase()
      if (check.answered.size === (b.items.get(check.chapter)?.length ?? 0)) {
        const row = b.book.chapters[check.chapter]
        if (row) row.status = "checked"
      }
      const feedback: Dto["CheckFeedbackDto"] = {
        correct,
        close: false,
        answer: item.answer,
        why: item.why,
        due: correct ? state.db.now + 3 * 86_400_000 : state.db.now + 600_000,
      }
      return HttpResponse.json(feedback)
    }
  ),
  http.post(`${API}/textbooks/:book/checks/:check/self-grade`, () => {
    return new HttpResponse(null, { status: 204 })
  }),
  http.get(`${API}/textbooks/:book/usage`, ({ params }) => {
    const b = find(params["ws"], params["book"])
    if (!b) return notFound("Book")
    const body: Dto["TextbookUsageDto"] = {
      autoCalls: b.book.autoCalls,
      requestedCalls: b.book.requestedCalls,
      callsCap: b.book.callsCap,
      cheapIn: 0,
      cheapOut: 0,
    }
    return HttpResponse.json(body)
  }),
]
