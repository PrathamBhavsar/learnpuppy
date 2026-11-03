import type { ChatSummary, ClassroomView } from "@/api/pending-types"

import { DAY, HOUR, MINUTE } from "./rng"

type ClassroomPlanNode = NonNullable<ClassroomView["plan"]>["nodes"][number]

/** Job output served by the mock `GET …/artifacts/:id`. */
export const ARTIFACTS: Record<string, { type: string; body: string }> = {
  art_research_1: {
    type: "text/markdown",
    body: "# Cancel-safe APIs in tokio\n\n- `mpsc::Receiver::recv` is cancel safe.\n- `AsyncReadExt::read_exact` is **not**: bytes may be lost.\n\n| API | Cancel safe |\n|---|---|\n| `recv` | yes |\n| `read_exact` | no |\n",
  },
  art_visual_1: {
    type: "text/html",
    body: '<!doctype html><html><body style="font-family:monospace;padding:16px"><h3>Poll loop</h3><p>executor -> poll -> Pending -> waker -> poll</p></body></html>',
  },
}

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
    {
      id: "p1",
      title: "What a future is",
      summary: "A future is a lazy value describing unfinished work.",
      status: statuses[0] ?? "upcoming",
    },
    {
      id: "p2",
      title: "Polling and wakers",
      summary: "poll returns Pending after storing the waker.",
      status: statuses[1] ?? "upcoming",
    },
    {
      id: "p3",
      title: "Pinning",
      summary: "Self-referential futures must not move once polled.",
      status: statuses[2] ?? "upcoming",
    },
    {
      id: "p4",
      title: "Executors",
      summary: "An executor owns tasks and polls them when woken.",
      status: statuses[3] ?? "upcoming",
    },
  ] as ClassroomPlanNode[],
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
        multiSelect: false,
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
          "Quick check first. Is a future eager or lazy?",
          now - 2 * DAY + 3 * MINUTE,
          { calls: [{ id: "call_quiz_1", name: "quiz" }] }
        ),
        msg(
          "m5",
          "user",
          "B. Lazy: nothing runs until polled",
          now - 2 * DAY + 4 * MINUTE,
          {
            answersCallId: "call_quiz_1",
          }
        ),
        msg(
          "m6",
          "assistant",
          "Let me check the idea landed.",
          now - 2 * DAY + 5 * MINUTE,
          { calls: [{ id: "call_quiz_2", name: "quiz" }] }
        ),
      ],
      jobs: [
        {
          id: "job_research_done",
          kind: "research",
          status: "done",
          title: "Research: cancel-safe APIs in tokio",
          artifactId: "art_research_1",
        },
        {
          id: "job_visual_done",
          kind: "visual",
          status: "done",
          title: "Diagram: the poll loop",
          artifactId: "art_visual_1",
        },
        {
          id: "job_pdf_failed",
          kind: "pdf",
          status: "failed",
          title: "Reading async-book.pdf",
          artifactId: null,
        },
      ],
      pending: {
        callId: "call_quiz_2",
        tool: "quiz",
        prompt: "What does poll return when a future is not finished?",
        options: [
          { key: "pending", text: "Poll::Pending, after registering a waker" },
          { key: "ready-none", text: "Poll::Ready(None)" },
          {
            key: "blocks",
            text: "It blocks the thread until the future is ready",
          },
          { key: "panics", text: "It panics unless the future was pinned" },
        ],
        allowFreeText: false,
        multiSelect: false,
        confidenceScale: 5,
        idkKey: "__idk__",
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
        multiSelect: false,
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
      turn: {
        state: "error",
        turnId: "turn_err_1",
        startedAt: now - 4 * HOUR,
        clientId: "client_err_1",
        error: {
          code: "engine_unavailable",
          message: "No model account answered. Check Accounts.",
          retryable: true,
          interrupted: false,
        },
      },
    },
    {
      ...base,
      id: "c_limited",
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
      turn: {
        state: "error",
        turnId: "turn_lim_1",
        startedAt: now - 4 * HOUR,
        clientId: "client_lim_1",
        error: {
          code: "RATE_LIMITED",
          message: "limited",
          provider: "claude",
          resetsAt: now + HOUR,
          retryable: true,
          interrupted: false,
        },
      },
    },
    {
      ...base,
      id: "c_queued",
      kind: "teach",
      title: "Send and Sync",
      phase: "teach",
      messages: [
        msg("m1", "user", "When is a type Send but not Sync?", now - MINUTE),
      ],
      pending: null,
      plan: PLAN(["current", "upcoming", "upcoming", "upcoming"]),
      probeProgress: { asked: 8, max: 8 },
      turn: {
        state: "queued",
        turnId: "turn_q_1",
        startedAt: now - MINUTE,
        clientId: "client_q_1",
        retryAt: now + 42_000,
        error: {
          code: "rate_limited",
          message: "Every model account is cooling down.",
          retryable: true,
          retryAfterMs: 42_000,
          interrupted: false,
        },
      },
    },
    {
      ...base,
      id: "c_interrupted",
      kind: "teach",
      title: "Drop order",
      phase: "teach",
      messages: [
        msg(
          "m1",
          "user",
          "In what order are struct fields dropped?",
          now - HOUR
        ),
        msg(
          "m2",
          "assistant",
          "Fields drop in declaration order, and",
          now - HOUR + MINUTE,
          {
            interrupted: true,
          }
        ),
      ],
      pending: null,
      plan: PLAN(["current", "upcoming", "upcoming", "upcoming"]),
      probeProgress: { asked: 8, max: 8 },
      turn: {
        state: "error",
        turnId: "turn_int_1",
        clientId: "client_int_1",
        error: {
          code: "interrupted",
          message: "The server stopped during this turn. Resume to continue.",
          retryable: true,
          interrupted: true,
        },
      },
    },
    {
      ...base,
      id: "c_practice_done",
      kind: "practice",
      title: "Practice: lifetimes",
      phase: "done",
      messages: [
        msg(
          "m1",
          "assistant",
          "Last problem: return the longer of two strings.",
          now - 8 * HOUR,
          {
            calls: [{ id: "call_problem_9", name: "problem" }],
          }
        ),
        msg(
          "m2",
          "user",
          "fn longest<'a>(a: &'a str, b: &'a str) -> &'a str",
          now - 8 * HOUR + MINUTE,
          {
            answersCallId: "call_problem_9",
          }
        ),
        msg(
          "m3",
          "tool",
          "Model solution: both inputs share lifetime 'a.",
          now - 8 * HOUR + 2 * MINUTE
        ),
      ],
      pending: null,
      plan: null,
      probeProgress: { asked: 0, max: 8 },
      problemResult: {
        callId: "call_problem_9",
        outcome: "solved",
        verdict:
          "You tied both borrows to one lifetime, which is exactly right.",
        rung: "r2",
        missClass: null,
        retest: false,
      },
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
    pending: view.pending != null,
    jobs: view.jobs.filter((j) => j.status === "running").length,
  }
}
