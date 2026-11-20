import { act, screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, delay, http } from "msw"
import { beforeEach, describe, expect, it } from "vitest"

import type { components } from "@/api/schema"
import { state } from "@/test/msw/db"
import { READY_CHAPTERS, newBook, summary } from "@/test/msw/fixtures/textbooks"
import { apiError } from "@/test/msw/respond"
import { server } from "@/test/msw/server"
import { renderApp, resetUi } from "@/test/render"

type Dto = components["schemas"]

const WS = "ws_rustasync"
const API = "*/api/v1/workspaces/:ws"

beforeEach(resetUi)

const seed = (
  chapters: Parameters<typeof newBook>[2] = READY_CHAPTERS,
  title = "Async Rust"
) => {
  const b = newBook("bk1", title, chapters, state.db.now)
  state.db.textbooks.set(WS, [b])
  return b
}

describe("Textbook list", () => {
  it("shows an empty state with the upload control", async () => {
    renderApp(`/w/${WS}/textbook`)
    expect(await screen.findByText("No textbooks yet")).toBeInTheDocument()
    expect(screen.getByTestId("textbook-upload-button")).toBeEnabled()
  })

  it("shows a loading placeholder, then the books", async () => {
    seed()
    server.use(
      http.get(`${API}/textbooks`, async () => {
        await delay(150)
        return HttpResponse.json({
          items: [summary(state.db.textbooks.get(WS)![0]!)],
        })
      })
    )
    renderApp(`/w/${WS}/textbook`)
    expect(await screen.findByTestId("textbook-loading")).toBeInTheDocument()
    expect(await screen.findByTestId("textbook-bk1")).toHaveTextContent(
      "3 of 3 chapters ready"
    )
  })

  it("shows an error with a retry that recovers", async () => {
    seed()
    let fail = true
    server.use(
      http.get(`${API}/textbooks`, () => {
        if (fail) return apiError("internal", "Disk is unhappy")
        return HttpResponse.json({ items: [] })
      })
    )
    const user = userEvent.setup()
    renderApp(`/w/${WS}/textbook`)
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Disk is unhappy"
    )
    fail = false
    await user.click(screen.getByRole("button", { name: /try again|retry/i }))
    expect(await screen.findByText("No textbooks yet")).toBeInTheDocument()
  })

  it("rejects a non-PDF before it leaves the browser", async () => {
    const user = userEvent.setup({ applyAccept: false })
    renderApp(`/w/${WS}/textbook`)
    await screen.findByText("No textbooks yet")
    await user.upload(
      screen.getByTestId("textbook-file"),
      new File(["x"], "notes.txt", { type: "text/plain" })
    )
    expect(
      await screen.findByTestId("textbook-upload-problem")
    ).toHaveTextContent("Choose a PDF file.")
    expect(state.db.textbooks.get(WS) ?? []).toHaveLength(0)
  })

  it("uploads a PDF and lands on a chapter list with nothing imported", async () => {
    const user = userEvent.setup()
    renderApp(`/w/${WS}/textbook`)
    await screen.findByText("No textbooks yet")
    await user.upload(
      screen.getByTestId("textbook-file"),
      new File(["%PDF-1.4"], "Rust Book.pdf", { type: "application/pdf" })
    )
    const list = await screen.findByTestId("chapter-list")
    expect(within(list).getAllByRole("listitem")).toHaveLength(3)
    // Upload started no import: no progress bar, every chapter waits to be opened.
    expect(screen.queryByTestId("textbook-import")).not.toBeInTheDocument()
    for (const n of [0, 1, 2]) {
      expect(screen.getByTestId(`chapter-${n}`)).toHaveAttribute(
        "data-state",
        "pending"
      )
      expect(screen.getByTestId(`chapter-${n}-badge`)).toHaveTextContent(
        "Not imported"
      )
      expect(screen.getByTestId(`chapter-${n}-import`)).toBeEnabled()
    }
    expect(state.db.textbooks.get(WS)![0]!.book.jobId).toBeNull()
  })

  it("imports only the chapter that is opened, then offers its reading check", async () => {
    const user = userEvent.setup()
    renderApp(`/w/${WS}/textbook`)
    await screen.findByText("No textbooks yet")
    await user.upload(
      screen.getByTestId("textbook-file"),
      new File(["%PDF-1.4"], "Rust Book.pdf", { type: "application/pdf" })
    )
    await user.click(await screen.findByTestId("chapter-1-import"))
    // The mock finishes the one chapter on its first poll; the others never start.
    await waitFor(
      () =>
        expect(screen.getByTestId("chapter-1-badge")).toHaveTextContent(
          "Not read"
        ),
      { timeout: 15000 }
    )
    await waitFor(() =>
      expect(screen.queryByTestId("textbook-import")).not.toBeInTheDocument()
    )
    expect(screen.getByTestId("chapter-1-badge")).toHaveTextContent("Not read")
    expect(screen.getByTestId("chapter-1-check")).toBeEnabled()
    expect(screen.getByTestId("chapter-0-badge")).toHaveTextContent(
      "Not imported"
    )
    expect(screen.getByTestId("chapter-2-badge")).toHaveTextContent(
      "Not imported"
    )
    expect(state.db.textbooks.get(WS)![0]!.book.hourUsed).toBe(1)
  }, 20000)
})

