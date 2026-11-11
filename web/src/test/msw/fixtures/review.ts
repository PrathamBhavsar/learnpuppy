import type { components } from "@/api/schema"

import { DAY, HOUR, mulberry32 } from "./rng"

type Schemas = components["schemas"]
type CardState = Schemas["CardState"]

/** A stored card: the wire `CardDetailDto` fields in one flat record; handlers slice what each endpoint serves. */
export type CardFixture = {
  id: string
  front: string
  options: Schemas["ChoiceDto"][]
  kind: string
  sourceDecisionId: string
  due: number
  state: CardState
  stability: number
  difficulty: number
  reps: number
  lapses: number
  correctKey: string
  explanation: string
  /** Recall cards are typed; `answer` is matched exactly here (the server uses a rubric). */
  answerMode?: "mcq" | "recall"
  answer?: string
}

const QUESTIONS: {
  front: string
  options: string[]
  correct: number
  why: string
}[] = [
  {
    front: "Why did the team pin tokio to the multi-threaded scheduler?",
    options: [
      "Work stealing evens out uneven task loads",
      "current_thread cannot run async code",
      "It removes the need for Send bounds",
      "It disables the I/O driver overhead",
    ],
    correct: 0,
    why: "Work stealing keeps all workers busy when task cost is uneven.",
  },
  {
    front: "Which channel was chosen for the shutdown signal?",
    options: [
      "mpsc with capacity 1",
      "watch, because only the latest value matters",
      "broadcast with a large buffer",
      "oneshot per worker",
    ],
    correct: 1,
    why: "A shutdown flag only needs the latest value, which is what watch keeps.",
  },
  {
    front: "What was the reason to avoid std::sync::Mutex in the handler?",
    options: [
      "It is slower than the tokio Mutex always",
      "Holding the guard across await can block the executor thread",
      "It cannot be shared across threads",
      "It does not support poisoning",
    ],
    correct: 1,
    why: "A std guard held over an await blocks a worker thread and risks deadlock.",
  },
  {
    front: "How was cancellation handled in the request pipeline?",
    options: [
      "Aborting the JoinHandle on every timeout",
      "A CancellationToken passed down the task tree",
      "Polling a boolean flag in each loop",
      "Dropping the runtime on error",
    ],
    correct: 1,
    why: "Tokens propagate shutdown through child tasks cooperatively.",
  },
  {
    front: "Why was spawn_blocking used for the file hash?",
    options: [
      "Hashing is async-safe",
      "File I/O and CPU work would otherwise stall the async workers",
      "It is required for Send futures",
      "It gives lower latency than spawn",
    ],
    correct: 1,
    why: "Blocking work belongs on the blocking pool so workers keep polling.",
  },
  {
    front: "What did the design choose for graph layout?",
    options: [
      "Server-side ForceAtlas2 with cached positions",
      "Client-side d3-force on every load",
      "A static grid by creation date",
      "Dagre for the whole graph",
    ],
    correct: 0,
    why: "Layout is domain logic in Rust; the client only renders cached positions.",
  },
  {
    front: "Why are wakers cloned instead of borrowed?",
    options: [
      "Borrowing is unsafe",
      "A task may need to be woken after poll returns",
      "Clones are zero-cost",
      "The executor requires unique wakers",
    ],
    correct: 1,
    why: "The waker outlives the poll call and is stored for later wake-ups.",
  },
  {
    front: "Which structure holds timers in tokio?",
    options: [
      "A binary heap",
      "A hierarchical timing wheel",
      "A sorted vector",
      "A skip list",
    ],
    correct: 1,
    why: "A hierarchical wheel gives O(1) insert and cancel.",
  },
]

const STATES: CardState[] = ["new", "learning", "review", "relearning"]

/** 30 cards for a workspace: `due` of them due now, the rest spread over the next two weeks. */
export function cardFixtures(
  wsId: string,
  now: number,
  due: number,
  seed: number
): CardFixture[] {
  const rand = mulberry32(seed)
  return Array.from({ length: 30 }, (_, i) => {
    const q = QUESTIONS[i % QUESTIONS.length]!
    const keys = ["a", "b", "c", "d"]
    const state = STATES[i % STATES.length]!
    const isDue = i < due
    return {
      id: `card_${wsId.slice(3)}_${String(i + 1).padStart(3, "0")}`,
      front: q.front,
      options: q.options.map((text, k) => ({ key: keys[k]!, text })),
      kind: "decision",
      sourceDecisionId: `dec_${(i % 6) + 1}`,
      due: isDue ? now - (1 + i) * HOUR : now + Math.ceil(rand() * 14) * DAY,
      state,
      stability: Math.round((1 + rand() * 20) * 10) / 10,
      difficulty: Math.round((3 + rand() * 6) * 10) / 10,
      reps: state === "new" ? 0 : Math.floor(rand() * 8) + 1,
      lapses: Math.floor(rand() * 3),
      correctKey: keys[q.correct]!,
      explanation: q.why,
    }
  })
}

export function statsFor(
  cards: CardFixture[],
  now: number
): Schemas["StatsDto"] {
  const dueNow = cards.filter((c) => c.due <= now).length
  const forecast = Array.from(
    { length: 14 },
    (_, d) =>
      cards.filter((c) => c.due > now + d * DAY && c.due <= now + (d + 1) * DAY)
        .length
  )
  return {
    dueNow,
    dueToday: dueNow + forecast[0]!,
    newToday: 3,
    streak: 6,
    reviewedToday: 4,
    retention30d: 0.87,
    forecast,
    forecastStart: now,
    now,
  }
}
