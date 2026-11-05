import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { methodApi, unwrap } from "@/api/client"
import { keys } from "@/api/keys"
import type {
  InsertPrereqRequest,
  MethodAnswerRequest,
  MethodStep,
} from "@/api/method-types"

const base = (ws: string, id: string) =>
  [...keys.ws(ws).all, "courses", id, "method"] as const

/** The plan map. A course on the stored-sequence path has none (the server answers 404), so `data` is undefined. */
export function useCoursePlan(ws: string, id: string) {
  return useQuery({
    queryKey: [...base(ws, id), "plan"],
    retry: false,
    queryFn: async ({ signal }) =>
      unwrap(
        await methodApi.GET("/workspaces/{ws}/courses/{id}/plan", {
          params: { path: { ws, id } },
          signal,
        })
      ),
  })
}

export function useMastery(ws: string, id: string, version: number) {
  return useQuery({
    queryKey: [...base(ws, id), "mastery", version],
    queryFn: async ({ signal }) =>
      unwrap(
        await methodApi.GET("/workspaces/{ws}/courses/{id}/mastery", {
          params: { path: { ws, id } },
          signal,
        })
      ).countLine,
  })
}

export function useRegister(ws: string, id: string, enabled: boolean) {
  return useQuery({
    queryKey: [...base(ws, id), "register"],
    enabled,
    queryFn: async ({ signal }) =>
      unwrap(
        await methodApi.GET("/workspaces/{ws}/courses/{id}/register", {
          params: { path: { ws, id } },
          signal,
        })
      ).rows,
  })
}

/** Every action is a plain POST; the server's pure gate decides whether it is allowed (409 with a code). */
export function useMethodActions(ws: string, id: string) {
  const path = { ws, id }
  const qc = useQueryClient()
  const refresh = () => qc.invalidateQueries({ queryKey: base(ws, id) })
  return {
    step: useMutation({
      mutationFn: async (): Promise<MethodStep> =>
        unwrap(
          await methodApi.POST("/workspaces/{ws}/courses/{id}/step", {
            params: { path },
          })
        ),
    }),
    confirmPlan: useMutation({
      mutationFn: async () =>
        await methodApi.POST("/workspaces/{ws}/courses/{id}/plan/confirm", {
          params: { path },
        }),
      onSuccess: refresh,
    }),
    insertPrereq: useMutation({
      mutationFn: async (body: InsertPrereqRequest) =>
        unwrap(
          await methodApi.POST("/workspaces/{ws}/courses/{id}/plan/prereq", {
            params: { path },
            body,
          })
        ),
      onSuccess: refresh,
    }),
    predict: useMutation({
      mutationFn: async (body: { lesson: string; guess: string }) =>
        await methodApi.POST("/workspaces/{ws}/courses/{id}/predict", {
          params: { path },
          body,
        }),
    }),
    explainSeen: useMutation({
      mutationFn: async (lesson: string) =>
        await methodApi.POST("/workspaces/{ws}/courses/{id}/explain-seen", {
          params: { path },
          body: { lesson },
        }),
    }),
    note: useMutation({
      mutationFn: async (body: { lesson: string; text: string }) =>
        await methodApi.POST("/workspaces/{ws}/courses/{id}/note", {
          params: { path },
          body,
        }),
    }),
    answer: useMutation({
      mutationFn: async (body: MethodAnswerRequest) =>
        unwrap(
          await methodApi.POST("/workspaces/{ws}/courses/{id}/quiz-answer", {
            params: { path },
            body,
          })
        ),
      onSuccess: refresh,
    }),
    feynman: useMutation({
      mutationFn: async (body: { lesson: string; explanation: string }) =>
        unwrap(
          await methodApi.POST("/workspaces/{ws}/courses/{id}/feynman", {
            params: { path },
            body,
          })
        ),
      onSuccess: refresh,
    }),
    rest: useMutation({
      mutationFn: async (lesson: string) =>
        await methodApi.POST("/workspaces/{ws}/courses/{id}/break", {
          params: { path },
          body: { lesson },
        }),
    }),
    close: useMutation({
      mutationFn: async () =>
        unwrap(
          await methodApi.POST("/workspaces/{ws}/courses/{id}/close", {
            params: { path },
          })
        ),
      onSuccess: refresh,
    }),
  }
}

export type MethodActions = ReturnType<typeof useMethodActions>
