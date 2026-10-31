import createClient, { type Middleware } from "openapi-fetch"

import { errorFromResponse, toApiError } from "./errors"
import type { PendingPaths } from "./pending-types"
import type { paths } from "./schema"

export const API_PREFIX = "/api/v1"

function baseUrl(): string {
  return typeof location === "undefined"
    ? API_PREFIX
    : location.origin + API_PREFIX
}

/** Maps non-2xx envelopes and network failures to `ApiError`; calls reject instead of returning `error`. */
const errorMiddleware: Middleware = {
  async onResponse({ response }) {
    if (!response.ok) throw await errorFromResponse(response)
    return undefined
  },
  onError({ error }) {
    return toApiError(error)
  },
}

/**
 * Conditional GET (ETag / If-None-Match) is the browser HTTP cache's job: the server sends validators on
 * graph, review stats and accounts, and `fetch` revalidates transparently, so a 304 never reaches here.
 */
function make<P extends object>() {
  const client = createClient<P>({
    baseUrl: baseUrl(),
    credentials: "same-origin",
    headers: { "X-LP-CSRF": "1" },
    // Resolve `fetch` per call (not at import) so test doubles installed later are honoured.
    fetch: (request) => globalThis.fetch(request),
  })
  client.use(errorMiddleware)
  return client
}

/** Endpoints present in the generated OpenAPI schema. */
export const api = make<paths>()

/** Endpoints from docs/web/03 that `openapi.json` does not contain yet (see pending-types.ts). */
export const pendingApi = make<PendingPaths>()

/** Result of a call that either has a body or rejected with `ApiError`. */
export function unwrap<T>(result: { data?: T | undefined }): T {
  if (result.data === undefined)
    throw toApiError(new Error("Empty response body"))
  return result.data
}

export function newClientId(): string {
  return globalThis.crypto.randomUUID()
}
