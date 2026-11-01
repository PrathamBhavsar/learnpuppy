import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { API_PREFIX, newClientId, pendingApi, unwrap } from "../client"
import { keys } from "../keys"
import { ApiError, errorFromResponse } from "../errors"
import type { components } from "../schema"
import type {
  AnswerInput,
  ChatSummary,
  ClassroomView,
  CreateChatBody,
  JobItem,
  TurnBody,
} from "../pending-types"

export function useChats(ws: string, enabled = true) {
  return useQuery({
    queryKey: keys.ws(ws).chats(),
    enabled: enabled && ws !== "",
    queryFn: async ({ signal }): Promise<ChatSummary[]> =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/chats", {
          params: { path: { ws } },
          signal,
        })
      ).items,
  })
}

export function useChat(ws: string, chatId: string | undefined) {
  return useQuery({
    queryKey: keys.ws(ws).chat(chatId ?? ""),
    enabled: Boolean(chatId),
    queryFn: async ({ signal }) =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/chats/{id}", {
          params: { path: { ws, id: chatId ?? "" } },
          signal,
        })
      ),
  })
}

export function useCreateChat(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (body: CreateChatBody) =>
      unwrap(
        await pendingApi.POST("/workspaces/{ws}/chats", {
          params: { path: { ws } },
          body,
        })
      ),
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.ws(ws).chats() }),
  })
}

export type TurnInput =
  | { type: "user"; text: string }
  | { type: "answer"; callId: string; answer: AnswerInput }
  | { type: "steer"; text: string }
  | { type: "resume" }

/** The fresh view the server embeds in a `stale_question` 409, when present. */
function staleView(error: unknown): ClassroomView | undefined {
  if (!(error instanceof ApiError) || error.code !== "stale_question")
    return undefined
  const view = error.details?.["view"]
  return typeof view === "object" && view !== null
    ? (view as ClassroomView)
    : undefined
}

/**
 * Posts a turn. The `clientId` is minted once per attempt of the same intent: pass it back on retry
 * and the server returns the original result instead of running the turn twice. Only a typed user
 * message is echoed optimistically; answers wait for server truth.
 */
type TurnRequestBody = components["schemas"]["TurnBody"]

export function useSendTurn(ws: string, chatId: string) {
  const qc = useQueryClient()
  const key = keys.ws(ws).chat(chatId)
  return useMutation({
    mutationFn: async ({
      input,
      clientId,
    }: {
      input: TurnInput
      clientId?: string
    }) => {
      const body = { ...input, clientId: clientId ?? newClientId() } as TurnBody
      return unwrap(
        await pendingApi.POST("/workspaces/{ws}/chats/{id}/turn", {
          params: { path: { ws, id: chatId } },
          // Server bug: the chat and review DTOs are both named `AnswerDto`, so the generated schema types
          // `answer` as the review result. The real body is `AnswerInput` (docs/web/03), hence the cast.
          body: body as unknown as TurnRequestBody,
        })
      )
    },
    onMutate: async ({ input, clientId }) => {
      if (input.type !== "user") return { previous: undefined }
      await qc.cancelQueries({ queryKey: key })
      const previous = qc.getQueryData<ClassroomView>(key)
      if (previous) {
        qc.setQueryData<ClassroomView>(key, {
          ...previous,
          messages: [
            ...previous.messages,
            {
              id: `local-${clientId ?? newClientId()}`,
              role: "user",
              text: input.text,
              calls: [],
              createdAt: Date.now(),
            },
          ],
          turn: { state: "running", startedAt: Date.now() },
        })
      }
      return { previous }
    },
    onError: (error, _input, context) => {
      const fresh = staleView(error)
      if (fresh) qc.setQueryData(key, fresh)
      else if (context?.previous) qc.setQueryData(key, context.previous)
    },
    onSettled: () => qc.invalidateQueries({ queryKey: key }),
  })
}

export function useCancelTurn(ws: string, chatId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async () => {
      await pendingApi.POST("/workspaces/{ws}/chats/{id}/cancel", {
        params: { path: { ws, id: chatId } },
      })
    },
    onSettled: () =>
      qc.invalidateQueries({ queryKey: keys.ws(ws).chat(chatId) }),
  })
}