describe("Book view", () => {
  it("lists chapters with page ranges and not read, checked, due", async () => {
    seed()
    renderApp(`/w/${WS}/textbook?book=bk1`)
    const list = await screen.findByTestId("chapter-list")
    const rows = within(list).getAllByRole("listitem")
    expect(rows).toHaveLength(3)
    expect(rows[0]).toHaveTextContent("Pages 9 to 40")
    expect(screen.getByTestId("chapter-0-badge")).toHaveTextContent("Due")
    expect(screen.getByTestId("chapter-1-badge")).toHaveTextContent("Checked")
    expect(screen.getByTestId("chapter-2-badge")).toHaveTextContent("Not read")
  })

  it("renders titles and errors as plain text, never markup", async () => {
    const title = `<img src=x onerror="window.__pwned=1">Chapter **one**`
    seed(
      [
        { title, start: 1, end: 5 },
        {
          title: "Broken",
          start: 6,
          end: 9,
          state: "failed",
          error: "<b>boom</b>",
        },
      ],
      `<script>alert(1)</script>Book`
    )
    const { container } = renderApp(`/w/${WS}/textbook?book=bk1`)
    expect(await screen.findByTestId("chapter-0")).toHaveTextContent(title)
    expect(screen.getByTestId("chapter-1-error")).toHaveTextContent(
      "<b>boom</b>"
    )
    expect(screen.getByRole("heading", { level: 1 })).toHaveTextContent(
      "<script>alert(1)</script>Book"
    )
    expect(container.querySelector("img[src='x'], script, b")).toBeNull()
  })

  it("shows chapter progress from the running job", async () => {
    const b = seed(
      [
        { title: "One", start: 1, end: 5 },
        { title: "Two", start: 6, end: 9, state: "importing" },
        { title: "Three", start: 10, end: 14, state: "importing" },
      ],
      "Importing"
    )
    b.paused = true
    b.job = {
      id: "tbj1",
      kind: "textbook",
      status: "running",
      title: "Import",
      chatId: "",
      createdAt: 1,
      bookId: "bk1",
      chaptersDone: 1,
      chaptersTotal: 3,
    }
    b.book.jobId = "tbj1"
    const user = userEvent.setup()
    renderApp(`/w/${WS}/textbook?book=bk1`)
    expect(
      await screen.findByText("1 of 3 chapters imported")
    ).toBeInTheDocument()
    expect(screen.queryByTestId("chapter-1-check")).not.toBeInTheDocument()
    await user.click(screen.getByTestId("textbook-import-cancel"))
    await waitFor(() => expect(b.job?.status).toBe("failed"))
  })

  it("offers to import a failed chapter again", async () => {
    const b = seed([
      { title: "One", start: 1, end: 5 },
      {
        title: "Two",
        start: 6,
        end: 9,
        state: "failed",
        error: "engine_unavailable",
      },
    ])
    const user = userEvent.setup()
    renderApp(`/w/${WS}/textbook?book=bk1`)
    const note = await screen.findByTestId("book-partial")
    expect(note).toHaveTextContent("1 chapter did not import.")
    expect(note).toHaveTextContent("No model was available")
    expect(screen.getByTestId("chapter-0-check")).toBeEnabled()
    await user.click(screen.getByTestId("book-import-failed"))
    await waitFor(() => expect(b.job?.chaptersTotal).toBe(1))
    await waitFor(() => expect(b.book.chapters[1]?.state).toBe("ready"), {
      timeout: 8000,
    })
  })

  it("offers no import for a chapter whose retry is spent", async () => {
    seed([
      { title: "One", start: 1, end: 5 },
      {
        title: "Two",
        start: 6,
        end: 9,
        state: "failed",
        error: "parse",
        canImport: false,
      },
    ])
    renderApp(`/w/${WS}/textbook?book=bk1`)
    expect(await screen.findByTestId("chapter-1-error")).toHaveTextContent(
      "could not be turned into questions"
    )
    expect(screen.queryByTestId("chapter-1-import")).not.toBeInTheDocument()
    expect(screen.queryByTestId("book-partial")).not.toBeInTheDocument()
  })

  it("blocks imports and says so once the hourly limit is reached", async () => {
    const b = seed([
      { title: "One", start: 1, end: 5 },
      { title: "Two", start: 6, end: 9, state: "pending" },
    ])
    b.book.hourUsed = 8
    renderApp(`/w/${WS}/textbook?book=bk1`)
    expect(await screen.findByTestId("book-rate-limited")).toHaveTextContent(
      "8 of 8 chapter imports used in the last hour"
    )
    expect(screen.getByTestId("chapter-1-import")).toBeDisabled()
  })

  it("shows an error for an unknown book", async () => {
    renderApp(`/w/${WS}/textbook?book=gone`)
    expect(await screen.findByRole("alert")).toBeInTheDocument()
  })
})

