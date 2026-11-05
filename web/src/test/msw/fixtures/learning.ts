import type {
  Clock,
  ProfileEntry,
  Schedule,
  SlotRecord,
} from "@/api/learning-types"

export type MockLearning = {
  clock: Clock
  checkinNeeded: boolean
  schedule: Schedule
  slots: SlotRecord[]
  profile: ProfileEntry[]
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
    schedule: { wakeTime: "", slots: [] },
    slots: [],
    profile: [],
  }
}
