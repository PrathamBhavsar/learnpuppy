import type { ChatSummary, ClassroomView } from "@/api/pending-types"

import { DAY, HOUR, MINUTE } from "./rng"

const msg = (
  id: string,
  role: ClassroomView["messages"][number]["role"],
  text: string,
  createdAt: number,
  extra: Partial<ClassroomView["messages"][number]> = {}
): ClassroomView["messages"][number] => ({
  id,
  role,
  text,
  createdAt,
  calls: [],
  ...extra,
})

const PLAN = (statuses: ("current" | "visited" | "upcoming")[]) => ({
  nodes: [
    { id: "p1", title: "What a future is", status: statuses[0] ?? "upcoming" },
    {
      id: "p2",
      title: "Polling and wakers",
      status: statuses[1] ?? "upcoming",
    },
    { id: "p3", title: "Pinning", status: statuses[2] ?? "upcoming" },
    { id: "p4", title: "Executors", status: statuses[3] ?? "upcoming" },
  ] as {
    id: string
    title: string
    status: "current" | "visited" | "upcoming"
  }[],
  edges: [
    { from: "p1", to: "p2" },
    { from: "p2", to: "p3" },
    { from: "p2", to: "p4" },
  ],
  currentId: ("p" + (statuses.findIndex((s) => s === "current") + 1)) as string,
})

