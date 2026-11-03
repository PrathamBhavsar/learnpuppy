import type { RequestHandler } from "msw"

import { accountHandlers } from "./handlers/accounts"
import { chatHandlers } from "./handlers/chats"
import { courseHandlers } from "./handlers/courses"
import { digestHandlers } from "./handlers/digest"
import { graphHandlers } from "./handlers/graph"
import { ingestHandlers } from "./handlers/ingest"
import { reviewHandlers } from "./handlers/review"
import { settingsHandlers } from "./handlers/settings"
import { systemHandlers } from "./handlers/system"
import { workspaceHandlers } from "./handlers/workspaces"

/**
 * One handler per endpoint in docs/web/03, backed by the in-memory db in ./db.ts. Vitest, the
 * `dev:mock` browser worker and Playwright all share this list. Order matters only where a literal
 * path (`/workspaces/import`) must win over a parameterised one.
 */
export const handlers: RequestHandler[] = [
  ...systemHandlers,
  ...accountHandlers,
  ...settingsHandlers,
  ...workspaceHandlers,
  ...graphHandlers,
  ...chatHandlers,
  ...courseHandlers,
  ...reviewHandlers,
  ...digestHandlers,
  ...ingestHandlers,
]
