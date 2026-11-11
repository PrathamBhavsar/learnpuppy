import {
  useMutation,
  useQueries,
  useQuery,
  useQueryClient,
} from "@tanstack/react-query"

import { api, newClientId, unwrap } from "@/api/client"
import { keys } from "@/api/keys"
import type { components } from "@/api/schema"

type Dto = components["schemas"]
export type Book = Dto["TextbookDto"]
export type BookSummary = Dto["TextbookSummaryDto"]
export type Chapter = Dto["ChapterRowDto"]
export type Job = Dto["JobItemDto"]
export type ReadingCheck = Dto["ReadingCheckDto"]
export type CheckItem = Dto["CheckItemDto"]
export type Feedback = Dto["CheckFeedbackDto"]

const bookKey = (ws: string, book?: string) =>
  [...keys.ws(ws).all, "textbooks", ...(book ? [book] : [])] as const

export function useBooks(ws: string) {
  return useQuery({
    queryKey: bookKey(ws),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/workspaces/{ws}/textbooks", {
          params: { path: { ws } },
          signal,
        })
      ).items,
    // SSE is the freshness path; a slow poll covers a dropped connection while an import runs.
    refetchInterval: (q) =>
      q.state.data?.some((b) => b.state === "importing") ? 4000 : false,
  })
}

export function useBook(ws: string, book: string) {
  return useQuery({
    queryKey: bookKey(ws, book),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/workspaces/{ws}/textbooks/{book}", {
          params: { path: { ws, book } },
          signal,
        })
      ),
    refetchInterval: (q) => (q.state.data?.jobId ? 4000 : false),
  })
}

/** Every book's detail, for the digest's "chapters due" line. One request per book, none to a model. */
export function useBooksDue(ws: string): number {
  const list = useBooks(ws)
  const rows = useQueries({
    queries: (list.data ?? []).map((b) => ({
      queryKey: bookKey(ws, b.id),
      queryFn: async ({ signal }: { signal: AbortSignal }) =>
        unwrap(
          await api.GET("/workspaces/{ws}/textbooks/{book}", {
            params: { path: { ws, book: b.id } },
            signal,
          })
        ),
    })),
  })
  return rows.reduce(
    (n, r) =>
      n + (r.data?.chapters.filter((c) => c.status === "due").length ?? 0),
    0
  )
}

export function useChapterDetail(
  ws: string,
  book: string,
  n: number,
  enabled: boolean
) {
  return useQuery({
    queryKey: [...bookKey(ws, book), "chapter", n],
    enabled,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/workspaces/{ws}/textbooks/{book}/chapters/{n}", {
          params: { path: { ws, book, n } },
          signal,
        })
      ),
  })
}

/** The workspace's jobs; the import's progress is the entry whose `bookId` matches. */
export function useJobs(ws: string, enabled: boolean) {
  return useQuery({
    queryKey: keys.ws(ws).jobs(),
    enabled,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/workspaces/{ws}/jobs", {
          params: { path: { ws } },
          signal,
        })
      ).items,
  })
}

export function useUpload(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (file: File) =>
      unwrap(
        await api.POST("/workspaces/{ws}/sources/textbook", {
          params: { path: { ws } },
          body: { file: file as unknown as string },
          bodySerializer: () => {
            const form = new FormData()
            form.append("file", file)
            return form
          },
        })
      ),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: bookKey(ws) })
      void qc.invalidateQueries({ queryKey: keys.ws(ws).jobs() })
    },
  })
}

export function useImport(ws: string, book: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (chapters?: number[]) =>
      unwrap(
        await api.POST("/workspaces/{ws}/textbooks/{book}/import", {
          params: { path: { ws, book } },
          body: chapters ? { chapters } : {},
        })
      ),
    onSuccess: () => {
      void qc.invalidateQueries({ queryKey: bookKey(ws) })
      void qc.invalidateQueries({ queryKey: keys.ws(ws).jobs() })
    },
  })
}

export function useAbort(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (id: string) =>
      await api.POST("/workspaces/{ws}/jobs/{id}/abort", {
        params: { path: { ws, id } },
      }),
    onSettled: () => {
      void qc.invalidateQueries({ queryKey: bookKey(ws) })
      void qc.invalidateQueries({ queryKey: keys.ws(ws).jobs() })
    },
  })
}

/** The reading check: start, answer, self-grade. Grading happens on the server, in code. */
export function useCheckActions(ws: string, book: string) {
  const qc = useQueryClient()
  const path = { ws, book }
  const refresh = () => qc.invalidateQueries({ queryKey: bookKey(ws) })
  return {
    start: useMutation({
      mutationFn: async (n: number): Promise<ReadingCheck> =>
        unwrap(
          await api.POST(
            "/workspaces/{ws}/textbooks/{book}/chapters/{n}/check",
            {
              params: { path: { ...path, n } },
            }
          )
        ),
    }),
    answer: useMutation({
      mutationFn: async (v: {
        check: string
        body: Dto["CheckAnswerRequest"]
      }): Promise<Feedback> =>
        unwrap(
          await api.POST(
            "/workspaces/{ws}/textbooks/{book}/checks/{check}/answer",
            {
              params: { path: { ...path, check: v.check } },
              body: { clientId: newClientId(), ...v.body },
            }
          )
        ),
      onSuccess: refresh,
    }),
    selfGrade: useMutation({
      mutationFn: async (v: { check: string; body: Dto["SelfGradeRequest"] }) =>
        await api.POST(
          "/workspaces/{ws}/textbooks/{book}/checks/{check}/self-grade",
          { params: { path: { ...path, check: v.check } }, body: v.body }
        ),
      onSuccess: refresh,
    }),
  }
}