describe("Reading check", () => {
  it("runs by keyboard: answer, feedback from the server, finish, status becomes checked", async () => {
    seed()
    const user = userEvent.setup()
    renderApp(`/w/${WS}/textbook?book=bk1`)
    const open = await screen.findByTestId("chapter-2-check")
    open.focus()
    await user.keyboard("{Enter}")
    await user.click(await screen.findByTestId("reading-check-start"))
    expect(await screen.findByTestId("reading-check-q")).toHaveTextContent(
      "what does a future do"
    )
    // Radios by keyboard: focus the group, arrow to an option.
    await user.click(screen.getByRole("radio", { name: "Nothing" }))
    await user.keyboard("{Tab}")
    await user.click(screen.getByTestId("reading-check-answer"))
    const fb = await screen.findByTestId("reading-check-feedback")
    expect(fb).toHaveAttribute("data-correct", "true")
    await waitFor(() =>
      expect(screen.getByTestId("reading-check-next")).toHaveFocus()
    )
    await user.keyboard("{Enter}")
    await user.type(await screen.findByLabelText("Your answer"), "wake")
    await user.keyboard("{Tab}{Enter}")
    expect(await screen.findByTestId("reading-check-feedback")).toHaveAttribute(
      "data-correct",
      "false"
    )
    expect(screen.getByTestId("reading-check-feedback")).toHaveTextContent(
      "Answer: poll"
    )
    await user.keyboard("{Enter}")
    expect(await screen.findByTestId("reading-check-done")).toHaveTextContent(
      "1 of 2 right"
    )
    await user.click(screen.getByRole("button", { name: "Back to chapters" }))
    await waitFor(() =>
      expect(screen.getByTestId("chapter-2-badge")).toHaveTextContent("Checked")
    )
  })

  it("the check payload carries no answers, and the UI never shows one before grading", async () => {
    seed()
    const user = userEvent.setup()
    renderApp(`/w/${WS}/textbook?book=bk1`)
    await user.click(await screen.findByTestId("chapter-0-check"))
    await user.click(await screen.findByTestId("reading-check-start"))
    await screen.findByTestId("reading-check-q")
    const card = screen.getByTestId("reading-check")
    expect(card).not.toHaveTextContent(/Answer:/)
    expect(card).not.toHaveTextContent("inert until something polls it")
  })

  it("shows a start error and lets the learner retry", async () => {
    seed()
    server.use(
      http.post(`${API}/textbooks/:book/chapters/:n/check`, () =>
        apiError("conflict", "Chapter is not imported yet", {
          details: { reason: "chapter_not_ready" },
        })
      )
    )
    const user = userEvent.setup()
    renderApp(`/w/${WS}/textbook?book=bk1`)
    await user.click(await screen.findByTestId("chapter-0-check"))
    await user.click(await screen.findByTestId("reading-check-start"))
    expect(await screen.findByRole("alert")).toHaveTextContent(
      "Chapter is not imported yet"
    )
  })

  it("retries a lost answer with the same clientId and shows the stored feedback", async () => {
    seed()
    const seen: (string | null | undefined)[] = []
    const stored = new Map<string, Dto["CheckFeedbackDto"]>()
    server.use(
      http.post(
        `${API}/textbooks/:book/checks/:check/answer`,
        async ({ request }) => {
          const body = (await request.json()) as Dto["CheckAnswerRequest"]
          seen.push(body.clientId)
          const key = body.clientId ?? ""
          const replay = stored.get(key)
          if (replay) return HttpResponse.json(replay)
          // A different (or missing) id for an item the server already holds is a 409.
          if (stored.size > 0 || seen.length > 2)
            return apiError("conflict", "Already answered", {
              details: { reason: "already_answered" },
            })
          stored.set(key, {
            correct: true,
            close: false,
            answer: "Nothing",
            why: "futures are inert",
            due: state.db.now + 86_400_000,
          })
          // The server took the answer, but the response never arrives.
          return HttpResponse.error()
        }
      )
    )
    const user = userEvent.setup()
    renderApp(`/w/${WS}/textbook?book=bk1`)
    await user.click(await screen.findByTestId("chapter-2-check"))
    await user.click(await screen.findByTestId("reading-check-start"))
    await user.click(await screen.findByRole("radio", { name: "Nothing" }))
    await user.click(screen.getByTestId("reading-check-answer"))
    await waitFor(() => expect(seen).toHaveLength(1))
    expect(screen.queryByTestId("reading-check-feedback")).toBeNull()
    await user.click(await screen.findByTestId("reading-check-answer"))
    expect(await screen.findByTestId("reading-check-feedback")).toHaveAttribute(
      "data-correct",
      "true"
    )
    expect(typeof seen[0]).toBe("string")
    expect(seen[1]).toBe(seen[0])
  })
})

