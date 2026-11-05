import { screen, within } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { HttpResponse, http } from "msw"
import { beforeEach, describe, expect, it } from "vitest"

import { state } from "@/test/msw/db"
import { SECRET_ANSWER, newMethodCourse } from "@/test/msw/fixtures/courses"
import { server } from "@/test/msw/server"
import { renderApp, resetUi } from "@/test/render"

const WS = "ws_rustasync"
const API = "*/api/v1/workspaces/:ws/courses/:id"

beforeEach(() => {
  resetUi()
  state.db.courses.set(WS, [newMethodCourse("m", "Rust async")])
})

describe("Method lesson", () => {
  it("runs a full lesson with every model call refused, and never shows the answer early", async () => {
    let modelCalls = 0
    let stepBodies = ""
    const refuse = () => {
      modelCalls += 1
      return HttpResponse.json(
        { error: { code: "engine_unavailable", message: "No AI" } },
        { status: 503 }
      )
    }
    server.use(
      http.post(`${API}/ask`, refuse),
      http.post(`${API}/check`, refuse),
      http.post(`${API}/step`, async ({ params }) => {
        const course = state.db.courses.get(WS)?.[0]
        const m = course?.method
        if (!m || params["id"] !== "m")
          return new HttpResponse(null, { status: 404 })
        const body = JSON.stringify(
          m.steps[Math.min(m.cursor, m.steps.length - 1)]
        )
        stepBodies += body
        return new HttpResponse(body, {
          headers: { "content-type": "application/json" },
        })
      })
    )
    const user = userEvent.setup()
    renderApp(`/w/${WS}/course?id=m`)
    await user.click(
      await screen.findByRole("button", { name: "Start lesson" })
    )

    // Plan map, then confirm.
    const plan = await screen.findByTestId("course-plan")
    expect(plan).toHaveTextContent("Builds on: Futures")
    await user.click(within(plan).getByRole("button", { name: /begin/ }))

    // Predict.
    const predict = await screen.findByTestId("step-predict")
    await user.click(within(predict).getByLabelText("A status"))
    await user.click(within(predict).getByRole("button", { name: /Show/ }))

    // Explain.
    await user.click(await screen.findByRole("button", { name: "Got it" }))

    // Notes gate.
    const notes = await screen.findByTestId("step-notes")
    await user.type(
      within(notes).getByLabelText("Your notes"),
      "poll gives a status"
    )
    await user.click(within(notes).getByRole("button", { name: /Start/ }))

    // Quiz with confidence.
    const quiz = await screen.findByTestId("step-quiz")
    expect(within(quiz).getByRole("button", { name: "Answer" })).toBeDisabled()
    await user.click(within(quiz).getByLabelText("poll returns a status"))
    await user.click(within(quiz).getByLabelText("Fairly sure"))
    await user.type(
      within(quiz).getByLabelText("Why do you think so?"),
      "it said so"
    )
    await user.click(within(quiz).getByRole("button", { name: "Answer" }))
    await user.click(await screen.findByRole("button", { name: "Continue" }))

    // Recall: the answer is nowhere before grading, and a retry shows its angle.
    const recall = await screen.findByTestId("step-recall")
    expect(screen.getByTestId("step-angle")).toHaveTextContent("kettle")
    expect(document.body.textContent).not.toContain(SECRET_ANSWER)
    expect(stepBodies).not.toContain(SECRET_ANSWER)
    await user.click(within(recall).getByRole("button", { name: /know/ }))
    await user.click(await screen.findByRole("button", { name: "Continue" }))

    // Feynman, break, close.
    const feynman = await screen.findByTestId("step-feynman")
    await user.type(
      within(feynman).getByLabelText("Your explanation"),
      "A future reports status"
    )
    await user.click(within(feynman).getByRole("button", { name: "Submit" }))
    expect(await screen.findByTestId("step-feynman-note")).toHaveTextContent(
      "graded later"
    )
    await user.click(screen.getByRole("button", { name: "Continue" }))
    await user.click(await screen.findByRole("button", { name: /back/ }))
    expect(await screen.findByTestId("course-close")).toHaveTextContent(
      "1 of 2 facts solid"
    )
    expect(modelCalls).toBe(0)
  })

  it("sends a gap as 'I don't know' with no text or confidence", async () => {
    let sent: unknown
    state.db.courses.get(WS)![0]!.method!.cursor = 4
    server.use(
      http.post(`${API}/quiz-answer`, async ({ request }) => {
        sent = await request.json()
        return HttpResponse.json({ status: "graded", correct: false })
      })
    )
    const user = userEvent.setup()
    renderApp(`/w/${WS}/course?id=m`)
    await user.click(
      await screen.findByRole("button", { name: "Start lesson" })
    )
    const quiz = await screen.findByTestId("step-quiz")
    await user.click(within(quiz).getByRole("button", { name: /know/ }))
    await screen.findByTestId("step-feedback")
    expect(sent).toMatchObject({ item: "m1", gap: true, text: "" })
  })

  it("shows the misconception register and the server's mastery line", async () => {
    const user = userEvent.setup()
    renderApp(`/w/${WS}/course?id=m`)
    await user.click(
      await screen.findByRole("button", { name: "Start lesson" })
    )
    expect(await screen.findByTestId("course-mastery")).toHaveTextContent(
      "0 of 2 facts solid"
    )
    await user.click(screen.getByRole("button", { name: "My misconceptions" }))
    expect(await screen.findByTestId("course-register")).toHaveTextContent(
      "poll blocks"
    )
  })

  it("keeps the step on screen when the server refuses an action with a gate", async () => {
    server.use(
      http.post(`${API}/plan/confirm`, () =>
        HttpResponse.json(
          {
            error: {
              code: "conflict",
              message: "Not yet",
              details: { gate: "plan" },
            },
          },
          { status: 409 }
        )
      )
    )
    const user = userEvent.setup()
    renderApp(`/w/${WS}/course?id=m`)
    await user.click(
      await screen.findByRole("button", { name: "Start lesson" })
    )
    await user.click(await screen.findByRole("button", { name: /begin/ }))
    expect(await screen.findByTestId("course-plan")).toBeInTheDocument()
  })
})
