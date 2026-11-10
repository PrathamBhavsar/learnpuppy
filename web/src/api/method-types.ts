/**
 * Learning-method course route types, taken from the generated OpenAPI schema so a server field
 * rename fails `tsc` here instead of showing `undefined` in the UI.
 * The server decides every step and every grade; nothing here carries an answer, a `why` or a rubric.
 */
import type { components } from "./schema"

type S = components["schemas"]

export type CoursePlan = S["CoursePlanDto"]
export type MethodItem = S["MethodItemDto"]
export type MethodStep = S["MethodStepDto"]
export type InsertPrereqRequest = S["InsertPrereqRequest"]
export type MethodAnswerRequest = S["MethodAnswerRequest"]
export type MethodAnswerResult = S["MethodAnswerResultDto"]
export type CloseReport = S["CloseReportDto"]
export type ReviewDate = S["ReviewDateDto"]
export type ReviewDatesRequest = S["ReviewDatesRequest"]
export type RegisterRow = S["RegisterRowDto"]
export type FeynmanRequest = S["FeynmanRequest"]
export type FeynmanResult = S["FeynmanResultDto"]
export type MasteryCount = S["MasteryCountDto"]