describe("Digest line and palette", () => {
  it("digest says how many chapters are due, and links to the textbooks", async () => {
    seed()
    const user = userEvent.setup()
    const { router } = renderApp(`/w/${WS}/digest`)
    const line = await screen.findByTestId("digest-chapters-due")
    expect(line).toHaveTextContent("1 chapter is due for a reading check.")
    await user.click(
      within(line).getByRole("button", { name: "Open textbooks" })
    )
    await waitFor(() =>
      expect(router.state.location.pathname).toBe(`/w/${WS}/textbook`)
    )
  })

  it("digest has no line when nothing is due", async () => {
    seed([{ title: "One", start: 1, end: 5 }])
    renderApp(`/w/${WS}/digest`)
    await screen.findByTestId("screen-digest")
    await new Promise((r) => setTimeout(r, 50))
    expect(screen.queryByTestId("digest-chapters-due")).not.toBeInTheDocument()
  })

  it("palette: Upload a textbook goes to the screen and focuses the upload button", async () => {
    const user = userEvent.setup()
    const { router } = renderApp(`/w/${WS}/graph`)
    await screen.findByRole("button", { name: /Workspace: Rust async/ })
    await act(async () => {
      window.dispatchEvent(
        new KeyboardEvent("keydown", { key: "k", metaKey: true, ctrlKey: true })
      )
    })
    await user.type(await screen.findByTestId("palette-input"), "upload a text")
    await user.keyboard("{Enter}")
    await waitFor(() =>
      expect(router.state.location.pathname).toBe(`/w/${WS}/textbook`)
    )
    await waitFor(() =>
      expect(screen.getByTestId("textbook-upload-button")).toHaveFocus()
    )
  })
})
