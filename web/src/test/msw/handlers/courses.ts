import { HttpResponse, http } from "msw"

import type { components } from "@/api/schema"

import { nextId, state } from "../db"
import { courseUsage, newCourse, placementItems } from "../fixtures/courses"
import { latency, notFound } from "../respond"

type Dto = components["schemas"]

const API = "*/api/v1/workspaces/:ws/courses"

const find = (ws: unknown, id: unknown) =>
  state.db.courses.get(String(ws))?.find((c) => c.id === id)

const noContent = () => new HttpResponse(null, { status: 204 })

export const courseHandlers = [
  http.get(API, async ({ params }) => {
    await latency()
    const body: Dto["CourseListDto"] = {
      courses: (state.db.courses.get(String(params["ws"])) ?? []).map((c) => ({
        id: c.id,
        title: c.title,
        version: 1,
        updatedAt: state.db.now,
      })),
    }
    return HttpResponse.json(body)
  }),

  http.post(API, async ({ params, request }) => {
    await latency()
    const { goal } = (await request.json()) as Dto["CreateCourseRequest"]
    const course = newCourse(nextId("course"), goal)
    const ws = String(params["ws"])
    state.db.courses.set(ws, [course, ...(state.db.courses.get(ws) ?? [])])
    const body: Dto["CreatedCourseDto"] = {
      id: course.id,
      items: placementItems,
    }
    return HttpResponse.json(body, { status: 201 })
  }),

  http.post(`${API}/:id/placement`, async ({ params, request }) => {
    const course = find(params["ws"], params["id"])
    if (!course) return notFound("Course")
    const { answers } = (await request.json()) as Dto["SubmitPlacementRequest"]
    course.graded = true
    // The mock server grades, like the real one; the client never does.
    const graded: Dto["GradedDto"][] = answers.map((a) => {
      const k = course.key[a.item]
      return {
        item: a.item,
        correct: a.text.toLowerCase() === k?.answer.toLowerCase(),
        answer: k?.answer ?? "",
        why: k?.why ?? "",
      }
    })
    return HttpResponse.json({ graded } satisfies Dto["GradedListDto"])
  }),

  http.get(`${API}/:id/misses`, ({ params }) => {
    const course = find(params["ws"], params["id"])
    if (!course) return notFound("Course")
    return HttpResponse.json({
      misses: Object.values(course.key).map((k) => ({
        q: k.q,
        answer: k.answer,
        why: k.why,
      })),
    } satisfies { misses: Dto["MissDto"][] })
  }),

  http.get(`${API}/:id/status`, ({ params }) => {
    const course = find(params["ws"], params["id"])
    if (!course) return notFound("Course")
    // Mock default: the Author finishes at once. Tests override this to show the wait.
    course.state = "ready"
    return HttpResponse.json({
      state: course.state,
    } satisfies Dto["CourseStatusDto"])
  }),

  http.post(`${API}/:id/retry`, () => noContent()),

  http.post(`${API}/:id/next`, ({ params }) => {
    const course = find(params["ws"], params["id"])
    if (!course) return notFound("Course")
    const step = course.steps[Math.min(course.cursor, course.steps.length - 1)]
    course.cursor += 1
    return HttpResponse.json(step)
  }),

  http.post(`${API}/:id/seen`, () => noContent()),
  http.post(`${API}/:id/self-grade`, () => noContent()),
  http.post(`${API}/:id/flag`, () => noContent()),

  http.post(`${API}/:id/answer`, async ({ request }) => {
    const body = (await request.json()) as Dto["CourseAnswerRequest"]
    const correct = body.text === "Pending or Ready" || body.text === "Pending"
    return HttpResponse.json({
      correct,
      close: false,
      answer: body.item === "q1" ? "Pending or Ready" : "Pending",
      why: "poll says whether the work is finished.",
    })
  }),

  http.post(`${API}/:id/ask`, () =>
    HttpResponse.json({
      reply: "Polling is how an executor checks progress.",
    } satisfies Dto["AskReplyDto"])
  ),
  http.post(`${API}/:id/check`, () =>
    HttpResponse.json({
      correct: true,
      note: "That is the same idea.",
    } satisfies Dto["CheckDto"])
  ),
  http.get(`${API}/:id/usage`, () => HttpResponse.json(courseUsage)),
]
