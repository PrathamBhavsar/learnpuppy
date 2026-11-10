import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query"

import { learningApi, unwrap } from "@/api/client"
import { keys } from "@/api/keys"
import type {
  CheckinRequest,
  CheckinResult,
  Clock,
  MethodSettings,
  OverrideRequest,
  ProfileResult,
  ReadyAnswers,
  Schedule,
  SetProfileRequest,
  SetScheduleRequest,
  SuggestSlotsResult,
  TrackSlotsResult,
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

/** Every tunable method setting with its default and allowed range. */
export function useMethodSettings(ws: string) {
  return useQuery({
    queryKey: [...base(ws), "method-settings"],
    queryFn: async ({ signal }): Promise<MethodSettings> =>
      unwrap(
        await learningApi.GET("/workspaces/{ws}/learning/settings", {
          params: { path: { ws } },
          signal,
        })
      ),
  })
}

/** F01: saved wake time and slots, kept and missed tracking. `.ics` export is deferred past G4. */
export function useSchedule(ws: string) {
  return useQuery({
    queryKey: [...base(ws), "schedule"],
    queryFn: async ({ signal }): Promise<Schedule> =>
      unwrap(
        await learningApi.GET("/workspaces/{ws}/learning/schedule", {
          params: { path: { ws } },
          signal,
        })
      ),
  })
}

export function useTrackSlots(ws: string) {
  return useQuery({
    queryKey: [...base(ws), "schedule", "track"],
    queryFn: async ({ signal }): Promise<TrackSlotsResult> =>
      unwrap(
        await learningApi.GET("/workspaces/{ws}/learning/schedule/track", {
          params: { path: { ws } },
          signal,
        })
      ),
  })
}

/** F34: dated profile entries; the latest per key wins, older ones stay in the log. */
export function useProfile(ws: string) {
  return useQuery({
    queryKey: [...base(ws), "profile"],
    queryFn: async ({ signal }): Promise<ProfileResult> =>
      unwrap(
        await learningApi.GET("/workspaces/{ws}/learning/profile", {
          params: { path: { ws } },
          signal,
        })
      ),
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
    setMethodSetting: useMutation({
      mutationFn: async (body: {
        name: string
        value: number
      }): Promise<MethodSettings> =>
        unwrap(
          await learningApi.POST("/workspaces/{ws}/learning/settings", {
            params: { path },
            body,
          })
        ),
      onSuccess: (settings) => {
        qc.setQueryData([...base(ws), "method-settings"], settings)
        // The clock renders against the saved limits.
        refreshClock()
      },
    }),
    setSchedule: useMutation({
      mutationFn: async (body: SetScheduleRequest): Promise<Schedule> =>
        unwrap(
          await learningApi.POST("/workspaces/{ws}/learning/schedule", {
            params: { path },
            body,
          })
        ),
      onSuccess: (schedule) =>
        qc.setQueryData([...base(ws), "schedule"], schedule),
    }),
    suggestSlots: useMutation({
      mutationFn: async (wakeTime: string): Promise<SuggestSlotsResult> =>
        unwrap(
          await learningApi.POST("/workspaces/{ws}/learning/schedule/suggest", {
            params: { path },
            body: { wakeTime },
          })
        ),
    }),
    setProfile: useMutation({
      mutationFn: async (body: SetProfileRequest): Promise<ProfileResult> =>
        unwrap(
          await learningApi.POST("/workspaces/{ws}/learning/profile", {
            params: { path },
            body,
          })
        ),
      onSuccess: (profile) =>
        qc.setQueryData([...base(ws), "profile"], profile),
    }),
  }
}

export type LearningActions = ReturnType<typeof useLearningActions>
