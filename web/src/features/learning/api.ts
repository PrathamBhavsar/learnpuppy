import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { learningApi, unwrap } from "@/api/client"
import { keys } from "@/api/keys"
import type {
  CheckinRequest,
  CheckinResult,
  Clock,
  OverrideRequest,
  ReadyAnswers,
} from "@/api/learning-types"

const base = (ws: string) => [...keys.ws(ws).all, "learning"] as const
const clockKey = (ws: string) => [...base(ws), "clock"] as const

/** F23's clock: built purely from server timestamps, so a refresh or restart cannot reset it. */
export function useClock(
  ws: string,
  options: { pollMs?: number | false } = {}
) {
  return useQuery({
    queryKey: clockKey(ws),
    refetchInterval: options.pollMs ?? 15_000,
    queryFn: async ({ signal }) =>
      unwrap(
        await learningApi.GET("/workspaces/{ws}/learning/clock", {
          params: { path: { ws } },
          signal,
        })
      ),
  })
}

/** True only before the first check-in or bout of the day (F04). */
export function useCheckinNeeded(ws: string) {
  return useQuery({
    queryKey: [...base(ws), "checkin-needed"],
    queryFn: async ({ signal }) =>
      unwrap(
        await learningApi.GET("/workspaces/{ws}/learning/checkin/needed", {
          params: { path: { ws } },
          signal,
        })
      ).needed,
  })
}

/** Every mutation here either carries the fresh clock back or is refetched right after. */
export function useLearningActions(ws: string) {
  const path = { ws }
  const qc = useQueryClient()
  const refreshClock = () => qc.invalidateQueries({ queryKey: base(ws) })
  const setClock = (clock: Clock) => qc.setQueryData(clockKey(ws), clock)
  return {
    startBout: useMutation({
      mutationFn: async (body: {
        course: string
        ready: ReadyAnswers
      }): Promise<Clock> =>
        unwrap(
          await learningApi.POST("/workspaces/{ws}/learning/bout/start", {
            params: { path },
            body,
          })
        ),
      onSuccess: setClock,
    }),
    heartbeat: useMutation({
      mutationFn: async (bout: string): Promise<Clock> =>
        unwrap(
          await learningApi.POST("/workspaces/{ws}/learning/bout/heartbeat", {
            params: { path },
            body: { bout },
          })
        ),
      onSuccess: setClock,
    }),
    endBout: useMutation({
      mutationFn: async (bout: string): Promise<Clock> =>
        unwrap(
          await learningApi.POST("/workspaces/{ws}/learning/bout/end", {
            params: { path },
            body: { bout },
          })
        ),
      onSuccess: setClock,
    }),
    focusLost: useMutation({
      mutationFn: async (bout: string) =>
        await learningApi.POST("/workspaces/{ws}/learning/bout/focus-lost", {
          params: { path },
          body: { bout },
        }),
    }),
    override: useMutation({
      mutationFn: async (body: OverrideRequest) =>
        await learningApi.POST("/workspaces/{ws}/learning/bout/override", {
          params: { path },
          body,
        }),
      onSuccess: refreshClock,
    }),
    checkin: useMutation({
      mutationFn: async (body: CheckinRequest): Promise<CheckinResult> =>
        unwrap(
          await learningApi.POST("/workspaces/{ws}/learning/checkin", {
            params: { path },
            body,
          })
        ),
      onSuccess: () =>
        qc.invalidateQueries({ queryKey: [...base(ws), "checkin-needed"] }),
    }),
  }
}

export type LearningActions = ReturnType<typeof useLearningActions>
