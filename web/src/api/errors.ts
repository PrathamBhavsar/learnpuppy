import type { ErrorCode } from "./pending-types"

export type FieldError = { path: string; message: string }

export type ApiErrorCode = ErrorCode | "network" | "unknown"

export type ApiErrorInit = {
  code: ApiErrorCode
  message: string
  status: number
  requestId?: string
  retryable?: boolean
  details?: Record<string, unknown> | null
}

/** Every failed API call surfaces as this, parsed from the server's error envelope (docs/web/03). */
export class ApiError extends Error {
  readonly code: ApiErrorCode
  readonly status: number
  readonly requestId: string | undefined
  readonly retryable: boolean
  readonly details: Record<string, unknown> | null
  readonly fields: FieldError[]
  /** Set for `rate_limited`. */
  readonly retryAfterMs: number | undefined

  constructor(init: ApiErrorInit) {
    super(init.message)
    this.name = "ApiError"
    this.code = init.code
    this.status = init.status
    this.requestId = init.requestId
    this.retryable = init.retryable ?? false
    this.details = init.details ?? null
    this.fields = readFields(this.details)
    const retryAfter = this.details?.["retryAfterMs"]
    this.retryAfterMs = typeof retryAfter === "number" ? retryAfter : undefined
  }

  /** Multi-line text for the "Copy details" toast action and bug reports. */
  describe(): string {
    return [
      `${this.code}: ${this.message}`,
      this.requestId ? `requestId: ${this.requestId}` : null,
      `status: ${this.status}`,
    ]
      .filter(Boolean)
      .join("\n")
  }
}

function readFields(details: Record<string, unknown> | null): FieldError[] {
  const raw = details?.["fields"]
  if (!Array.isArray(raw)) return []
  return raw.flatMap((entry: unknown) => {
    if (typeof entry !== "object" || entry === null) return []
    const { path, message } = entry as Record<string, unknown>
    return typeof path === "string" && typeof message === "string"
      ? [{ path, message }]
      : []
  })
}

/** Network failures (offline, server down) are retried; so is whatever the server marks retryable. */
export function toApiError(value: unknown): ApiError {
  if (value instanceof ApiError) return value
  if (value instanceof DOMException && value.name === "AbortError") {
    return new ApiError({
      code: "unknown",
      message: "Request aborted",
      status: 0,
    })
  }
  if (value instanceof TypeError) {
    return new ApiError({
      code: "network",
      message: "Could not reach the LearnPuppy server.",
      status: 0,
      retryable: true,
    })
  }
  return new ApiError({
    code: "unknown",
    message: value instanceof Error ? value.message : "Unexpected error",
    status: 0,
  })
}

type Envelope = {
  error?: {
    code?: string
    message?: string
    retryable?: boolean
    requestId?: string
    details?: Record<string, unknown> | null
  }
}

/** Builds an ApiError from a non-2xx response; tolerates bodies that are not an envelope. */
export async function errorFromResponse(response: Response): Promise<ApiError> {
  const requestId = response.headers.get("x-request-id") ?? undefined
  const envelope = await response
    .clone()
    .json()
    .then((json) => json as Envelope | null)
    .catch(() => null)
  const body = envelope?.error
  if (body?.code) {
    return new ApiError({
      code: body.code as ApiErrorCode,
      message: body.message ?? response.statusText,
      status: response.status,
      requestId: body.requestId ?? requestId,
      retryable: body.retryable ?? false,
      details: body.details ?? null,
    })
  }
  return new ApiError({
    code: response.status === 401 ? "auth_required" : "unknown",
    message: response.statusText || `HTTP ${response.status}`,
    status: response.status,
    requestId,
    retryable: response.status >= 502 && response.status <= 504,
  })
}

/** TanStack Query `retry`: transient failures only, at most 3 attempts. */
export function shouldRetry(count: number, error: unknown): boolean {
  return count < 3 && toApiError(error).retryable
}
