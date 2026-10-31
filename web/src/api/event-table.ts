import type { QueryClient } from "@tanstack/react-query"

import { useLive } from "@/stores/live"

import { keys } from "./keys"
import type { components } from "./schema"

type Schemas = components["schemas"]
type ServerEvent = Schemas["ServerEvent"]

/** Event name → payload, derived from the generated `ServerEvent` union. */
export type EventMap = {
  [E in ServerEvent as E["type"]]: E["data"]
}
export type EventName = keyof EventMap

export const EVENT_NAMES = [
  "hello",
  "workspace.created",
  "workspace.updated",
  "workspace.deleted",
  "accounts.changed",
  "graph.changed",
  "chat.created",
  "chat.updated",
  "chat.delta",
  "chat.toolCall",
  "chat.rest",
  "chat.error",
  "job.updated",
  "ingest.progress",
  "ingest.done",
  "review.changed",
  "export.ready",
  "resync",
] as const satisfies readonly EventName[]

/** Side effects that need UI (navigation, toasts); wired by the shell, absent in tests. */
export type UiHooks = {
  workspaceDeleted?: (workspaceId: string) => void
  exportReady?: (data: EventMap["export.ready"]) => void
  ingestDone?: (data: EventMap["ingest.done"]) => void
}

export type EventContext = {
  queryClient: QueryClient
  /** Workspace the stream is scoped to; workspace-scoped events carry no id of their own. */
  workspaceId: string | null
  ui?: UiHooks
}

/**
 * The docs/web/03 event → query table. Returns nothing; every branch invalidates or updates
 * transient state so a missed event only costs freshness until the next resync.
 */
export function applyEvent<N extends EventName>(
  name: N,
  data: EventMap[N],
  ctx: EventContext
): void {
  const { queryClient: qc, workspaceId: ws, ui } = ctx
  const inWs = (fn: (id: string) => void) => {
    if (ws) fn(ws)
  }
  // The union above makes `data` precise per case.
  const payload = data as never

  switch (name) {
    case "hello":
      return
    case "workspace.created":
    case "workspace.updated": {
      void qc.invalidateQueries({ queryKey: keys.workspacesAll() })
      void qc.invalidateQueries({ queryKey: keys.bootstrap() })
      const id = (payload as EventMap["workspace.updated"]).workspaceId
      void qc.invalidateQueries({ queryKey: keys.ws(id).detail() })
      return
    }
    case "workspace.deleted": {
      const id = (payload as EventMap["workspace.deleted"]).workspaceId
      void qc.invalidateQueries({ queryKey: keys.workspacesAll() })
      void qc.invalidateQueries({ queryKey: keys.bootstrap() })
      void qc.invalidateQueries({ queryKey: keys.trash() })
      qc.removeQueries({ queryKey: keys.ws(id).all })
      ui?.workspaceDeleted?.(id)
      return
    }
    case "accounts.changed":
      void qc.invalidateQueries({ queryKey: keys.accounts() })
      void qc.invalidateQueries({ queryKey: keys.bootstrap() })
      return
    case "graph.changed":
      inWs((id) => {
        void qc.invalidateQueries({ queryKey: keys.ws(id).graph() })
        void qc.invalidateQueries({ queryKey: keys.workspacesAll() })
      })
      return
    case "chat.created":
    case "chat.updated": {
      const { chatId } = payload as EventMap["chat.updated"]
      inWs((id) => {
        void qc.invalidateQueries({ queryKey: keys.ws(id).chats() })
        void qc.invalidateQueries({ queryKey: keys.ws(id).chat(chatId) })
      })
      return
    }
    case "chat.delta": {
      const delta = payload as EventMap["chat.delta"]
      if (useLive.getState().applyDelta(delta) === "gap") {
        inWs((id) =>
          qc.invalidateQueries({ queryKey: keys.ws(id).chat(delta.chatId) })
        )
      }
      return
    }
    case "chat.toolCall": {
      const { chatId, turnId, call } = payload as EventMap["chat.toolCall"]
      useLive.getState().addToolCall(chatId, turnId, call)
      return
    }
    case "chat.rest": {
      const { chatId } = payload as EventMap["chat.rest"]
      useLive.getState().clearTurn(chatId)
      inWs((id) => {
        void qc.invalidateQueries({ queryKey: keys.ws(id).chat(chatId) })
        void qc.invalidateQueries({ queryKey: keys.ws(id).chats() })
      })
      return
    }
    case "chat.error": {
      const { chatId, turnId, error } = payload as EventMap["chat.error"]
      useLive.getState().setTurnError(chatId, turnId, error)
      inWs((id) => qc.invalidateQueries({ queryKey: keys.ws(id).chat(chatId) }))
      return
    }
    case "job.updated":
      inWs((id) => {
        void qc.invalidateQueries({ queryKey: keys.ws(id).jobs() })
        void qc.invalidateQueries({ queryKey: keys.ws(id).chats() })
        // Jobs are embedded in ClassroomView.
        void qc.invalidateQueries({
          predicate: (q) =>
            q.queryKey[0] === "ws" &&
            q.queryKey[1] === id &&
            q.queryKey[2] === "chat",
        })
      })
      return
    case "ingest.progress":
      useLive.getState().setIngest(payload as EventMap["ingest.progress"])
      return
    case "ingest.done": {
      const done = payload as EventMap["ingest.done"]
      useLive.getState().clearIngest(done.runId)
      inWs((id) => {
        void qc.invalidateQueries({ queryKey: keys.ws(id).ingest() })
        void qc.invalidateQueries({ queryKey: keys.ws(id).digestAll() })
        void qc.invalidateQueries({ queryKey: keys.ws(id).graph() })
        void qc.invalidateQueries({ queryKey: keys.ws(id).review() })
        void qc.invalidateQueries({ queryKey: keys.workspacesAll() })
      })
      ui?.ingestDone?.(done)
      return
    }
    case "review.changed":
      inWs((id) => qc.invalidateQueries({ queryKey: keys.ws(id).review() }))
      void qc.invalidateQueries({ queryKey: keys.workspacesAll() })
      return
    case "export.ready":
      ui?.exportReady?.(payload as EventMap["export.ready"])
      return
    case "resync":
      useLive.getState().reset()
      void qc.invalidateQueries()
      return
    default: {
      const unreachable: never = name
      void unreachable
    }
  }
}