/** One chat in each phase, plus a running turn and an errored turn, for the Rust workspace. */
export function chatFixtures(now: number): ClassroomView[] {
  const base = {
    viewVersion: 3,
    verdicts: [],
    jobs: [],
    turn: { state: "idle" as const },
  }
  return [
    {
      ...base,
      id: "c_probe_pending",
      kind: "teach",
      title: "How does async Rust work?",
      phase: "probe",
      messages: [
        msg(
          "m1",
          "user",
          "Teach me how async Rust works under the hood.",
          now - 3 * HOUR
        ),
        msg(
          "m2",
          "assistant",
          "Before I plan this, a few questions to see where you are. What do you think happens when you call an `async fn`?",
          now - 3 * HOUR + MINUTE,
          { calls: [{ id: "call_probe_3", name: "probe" }] }
        ),
      ],
      pending: {
        callId: "call_probe_3",
        tool: "probe",
        prompt:
          "What do you think happens when you call an `async fn` without awaiting the result?",
        allowFreeText: true,
      },
      plan: null,
      probeProgress: { asked: 3, max: 8 },
    },
    {
      ...base,
      id: "c_plan",
      kind: "teach",
      title: "Tokio scheduler internals",
      phase: "plan",
      messages: [
        msg(
          "m1",
          "user",
          "I want to understand how tokio schedules tasks.",
          now - DAY
        ),
        msg(
          "m2",
          "assistant",
          "Here is a plan that starts from wakers and ends at the work-stealing scheduler. Tell me if you want to reorder anything.",
          now - DAY + MINUTE
        ),
      ],
      pending: null,
      plan: PLAN(["current", "upcoming", "upcoming", "upcoming"]),
      probeProgress: { asked: 8, max: 8 },
    },
    {
      ...base,
      id: "c_teach_quiz",
      kind: "teach",
      title: "Futures, polling and wakers",
      phase: "teach",
      viewVersion: 9,
      messages: [
        msg("m1", "user", "Teach me how async Rust works.", now - 2 * DAY),
        msg(
          "m2",
          "assistant",
          "A future is a value that describes work that has not finished yet. Nothing happens until an executor **polls** it.\n\n```rust\nlet fut = async { 1 + 1 }; // nothing runs yet\n```",
          now - 2 * DAY + MINUTE,
          { thinking: "Start from laziness, then polling." }
        ),
        msg(
          "m3",
          "tool",
          "Explained: what a future is",
          now - 2 * DAY + 2 * MINUTE,
          { calls: [{ id: "call_explain_1", name: "explain" }] }
        ),
        msg(
          "m4",
          "assistant",
          "Let me check the idea landed.",
          now - 2 * DAY + 3 * MINUTE,
          { calls: [{ id: "call_quiz_2", name: "quiz" }] }
        ),
      ],
      pending: {
        callId: "call_quiz_2",
        tool: "quiz",
        prompt: "What does poll return when a future is not finished?",
        options: [
          { key: "A", text: "Poll::Pending, after registering a waker" },
          { key: "B", text: "Poll::Ready(None)" },
          { key: "C", text: "It blocks the thread until the future is ready" },
          { key: "D", text: "It panics unless the future was pinned" },
        ],
        allowFreeText: false,
        confidenceScale: 5,
      },
      plan: PLAN(["visited", "current", "upcoming", "upcoming"]),
      probeProgress: { asked: 8, max: 8 },
      verdicts: [
        {
          callId: "call_quiz_1",
          correct: true,
          explanation: "Right: futures are lazy.",
        },
      ],
    },
    {
      ...base,
      id: "c_practice_problem",
      kind: "practice",
      title: "Practice: pinning",
      phase: "practice",
      messages: [
        msg(
          "m1",
          "assistant",
          "Time to apply what you know about pinning.",
          now - 5 * HOUR,
          { calls: [{ id: "call_problem_1", name: "problem" }] }
        ),
      ],
      pending: {
        callId: "call_problem_1",
        tool: "problem",
        prompt: "Solve the problem below.",
        allowFreeText: true,
        problem: {
          mode: "composition",
          statement:
            "Write a function `fn run(fut: impl Future<Output = u32>) -> u32` that drives a future to completion on the current thread. Explain why the future must be pinned before it is polled.",
          rungs: [
            "Think about what poll takes as its receiver.",
            "std::pin::pin! pins on the stack.",
            "You need a Context built from a waker.",
          ],
        },
      },
      plan: null,
      probeProgress: { asked: 0, max: 8 },
    },
    {
      ...base,
      id: "c_done",
      kind: "teach",
      title: "Channels and backpressure",
      phase: "done",
      messages: [
        msg("m1", "user", "Explain mpsc backpressure.", now - 6 * DAY),
        msg(
          "m2",
          "assistant",
          "Bounded channels make senders wait when the buffer is full, which pushes pressure back to the producer.",
          now - 6 * DAY + MINUTE
        ),
        msg(
          "m3",
          "compaction",
          "Earlier conversation summarized",
          now - 6 * DAY + 5 * MINUTE
        ),
        msg(
          "m4",
          "assistant",
          "That covers the plan. Nice work.",
          now - 6 * DAY + 20 * MINUTE
        ),
      ],
      pending: null,
      plan: PLAN(["visited", "visited", "visited", "visited"]),
      probeProgress: { asked: 8, max: 8 },
      verdicts: [
        { callId: "call_quiz_a", correct: true },
        {
          callId: "call_quiz_b",
          correct: false,
          explanation: "Unbounded channels never apply backpressure.",
        },
      ],
    },
    {
      ...base,
      id: "c_running",
      kind: "teach",
      title: "Cancellation safety",
      phase: "teach",
      messages: [
        msg("m1", "user", "What makes a future cancel safe?", now - 2 * MINUTE),
      ],
      pending: null,
      plan: PLAN(["current", "upcoming", "upcoming", "upcoming"]),
      probeProgress: { asked: 8, max: 8 },
      jobs: [
        {
          id: "job_research_1",
          kind: "research",
          status: "running",
          title: "Research: cancel-safe APIs in tokio",
        },
      ],
      turn: {
        state: "running",
        turnId: "turn_live_1",
        startedAt: now - 90_000,
      },
    },
    {
      ...base,
      id: "c_error",
      kind: "teach",
      title: "Mutex across await",
      phase: "teach",
      messages: [
        msg(
          "m1",
          "user",
          "Why can holding a Mutex across await deadlock?",
          now - 4 * HOUR
        ),
      ],
      pending: null,
      plan: PLAN(["current", "upcoming", "upcoming", "upcoming"]),
      probeProgress: { asked: 8, max: 8 },
      turn: { state: "error", turnId: "turn_err_1", startedAt: now - 4 * HOUR },
    },
  ]
}

export function chatsForOtherWorkspaces(
  now: number
): Record<string, ClassroomView[]> {
  const make = (id: string, title: string, at: number): ClassroomView => ({
    id,
    kind: "teach",
    title,
    phase: "teach",
    viewVersion: 2,
    messages: [msg("m1", "user", title, at)],
    pending: null,
    plan: PLAN(["current", "upcoming", "upcoming", "upcoming"]),
    probeProgress: { asked: 8, max: 8 },
    verdicts: [],
    jobs: [],
    turn: { state: "idle" },
  })
  return {
    ws_distsys: [make("c_raft", "Raft leader election", now - 3 * DAY)],
    ws_designnotes: [make("c_type", "Type scale ratios", now - 20 * MINUTE)],
  }
}

export function summarize(view: ClassroomView): ChatSummary {
  const last = view.messages[view.messages.length - 1]
  return {
    id: view.id,
    kind: view.kind,
    title: view.title,
    phase: view.phase,
    updatedAt: last?.createdAt ?? 0,
    messageCount: view.messages.length,
    pending: view.pending !== null,
    jobs: view.jobs.filter((j) => j.status === "running").length,
  }
}
