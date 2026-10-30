import type { RequestHandler } from "msw"

/** Shared MSW handlers; the API client task (F10) adds the OpenAPI-derived ones. */
export const handlers: RequestHandler[] = []
