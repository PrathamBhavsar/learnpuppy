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
