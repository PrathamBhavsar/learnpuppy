import { HttpResponse, http } from "msw"
import { describe, expect, it } from "vitest"

import { server } from "@/test/msw/server"

import { api, newClientId, pendingApi } from "./client"
import { ApiError, shouldRetry } from "./errors"
import { createQueryClient } from "./query-client"
import { useStatus } from "@/stores/status"

const envelope = (code: string, message: string, extra: object = {}) => ({
  error: { code, message, requestId: "req_1", retryable: false, ...extra },
})

async function failure(promise: Promise<unknown>): Promise<ApiError> {
  try {
    await promise
  } catch (error) {
    if (error instanceof ApiError) return error
    throw error
  }
  throw new Error("expected the call to reject")
}

describe("api client", () => {
  it("sends the CSRF header and same-origin credentials", async () => {
    let seen: Request | undefined
    server.use(
      http.get("*/api/v1/health", ({ request }) => {
        seen = request
        return HttpResponse.json({ ok: true })
      })
    )
    await api.GET("/health")
    expect(seen?.headers.get("x-lp-csrf")).toBe("1")
    expect(seen?.credentials).toBe("same-origin")
  })

  it("maps the error envelope to a typed ApiError", async () => {
    server.use(
      http.get("*/api/v1/accounts", () =>
        HttpResponse.json(
          envelope("validation", "Bad input", {
            details: { fields: [{ path: "name", message: "Required" }] },
            retryable: false,
          }),
          { status: 422 }
        )
      )
    )
    const error = await failure(pendingApi.GET("/accounts"))
    expect(error).toMatchObject({
      code: "validation",
      message: "Bad input",
      status: 422,
      requestId: "req_1",
      retryable: false,
      fields: [{ path: "name", message: "Required" }],
    })
    expect(error.describe()).toContain("requestId: req_1")
  })

  it("reads retry-after-ms for rate limits", async () => {
    server.use(
      http.get("*/api/v1/accounts", () =>
        HttpResponse.json(
          envelope("rate_limited", "Slow down", {
            retryable: true,
            details: { retryAfterMs: 1500 },
          }),
          { status: 429 }
        )
      )
    )
    const error = await failure(pendingApi.GET("/accounts"))
    expect(error.retryAfterMs).toBe(1500)
    expect(error.retryable).toBe(true)
  })

  it("falls back to the x-request-id header and the status text for non-envelope bodies", async () => {
    server.use(
      http.get(
        "*/api/v1/accounts",
        () =>
          new HttpResponse("<html>bad gateway</html>", {
            status: 502,
            statusText: "Bad Gateway",
            headers: { "x-request-id": "req_hdr" },
          })
      )
    )
    const error = await failure(pendingApi.GET("/accounts"))
    expect(error).toMatchObject({
      code: "unknown",
      message: "Bad Gateway",
      requestId: "req_hdr",
      retryable: true,
    })
  })

  it("treats a bare 401 as auth_required", async () => {
    server.use(
      http.get(
        "*/api/v1/accounts",
        () => new HttpResponse(null, { status: 401 })
      )
    )
    expect((await failure(pendingApi.GET("/accounts"))).code).toBe(
      "auth_required"
    )
  })

  it("reports an unreachable server as a retryable network error", async () => {
    server.use(http.get("*/api/v1/accounts", () => HttpResponse.error()))
    const error = await failure(pendingApi.GET("/accounts"))
    expect(error.code).toBe("network")
    expect(error.retryable).toBe(true)
  })

  it("generates unique client ids", () => {
    expect(newClientId()).not.toBe(newClientId())
  })
})

describe("retry policy and global failures", () => {
  it("retries only retryable errors, at most three times", () => {
    const transient = new ApiError({
      code: "network",
      message: "x",
      status: 0,
      retryable: true,
    })
    const fatal = new ApiError({ code: "not_found", message: "x", status: 404 })
    expect(shouldRetry(0, transient)).toBe(true)
    expect(shouldRetry(3, transient)).toBe(false)
    expect(shouldRetry(0, fatal)).toBe(false)
  })

  it("raises the auth and engine flags from failed queries", async () => {
    const qc = createQueryClient()
    server.use(
      http.get("*/api/v1/accounts", () =>
        HttpResponse.json(envelope("auth_required", "Sign in"), { status: 401 })
      ),
      http.get("*/api/v1/settings", () =>
        HttpResponse.json(envelope("engine_unavailable", "No engine"), {
          status: 503,
        })
      )
    )
    await qc
      .fetchQuery({
        queryKey: ["a"],
        retry: false,
        queryFn: async () => (await pendingApi.GET("/accounts")).data,
      })
      .catch(() => undefined)
    expect(useStatus.getState().authRequired).toBe(true)
    await qc
      .fetchQuery({
        queryKey: ["b"],
        retry: false,
        queryFn: async () => (await pendingApi.GET("/settings")).data,
      })
      .catch(() => undefined)
    expect(useStatus.getState().engineUnavailable).toBe(true)
  })
})
