import { beforeEach, describe, expect, it } from "vitest"

import { state } from "../db"
import { READY_CHAPTERS, newBook } from "../fixtures/textbooks"

const base = "http://localhost/api/v1/workspaces/ws_rustasync"
const call = (path: string, init?: RequestInit) => fetch(`${base}${path}`, init)
const json = (body: unknown): RequestInit => ({
  method: "POST",
  headers: { "content-type": "application/json" },
  body: JSON.stringify(body),
})
const pdf = (tag: string) => {
  const form = new FormData()
  form.append(
    "file",
    new File([`%PDF-1.4 ${tag}`], `${tag}.pdf`, { type: "application/pdf" })
  )
  return { method: "POST", body: form }
}

beforeEach(() => {
  state.db.textbooks.set("ws_rustasync", [
    newBook("bk1", "Async Rust", READY_CHAPTERS, state.db.now),
  ])
})

describe("textbook mock handlers", () => {
  it("lists books and details with the contract's shape", async () => {
    const list = await (await call("/textbooks")).json()
    expect(list.items[0]).toMatchObject({
      id: "bk1",
      state: "ready",
      chapters: 3,
    })
    const book = await (await call("/textbooks/bk1")).json()
    expect(book.chapters[0]).toMatchObject({
      startPage: 9,
      endPage: 40,
      status: "due",
    })
    expect((await call("/textbooks/nope")).status).toBe(404)
  })

  it("a reading check never carries answers, and grading is server-side", async () => {
    const check = await (
      await call("/textbooks/bk1/chapters/0/check", { method: "POST" })
    ).json()
    const text = JSON.stringify(check)
    for (const key of ["answer", "why", "accept", "correct"])
      expect(text).not.toContain(`"${key}"`)
    const item = check.items[0]
    const res = await call(
      `/textbooks/bk1/checks/${check.checkId}/answer`,
      json({ item: item.id, text: "Blocks the caller" })
    )
    expect(await res.json()).toMatchObject({
      correct: false,
      answer: "Nothing",
    })
    const again = await call(
      `/textbooks/bk1/checks/${check.checkId}/answer`,
      json({ item: item.id, text: "Nothing" })
    )
    expect(again.status).toBe(409)
    expect((await again.json()).error.details.reason).toBe("already_answered")
  })

  it("refuses a check on a chapter that is not ready", async () => {
    state.db.textbooks.get("ws_rustasync")![0]!.book.chapters[2]!.state =
      "failed"
    const res = await call("/textbooks/bk1/chapters/2/check", {
      method: "POST",
    })
    expect(res.status).toBe(409)
    expect((await res.json()).error.details.reason).toBe("chapter_not_ready")
  })

  it("accepts a PDF once, reports a repeat as existing, and rejects other files", async () => {
    const first = await call("/sources/textbook", pdf("a"))
    expect(first.status).toBe(202)
    const again = await call("/sources/textbook", pdf("a"))
    expect(again.status).toBe(200)
    expect((await again.json()).existing).toBe(true)
    const bad = new FormData()
    bad.append("file", new File(["hello"], "a.txt"))
    expect(
      (await call("/sources/textbook", { method: "POST", body: bad })).status
    ).toBe(400)
  })

  it("import conflicts while running and when nothing is pending", async () => {
    expect((await call("/textbooks/bk1/import", json({}))).status).toBe(409)
    await call("/sources/textbook", pdf("b"))
    const id = (await (await call("/textbooks")).json()).items[0].id
    const running = await call(`/textbooks/${id}/import`, json({}))
    expect((await running.json()).error.details.reason).toBe("import_running")
  })

  it("lists the import as a textbook job with chapter progress", async () => {
    await call("/sources/textbook", pdf("c"))
    const jobs = await (await call("/jobs")).json()
    expect(jobs.items[0]).toMatchObject({
      kind: "textbook",
      bookId: expect.stringMatching(/^bk_/),
      chaptersDone: 0,
      chaptersTotal: 3,
    })
  })
})