export function useRenameChat(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async ({ id, title }: { id: string; title: string }) =>
      unwrap(
        await pendingApi.PATCH("/workspaces/{ws}/chats/{id}", {
          params: { path: { ws, id } },
          body: { title },
        })
      ),
    onMutate: async ({ id, title }) => {
      await qc.cancelQueries({ queryKey: keys.ws(ws).chats() })
      const previous = qc.getQueryData<ChatSummary[]>(keys.ws(ws).chats())
      if (previous)
        qc.setQueryData<ChatSummary[]>(
          keys.ws(ws).chats(),
          previous.map((c) => (c.id === id ? { ...c, title } : c))
        )
      return { previous }
    },
    onError: (_e, _v, context) => {
      if (context?.previous)
        qc.setQueryData(keys.ws(ws).chats(), context.previous)
    },
    onSettled: (_d, _e, { id }) =>
      Promise.all([
        qc.invalidateQueries({ queryKey: keys.ws(ws).chats() }),
        qc.invalidateQueries({ queryKey: keys.ws(ws).chat(id) }),
      ]),
  })
}

export function useDeleteChat(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (id: string) => {
      await pendingApi.DELETE("/workspaces/{ws}/chats/{id}", {
        params: { path: { ws, id } },
      })
    },
    onSuccess: () => qc.invalidateQueries({ queryKey: keys.ws(ws).chats() }),
  })
}

export function useJobs(ws: string) {
  return useQuery({
    queryKey: keys.ws(ws).jobs(),
    enabled: ws !== "",
    queryFn: async ({ signal }): Promise<JobItem[]> =>
      unwrap(
        await pendingApi.GET("/workspaces/{ws}/jobs", {
          params: { path: { ws } },
          signal,
        })
      ).items,
  })
}

export function useAbortJob(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (id: string) =>
      unwrap(
        await pendingApi.POST("/workspaces/{ws}/jobs/{id}/abort", {
          params: { path: { ws, id } },
        })
      ),
    onSettled: () => qc.invalidateQueries({ queryKey: keys.ws(ws).jobs() }),
  })
}

/** Attaches a PDF to a chat; the server starts a `pdf` workbench job. */
export function useUploadPdf(ws: string, chatId: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (file: File) => {
      const form = new FormData()
      form.set("file", file)
      return unwrap(
        await pendingApi.POST("/workspaces/{ws}/sources/pdf", {
          params: { path: { ws }, query: { chat: chatId } },
          body: form as unknown as { file: string },
          bodySerializer: (body) => body as unknown as FormData,
        })
      )
    },
    onSuccess: () =>
      Promise.all([
        qc.invalidateQueries({ queryKey: keys.ws(ws).jobs() }),
        qc.invalidateQueries({ queryKey: keys.ws(ws).chats() }),
      ]),
  })
}

export type Artifact =
  | { kind: "text"; contentType: string; text: string }
  | { kind: "image"; contentType: string; url: string }

/** Job output with the server's content type; images come back as an object URL. */
export function useArtifact(ws: string, artifactId: string | null) {
  return useQuery({
    queryKey: [...keys.ws(ws).jobs(), "artifact", artifactId] as const,
    enabled: artifactId !== null && ws !== "",
    staleTime: Infinity,
    queryFn: async ({ signal }): Promise<Artifact> => {
      const response = await fetch(
        `${API_PREFIX}/workspaces/${encodeURIComponent(ws)}/artifacts/${encodeURIComponent(artifactId ?? "")}`,
        { signal, credentials: "same-origin", headers: { "X-LP-CSRF": "1" } }
      )
      if (!response.ok) throw await errorFromResponse(response)
      const contentType = response.headers.get("content-type") ?? "text/plain"
      if (contentType.startsWith("image/") && !contentType.includes("svg")) {
        return {
          kind: "image",
          contentType,
          url: URL.createObjectURL(await response.blob()),
        }
      }
      return { kind: "text", contentType, text: await response.text() }
    },
  })
}

/** Markdown transcript of a chat, fetched on demand and handed back for download. */
export async function fetchChatExport(
  ws: string,
  chatId: string
): Promise<string> {
  const response = await fetch(
    `${API_PREFIX}/workspaces/${encodeURIComponent(ws)}/chats/${encodeURIComponent(chatId)}/export.md`,
    { credentials: "same-origin", headers: { "X-LP-CSRF": "1" } }
  )
  if (!response.ok) throw await errorFromResponse(response)
  return response.text()
}
