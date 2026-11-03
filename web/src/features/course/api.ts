import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { api, unwrap } from "@/api/client"
import { keys } from "@/api/keys"
import type { components } from "@/api/schema"

type Dto = components["schemas"]
export type Step = Dto["StepDto"]

const courseKey = (ws: string, id?: string) =>
  [...keys.ws(ws).all, "courses", ...(id ? [id] : [])] as const

export function useCourses(ws: string) {
  return useQuery({
    queryKey: courseKey(ws),
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/workspaces/{ws}/courses", {
          params: { path: { ws } },
          signal,
        })
      ).courses,
  })
}

export function useCreateCourse(ws: string) {
  const qc = useQueryClient()
  return useMutation({
    mutationFn: async (goal: string) =>
      unwrap(
        await api.POST("/workspaces/{ws}/courses", {
          params: { path: { ws } },
          body: { goal },
        })
      ),
    onSuccess: () => qc.invalidateQueries({ queryKey: courseKey(ws) }),
  })
}

/** Polls while the Author writes; stops once the course is `ready` or `failed`. */
export function useCourseStatus(ws: string, id: string, enabled: boolean) {
  return useQuery<Dto["CourseStatusDto"]>({
    queryKey: [...courseKey(ws, id), "status"],
    enabled,
    refetchInterval: (q) => {
      const state = q.state.data?.state
      return state === "ready" || state === "failed" ? false : 2000
    },
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/workspaces/{ws}/courses/{id}/status", {
          params: { path: { ws, id } },
          signal,
        })
      ),
  })
}

export function useMisses(ws: string, id: string, enabled: boolean) {
  return useQuery({
    queryKey: [...courseKey(ws, id), "misses"],
    enabled,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/workspaces/{ws}/courses/{id}/misses", {
          params: { path: { ws, id } },
          signal,
        })
      ).misses,
  })
}

export function useUsage(ws: string, id: string, enabled: boolean) {
  return useQuery({
    queryKey: [...courseKey(ws, id), "usage"],
    enabled,
    queryFn: async ({ signal }) =>
      unwrap(
        await api.GET("/workspaces/{ws}/courses/{id}/usage", {
          params: { path: { ws, id } },
          signal,
        })
      ),
  })
}

/** Every course action is a plain POST; the server decides the result. */
export function useCourseActions(ws: string, id: string) {
  const path = { ws, id }
  const qc = useQueryClient()
  return {
    placement: useMutation({
      mutationFn: async (answers: Dto["PlacementAnswerDto"][]) =>
        unwrap(
          await api.POST("/workspaces/{ws}/courses/{id}/placement", {
            params: { path },
            body: { answers },
          })
        ).graded,
      onSuccess: () => qc.invalidateQueries({ queryKey: courseKey(ws, id) }),
    }),
    retry: useMutation({
      mutationFn: async () =>
        await api.POST("/workspaces/{ws}/courses/{id}/retry", {
          params: { path },
        }),
      onSuccess: () => qc.invalidateQueries({ queryKey: courseKey(ws, id) }),
    }),
    next: useMutation({
      mutationFn: async (): Promise<Step> =>
        unwrap(
          await api.POST("/workspaces/{ws}/courses/{id}/next", {
            params: { path },
          })
        ),
    }),
    seen: useMutation({
      mutationFn: async (lesson: string) =>
        await api.POST("/workspaces/{ws}/courses/{id}/seen", {
          params: { path },
          body: { lesson },
        }),
    }),
    answer: useMutation({
      mutationFn: async (body: Dto["CourseAnswerRequest"]) =>
        unwrap(
          await api.POST("/workspaces/{ws}/courses/{id}/answer", {
            params: { path },
            body,
          })
        ),
    }),
    selfGrade: useMutation({
      mutationFn: async (body: Dto["SelfGradeRequest"]) =>
        await api.POST("/workspaces/{ws}/courses/{id}/self-grade", {
          params: { path },
          body,
        }),
    }),
    flag: useMutation({
      mutationFn: async (body: Dto["FlagRequest"]) =>
        await api.POST("/workspaces/{ws}/courses/{id}/flag", {
          params: { path },
          body,
        }),
    }),
    ask: useMutation({
      mutationFn: async (question: string) =>
        unwrap(
          await api.POST("/workspaces/{ws}/courses/{id}/ask", {
            params: { path },
            body: { question },
          })
        ).reply,
    }),
    check: useMutation({
      mutationFn: async (body: Dto["CheckRequest"]) =>
        unwrap(
          await api.POST("/workspaces/{ws}/courses/{id}/check", {
            params: { path },
            body,
          })
        ),
    }),
  }
}
