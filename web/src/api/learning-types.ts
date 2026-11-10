/**
 * Bout clock, check-in, schedule and profile wire types, taken from the generated OpenAPI schema
 * so a server field rename fails `tsc` here instead of showing `undefined` in the UI. Bouts are
 * workspace-wide, not per-course (`brain/learning-method/architecture.md` section 3).
 */
import type { components } from "./schema"

type S = components["schemas"]

export type Clock = S["ClockDto"]
export type ReadyAnswers = S["ReadyAnswersDto"]
export type OverrideRequest = S["OverrideRequest"]
export type CheckinRequest = S["CheckinRequest"]
export type CheckinResult = S["CheckinResultDto"]
export type Schedule = S["ScheduleDto"]
export type SetScheduleRequest = S["SetScheduleRequest"]
export type SuggestSlotsResult = S["SuggestSlotsResultDto"]
export type TrackSlotsResult = S["TrackSlotsResultDto"]
export type SetProfileRequest = S["SetProfileRequest"]
export type ProfileResult = S["ProfileResultDto"]
export type MethodSetting = S["MethodSettingDto"]
export type MethodSettings = S["MethodSettingsDto"]
export type StartBoutRequest = S["StartBoutRequest"]
export type SuggestSlotsRequest = S["SuggestSlotsRequest"]
export type SlotRecord = S["SlotRecordDto"]
export type ProfileEntry = S["ProfileEntryDto"]
