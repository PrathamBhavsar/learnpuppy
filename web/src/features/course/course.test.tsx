import { screen, waitFor, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, delay, http } from "msw"
import { beforeEach, describe, expect, it } from "vitest"

import { state } from "@/test/msw/db"
import { newCourse } from "@/test/msw/fixtures/courses"
import { apiError } from "@/test/msw/respond"
import { server } from "@/test/msw/server"
import { renderApp, resetUi } from "@/test/render"

const WS = "ws_rustasync"
const API = "*/api/v1/workspaces/:ws/courses"

beforeEach(() => {
  resetUi()
})

const seed = (placed = true) =>
  state.db.courses.set(WS, [
    { ...newCourse("c1", "Rust async"), graded: placed },
  ])

describe("Course screen", () => {
  it("shows the questions when a course with unsubmitted placement is opened by URL", async () => {
    seed(false)
    renderApp(`/w/${WS}/course?id=c1`)
    const form = await screen.findByTestId("course-placement")
    expect(form).toHaveTextContent("What does `await` do?")
    expect(screen.queryByText(/could not be written/)).not.toBeInTheDocument()
  })

  it("sends one request for two rapid clicks on a grade", async () => {
    seed()
    let grades = 0
    server.use(
      http.post(`${API}/:id/self-grade`, async () => {
        grades += 1
        await delay(50)
        return new HttpResponse(null, { status: 204 })
      })
    )
    const user = userEvent.setup()
    renderApp(`/w/${WS}/course?id=c1`)
    await user.click(
      await screen.findByRole("button", { name: "Start lesson" })
    )
    await user.click(await screen.findByRole("button", { name: "Got it" }))
    const q1 = await screen.findByTestId("course-question")
    await user.click(within(q1).getByLabelText("Pending or Ready"))
    await user.click(within(q1).getByRole("button", { name: "Answer" }))
    const good = await screen.findByRole("button", { name: "good" })
    await user.dblClick(good)
    await screen.findByText(/Name the value poll returns/)
    expect(grades).toBe(1)
  })

  it("creates a course from a goal, runs placement, then shows the misses", async () => {
    const user = userEvent.setup()
    renderApp(`/w/${WS}/course`)
    await user.type(await screen.findByLabelText("Course goal"), "Rust async")
    await user.click(screen.getByRole("button", { name: "Start course" }))
    const form = await screen.findByTestId("course-placement")
    await user.click(within(form).getByLabelText("Blocks the thread"))
    await user.type(within(form).getByLabelText("Your answer"), "Stream")
    await user.click(
      within(form).getByRole("button", { name: "Check my answers" })
    )
    // Both wrong: the wait is filled with what was missed and why.
    const misses = await screen.findByTestId("course-misses")
    expect(misses).toHaveTextContent("Anything you can poll implements")
    expect(screen.getByText("You got 0 of 2 right.")).toBeInTheDocument()
    await user.click(
      await screen.findByRole("button", { name: "Start lesson" })
    )
    expect(await screen.findByTestId("course-explain")).toBeInTheDocument()
  })

  it("shows the misses while the course is Writing, then offers the lesson", async () => {
    seed()
    let polls = 0
    server.use(
      http.get(`${API}/:id/status`, () => {
        polls += 1
        return HttpResponse.json({ state: polls < 2 ? "writing" : "ready" })
      })
    )
    renderApp(`/w/${WS}/course?id=c1`)
    expect(await screen.findByText(/Writing your lessons/)).toBeInTheDocument()
    expect(await screen.findByTestId("course-misses")).toHaveTextContent(
      "Yields until the future is ready"
    )
    expect(
      await screen.findByRole(
        "button",
        { name: "Start lesson" },
        { timeout: 5000 }
      )
    ).toBeInTheDocument()
  }, 10_000)

  it("offers a retry when writing failed", async () => {
    seed()
    server.use(
      http.get(`${API}/:id/status`, () =>
        HttpResponse.json({ state: "failed", reason: "model refused" })
      )
    )
    renderApp(`/w/${WS}/course?id=c1`)
    expect(await screen.findByText(/model refused/)).toBeInTheDocument()
    expect(
      screen.getByRole("button", { name: "Try again" })
    ).toBeInTheDocument()
  })

  it("finishes a lesson with every model-backed call refused (stored steps need no model)", async () => {
    seed()
    const refuse = () =>
      apiError("engine_unavailable", "All your AIs are limited")
    server.use(
      http.post(API, refuse),
      http.post(`${API}/:id/ask`, refuse),
      http.post(`${API}/:id/check`, refuse),
      http.post(`${API}/:id/retry`, refuse)
    )
    const user = userEvent.setup()
    renderApp(`/w/${WS}/course?id=c1`)
    await user.click(
      await screen.findByRole("button", { name: "Start lesson" })
    )

    // explain -> seen
    expect(await screen.findByTestId("course-explain")).toHaveTextContent(
      "Polling"
    )
    await user.click(screen.getByRole("button", { name: "Got it" }))

    // ask (mcq) -> answer -> feedback with answer and why -> continue
    const q1 = await screen.findByTestId("course-question")
    await user.click(within(q1).getByLabelText("Pending or Ready"))
    await user.click(within(q1).getByRole("button", { name: "Answer" }))
    const feedback = await screen.findByTestId("course-feedback")
    expect(feedback).toHaveTextContent("Correct")
    expect(feedback).toHaveTextContent(
      "poll says whether the work is finished."
    )
    await user.click(within(feedback).getByRole("button", { name: "Continue" }))

    // ask (recall) -> answer -> self-grade moves on
    const q2 = await screen.findByText(/Name the value poll returns/)
    const question = q2.closest("[data-testid=course-question]") as HTMLElement
    await user.type(within(question).getByLabelText("Your answer"), "Pending")
    await user.click(within(question).getByRole("button", { name: "Answer" }))
    await user.click(await screen.findByRole("button", { name: "good" }))

    // done -> summary with usage
    expect(await screen.findByTestId("course-done")).toBeInTheDocument()
    expect(screen.getByTestId("course-usage")).toHaveTextContent(
      "strong 1200 in"
    )
  })

  it("keeps the lesson usable when ask is refused", async () => {
    seed()
    server.use(
      http.post(`${API}/:id/ask`, () =>
        apiError("engine_unavailable", "All your AIs are limited")
      )
    )
    const user = userEvent.setup()
    renderApp(`/w/${WS}/course?id=c1`)
    await user.click(
      await screen.findByRole("button", { name: "Start lesson" })
    )
    await screen.findByTestId("course-explain")
    await user.type(screen.getByLabelText("Ask a question"), "why?")
    await user.click(screen.getByRole("button", { name: "Ask" }))
    await waitFor(() =>
      expect(screen.getByLabelText("Ask a question")).toHaveValue("why?")
    )
    await user.click(screen.getByRole("button", { name: "Got it" }))
    expect(await screen.findByTestId("course-question")).toBeInTheDocument()
  })

  it("flags a question that looks wrong", async () => {
    seed()
    let flagged: unknown
    server.use(
      http.post(`${API}/:id/flag`, async ({ request }) => {
        flagged = await request.json()
        return new HttpResponse(null, { status: 204 })
      })
    )
    const user = userEvent.setup()
    renderApp(`/w/${WS}/course?id=c1`)
    await user.click(
      await screen.findByRole("button", { name: "Start lesson" })
    )
    await user.click(await screen.findByRole("button", { name: "Got it" }))
    const q1 = await screen.findByTestId("course-question")
    await user.click(within(q1).getByLabelText("Ok or Err"))
    await user.click(within(q1).getByRole("button", { name: "Answer" }))
    await user.click(
      await screen.findByRole("button", { name: "This looks wrong" })
    )
    await user.type(screen.getByLabelText("What looks wrong?"), "bad options")
    await user.click(screen.getByRole("button", { name: "Send" }))
    await waitFor(() =>
      expect(flagged).toEqual({ item: "q1", reason: "bad options" })
    )
  })
})
