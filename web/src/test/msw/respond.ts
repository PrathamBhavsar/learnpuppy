import { HttpResponse, delay } from "msw"

import type { ErrorCode } from "@/api/pending-types"

import { state } from "./db"

const STATUS: Record<ErrorCode, number> = {
  not_found: 404,
  validation: 400,
  conflict: 409,
  turn_in_progress: 409,
  stale_question: 409,
  workspace_archived: 409,
  storage_full: 507,
  engine_unavailable: 503,
  rate_limited: 429,
  auth_required: 401,
  forbidden: 403,
  bad_host: 421,
  method_not_allowed: 405,
  internal: 500,
}

let requestCounter = 0

/** The error envelope from docs/web/03 §Conventions. */
export function apiError(
  code: ErrorCode,
  message: string,
  options: { details?: Record<string, unknown>; retryable?: boolean } = {}
) {
  requestCounter += 1
  return HttpResponse.json(
    {
      error: {
        code,
        message,
        details: options.details ?? null,
        retryable: options.retryable ?? false,
        requestId: `req_mock_${requestCounter}`,
      },
    },
    { status: STATUS[code] }
  )
}

export const notFound = (what: string) =>
  apiError("not_found", `${what} was not found`)

export async function latency(): Promise<void> {
  if (state.db.latencyMs > 0) await delay(state.db.latencyMs)
}

/** Replays the stored result for a repeated `clientId`. */
export function once<T>(clientId: string | undefined, compute: () => T): T {
  if (!clientId) return compute()
  const key = `once:${clientId}`
  if (state.db.idempotent.has(key)) return state.db.idempotent.get(key) as T
  const value = compute()
  state.db.idempotent.set(key, value)
  return value
}
