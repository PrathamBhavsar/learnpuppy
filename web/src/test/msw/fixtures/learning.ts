import type {
  Clock,
  MethodSetting,
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
  settings: MethodSetting[]
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
    settings: [
      { name: "bout_limit_minutes", value: 90, default: 90, min: 5, max: 180 },
      {
        name: "attempts_before_parking",
        value: 3,
        default: 3,
        min: 1,
        max: 10,
      },
    ],
  }
}
