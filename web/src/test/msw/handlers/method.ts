import { HttpResponse, http } from "msw"

import type {
  CloseReport,
  CoursePlan,
  FeynmanRequest,
  FeynmanResult,
  MasteryCount,
  MethodAnswerRequest,
  MethodAnswerResult,
  MethodStep,
  RegisterRow,
  ReviewDate,
} from "@/api/method-types"

import { state } from "../db"
import { SECRET_ANSWER, methodPlan } from "../fixtures/courses"
import { notFound } from "../respond"

const API = "*/api/v1/workspaces/:ws/courses/:id"

const find = (ws: unknown, id: unknown) =>
  state.db.courses.get(String(ws))?.find((c) => c.id === id)

const noContent = () => new HttpResponse(null, { status: 204 })

/** Each learner action moves the mock one stored step on; the real server's gate decides this. */
function advancing(path: string) {
  return http.post(`${API}/${path}`, ({ params }) => {
    const course = find(params["ws"], params["id"])
    if (!course?.method) return notFound("Course")
    course.method.cursor += 1
    return noContent()
  })
}

export const methodHandlers = [
  // A stored-sequence course has no plan: the lesson screen falls back to the old loop.
  http.get(`${API}/plan`, ({ params }) => {
    const course = find(params["ws"], params["id"])
    if (!course?.method) return notFound("Plan")
    const plan: CoursePlan = {
      ...methodPlan,
      confirmed: course.method.cursor > 0,
    }
    return HttpResponse.json(plan)
  }),

  http.post(`${API}/step`, ({ params }) => {
    const course = find(params["ws"], params["id"])
    if (!course?.method) return notFound("Course")
    const { steps, cursor } = course.method
    const step: MethodStep = steps[Math.min(cursor, steps.length - 1)]!
    return HttpResponse.json(step)
  }),

  advancing("plan/confirm"),
  advancing("predict"),
  advancing("explain-seen"),
  advancing("note"),
  advancing("break"),

  http.post(`${API}/quiz-answer`, async ({ params, request }) => {
    const course = find(params["ws"], params["id"])
    if (!course?.method) return notFound("Course")
    const body = (await request.json()) as MethodAnswerRequest
    course.method.cursor += 1
    const result: MethodAnswerResult =
      body.item === "m2" && !body.gap
        ? body.text === SECRET_ANSWER
          ? { status: "graded", correct: true }
          : { status: "pending" }
        : { status: "graded", correct: false }
    return HttpResponse.json(result)
  }),

  http.post(`${API}/feynman`, async ({ params, request }) => {
    const course = find(params["ws"], params["id"])
    if (!course?.method) return notFound("Course")
    ;(await request.json()) as FeynmanRequest
    course.method.cursor += 1
    const result: FeynmanResult = {
      correct: true,
      note: "Saved, graded later.",
    }
    return HttpResponse.json(result)
  }),

  http.post(`${API}/close`, () => {
    const report: CloseReport = {
      takeaways: ["poll returns a status"],
      focus: [],
      countLine: "1 of 2 facts solid",
      sureWrong: 0,
      sureTotal: 1,
      resume: null,
    }
    return HttpResponse.json(report)
  }),

  http.post(`${API}/braindump`, async ({ request }) => {
    const { text } = (await request.json()) as { text: string }
    const missed = text.includes("status") ? [] : ["poll returns a status"]
    return HttpResponse.json({ missed })
  }),

  http.get(`${API}/review-dates`, () => {
    const dates: ReviewDate[] = [
      {
        cardId: "c:m:m1",
        label: "poll returns a status",
        due: Date.now() + 20 * 3_600_000,
      },
    ]
    return HttpResponse.json({ dates })
  }),

  http.post(`${API}/review-dates`, () => noContent()),

  http.get(`${API}/mastery`, () => {
    const mastery: MasteryCount = { countLine: "0 of 2 facts solid" }
    return HttpResponse.json(mastery)
  }),

  http.get(`${API}/register`, () => {
    const rows: RegisterRow[] = [
      { item: "m1", belief: "poll blocks", status: "open", misses: 2 },
    ]
    return HttpResponse.json({ rows })
  }),
]
