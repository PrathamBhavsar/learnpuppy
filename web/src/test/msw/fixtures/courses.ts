import type { CoursePlan, MethodStep } from "@/api/method-types"
import type { components } from "@/api/schema"

type Dto = components["schemas"]

export type MockCourse = {
  id: string
  title: string
  /** Placement answers by item id: what the mock "server" grades against. */
  key: Record<string, { answer: string; why: string; q: string }>
  /** Stored steps handed out in order by `next`. */
  steps: Dto["StepDto"][]
  cursor: number
  state: Dto["CourseState"]
  graded: boolean
  /** Set for a learning-method course: the stored steps `step` hands out, one per learner action. */
  method?: { steps: MethodStep[]; cursor: number }
}

export const placementItems: Dto["PlacementItemDto"][] = [
  {
    id: "p1",
    kind: "mcq",
    q: "What does `await` do?",
    options: ["Blocks the thread", "Yields until the future is ready"],
  },
  {
    id: "p2",
    kind: "recall",
    q: "Which trait does a future implement?",
    options: [],
  },
]

const placementKey: MockCourse["key"] = {
  p1: {
    q: "What does `await` do?",
    answer: "Yields until the future is ready",
    why: "`await` hands control back to the executor.",
  },
  p2: {
    q: "Which trait does a future implement?",
    answer: "Future",
    why: "Anything you can poll implements `Future`.",
  },
}

const lessonSteps: Dto["StepDto"][] = [
  {
    step: "explain",
    lesson: {
      id: "l1",
      title: "Polling",
      objective: "Know what poll returns.",
      explain: "A future is polled until it returns `Ready`.",
      keyFacts: ["poll returns Pending or Ready"],
    },
  },
  {
    step: "ask",
    item: {
      id: "q1",
      lesson: "l1",
      ask: "new",
      kind: "mcq",
      q: "What can poll return?",
      options: ["Pending or Ready", "Ok or Err"],
    },
  },
  {
    step: "ask",
    item: {
      id: "q2",
      lesson: "l1",
      ask: "new",
      kind: "recall",
      q: "Name the value poll returns when not finished.",
      options: [],
    },
  },
  { step: "done" },
]

export const courseUsage: Dto["UsageDto"] = {
  strongIn: 1200,
  strongOut: 3400,
  cheapIn: 300,
  cheapOut: 150,
  strongCallsThisHour: 1,
}

export function newCourse(id: string, goal: string): MockCourse {
  return {
    id,
    title: goal,
    key: placementKey,
    steps: lessonSteps,
    cursor: 0,
    state: "writing",
    graded: false,
  }
}

/** The answer the mock "server" holds back until grading; it must never reach a payload or the DOM before. */
export const SECRET_ANSWER = "Pending-under-lock"

const methodSteps: MethodStep[] = [
  { step: "confirm_plan" },
  {
    step: "predict",
    lesson: "l1",
    title: "Polling",
    q: "What will poll give back?",
    options: ["A value", "A status"],
  },
  {
    step: "explain",
    id: "l1",
    title: "Polling",
    hook: "Futures do nothing until asked.",
    explain: "A future is polled until it returns `Ready`.",
    keyFacts: ["poll returns Pending or Ready"],
  },
  { step: "notes", lesson: "l1", title: "Polling" },
  {
    step: "ask",
    id: "m1",
    lesson: "l1",
    kind: "mcq",
    q: "Which claim is true?",
    options: ["poll blocks", "poll returns a status", "poll allocates"],
    ask: "new",
  },
  {
    step: "ask",
    id: "m2",
    lesson: "l1",
    kind: "recall",
    q: "What does poll return when unfinished?",
    options: [],
    ask: "retry",
    angle: "Think of a kettle: not boiled yet means not ready.",
  },
  { step: "feynman", lesson: "l1", title: "Polling", keyFacts: ["Pending"] },
  { step: "break", lesson: "l1", title: "Polling" },
  { step: "close" },
]

export const methodPlan: CoursePlan = {
  confirmed: false,
  why: "I want to read other people's async code",
  strands: [
    { name: "Futures", label: "known" },
    { name: "Polling", label: "edge" },
    { name: "Executors", label: "unknown" },
  ],
  lessons: [
    { id: "l0", title: "Futures", kind: "definitional", dependsOn: [] },
    { id: "l1", title: "Polling", kind: "conceptual", dependsOn: ["l0"] },
  ],
}

export function newMethodCourse(id: string, goal: string): MockCourse {
  return {
    ...newCourse(id, goal),
    graded: true,
    method: { steps: methodSteps, cursor: 0 },
  }
}
