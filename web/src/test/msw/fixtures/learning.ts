import type { Clock } from "@/api/learning-types"

export type MockLearning = {
  clock: Clock
  checkinNeeded: boolean
}

export function learningFixture(): MockLearning {
  return {
    clock: {
      bout: null,
      status: { status: "active", activeMinutes: 0 },
      minutesToday: 0,
      overrideUsedToday: false,
      gapPauseDue: false,
      gapPauseSeconds: 10,
      boutLimitMinutes: 90,
      dailyLimitMinutes: 270,
    },
    checkinNeeded: true,
  }
}
