/**
 * Contract for the learning-method bout and check-in routes (M8's `routes/learning.rs`, not yet
 * implemented): mirrors `lp_service::learning` (`Clock`, `Refusal`, `Ready`, `SleepSummary`) until
 * the real DTOs land in `openapi/openapi.json`. When they appear in the generated `schema.d.ts`,
 * delete this file and point `learningApi` in `client.ts` at `paths`.
 * Bouts are workspace-wide, not per-course (`brain/learning-method/architecture.md` §3), so every
 * path here takes only `{ws}`. F23's limits and F24's gap pauses are decided entirely server-side;
 * nothing here computes a minute count, a pause schedule or an override rule.
 */

export type BoutStatus =
  | { status: "active"; activeMinutes: number }
  | { status: "bout_limit_reached" }
  | { status: "gap"; remainingMinutes: number }
  | { status: "daily_limit_reached" }

export type Clock = {
  /** The open bout's id, or `null` between bouts. */
  bout: string | null
  status: BoutStatus
  minutesToday: number
  overrideUsedToday: boolean
  /** F24: a blank pause is due right now; cleared by the next heartbeat. */
  gapPauseDue: boolean
  gapPauseSeconds: number
  boutLimitMinutes: number
  dailyLimitMinutes: number
}

/** F03's ready-gate checklist; `focusSeconds` is the time the focus timer actually ran. */
export type ReadyAnswers = {
  phoneAway: boolean
  tabsClosed: boolean
  focusSeconds: number
}
export type StartBoutRequest = { course: string; ready: ReadyAnswers }
export type HeartbeatRequest = { bout: string }
export type OverrideRequest = { reason: string }

export type CheckinNeeded = { needed: boolean }
export type CheckinRequest = { sleepHours: number; note?: string }
/** `reviewOnlySuggested`: self-reported sleep came in under the poor-sleep threshold (F04). */
export type CheckinResult = { reviewOnlySuggested: boolean }

type Ws = { ws: string }
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

export interface LearningPaths {
  "/workspaces/{ws}/learning/clock": Get<Clock>
  "/workspaces/{ws}/learning/bout/start": Call<Clock, StartBoutRequest>
  "/workspaces/{ws}/learning/bout/heartbeat": Call<Clock, HeartbeatRequest>
  "/workspaces/{ws}/learning/bout/end": Call<Clock, HeartbeatRequest>
  "/workspaces/{ws}/learning/bout/focus-lost": Send<HeartbeatRequest>
  "/workspaces/{ws}/learning/bout/override": Send<OverrideRequest>
  "/workspaces/{ws}/learning/checkin/needed": Get<CheckinNeeded>
  "/workspaces/{ws}/learning/checkin": Call<CheckinResult, CheckinRequest>
}
