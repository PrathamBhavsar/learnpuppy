/**
 * Contract for the learning-method course routes (M8c): mirrors `dto/course.rs` (`MethodStepDto` and the
 * request/response DTOs) until M8 lands them in `openapi/openapi.json`. When they appear in the generated
 * `schema.d.ts`, delete this file and point `methodApi` in `client.ts` at `paths`.
 * The server decides every step and every grade; nothing here carries an answer, a `why` or a rubric.
 */

type LessonKind = "conceptual" | "procedural" | "definitional"
type MethodAskKind = "review" | "new" | "retry" | "mixed"
type MethodItemKind = "mcq" | "recall"

type PlanLesson = {
  id: string
  title: string
  kind: LessonKind
  dependsOn: string[]
}

export type CoursePlan = { confirmed: boolean; lessons: PlanLesson[] }

export type MethodItem = {
  id: string
  lesson: string
  kind: MethodItemKind
  q: string
  options: string[]
  ask: MethodAskKind
  /** An alternate explanation to show ahead of a retry. */
  angle?: string | null
}

export type MethodStep =
  | { step: "confirm_plan" }
  | {
      step: "predict"
      lesson: string
      title: string
      q: string
      options: string[]
    }
  | {
      step: "explain"
      id: string
      title: string
      hook: string
      explain: string
      keyFacts: string[]
      /** The faded-example text at the current rung; procedural lessons only. */
      rungText?: string | null
    }
  | { step: "notes"; lesson: string; title: string }
  | ({ step: "ask" } & MethodItem)
  | { step: "break"; lesson: string; title: string }
  | { step: "feynman"; lesson: string; title: string; keyFacts: string[] }
  | { step: "close" }

type PredictRequest = { lesson: string; guess: string }
type LessonRef = { lesson: string }
type NoteRequest = { lesson: string; text: string }
export type InsertPrereqRequest = {
  before: string
  missing: string
  visited?: string[]
}
export type MethodAnswerRequest = {
  item: string
  lesson: string
  text: string
  confidence?: number | null
  ms?: number | null
  reasoning: string
  /** "I don't know": a gap, never a miss. */
  gap: boolean
}
export type MethodAnswerResult =
  { status: "graded"; correct: boolean } | { status: "pending" }
export type FeynmanRequest = { lesson: string; explanation: string }
export type FeynmanResult = { correct: boolean; note: string }

export type CloseReport = {
  takeaways: string[]
  focus: string[]
  countLine: string
  sureWrong: number
  sureTotal: number
  resume?: string | null
}

export type RegisterRow = {
  item: string
  belief: string
  status: "open" | "retested-ok" | "recurring"
  misses: number
}

export type MasteryCount = { countLine: string }

type Ws = { ws: string; id: string }
type Res<T> = {
  200: { headers: Record<string, unknown>; content: { "application/json": T } }
}
type Done = { 204: { headers: Record<string, unknown>; content?: never } }
type Params = { query?: never; header?: never; cookie?: never; path: Ws }
type Bodyless<R> = {
  parameters: Params
  requestBody?: never
  responses: R
}
type WithBody<R, B> = {
  parameters: Params
  requestBody: { content: { "application/json": B } }
  responses: R
}
type NoMethods = {
  get?: never
  put?: never
  post?: never
  delete?: never
  patch?: never
  options?: never
  head?: never
  trace?: never
}
type Path<M extends object> = {
  parameters: { query?: never; header?: never; path?: never; cookie?: never }
} & Omit<NoMethods, keyof M> &
  M

type Get<R> = Path<{ get: Bodyless<Res<R>> }>
type Send<B> = Path<{ post: WithBody<Done, B> }>
type Call<R, B> = Path<{ post: WithBody<Res<R>, B> }>

export interface MethodPaths {
  "/workspaces/{ws}/courses/{id}/plan": Get<CoursePlan>
  "/workspaces/{ws}/courses/{id}/plan/confirm": Path<{ post: Bodyless<Done> }>
  "/workspaces/{ws}/courses/{id}/plan/prereq": Call<
    CoursePlan,
    InsertPrereqRequest
  >
  "/workspaces/{ws}/courses/{id}/step": Path<{
    post: Bodyless<Res<MethodStep>>
  }>
  "/workspaces/{ws}/courses/{id}/predict": Send<PredictRequest>
  "/workspaces/{ws}/courses/{id}/explain-seen": Send<LessonRef>
  "/workspaces/{ws}/courses/{id}/note": Send<NoteRequest>
  "/workspaces/{ws}/courses/{id}/quiz-answer": Call<
    MethodAnswerResult,
    MethodAnswerRequest
  >
  "/workspaces/{ws}/courses/{id}/feynman": Call<FeynmanResult, FeynmanRequest>
  "/workspaces/{ws}/courses/{id}/break": Send<LessonRef>
  "/workspaces/{ws}/courses/{id}/close": Path<{
    post: Bodyless<Res<CloseReport>>
  }>
  "/workspaces/{ws}/courses/{id}/register": Get<{ rows: RegisterRow[] }>
  "/workspaces/{ws}/courses/{id}/mastery": Get<MasteryCount>
}
