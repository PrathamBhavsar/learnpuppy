import type { components } from "@/api/schema"

type Dto = components["schemas"]

/** A stored book as the mock "server" keeps it: the DTO plus the answer key the DTOs never carry. */
export type MockBook = {
  book: Dto["TextbookDto"]
  createdAt: number
  job: Dto["JobItemDto"] | null
  /** A test can freeze the import where it is. */
  paused?: boolean
  /** Wall-clock time of the last import step; steps are paced so progress is visible. */
  tickAt?: number
  /** Items by chapter index; `answer` and `why` stay here and only leave through feedback. */
  items: Map<number, MockItem[]>
  checks: Map<string, { chapter: number; answered: Set<string> }>
}

type MockItem = Dto["CheckItemDto"] & { answer: string; why: string }

type ChapterSeed = {
  title: string
  start: number
  end: number
  state?: Dto["ChapterStateDto"]
  status?: Dto["ChapterStatusDto"]
  kind?: Dto["ChapterKindDto"]
  error?: string
  /** Defaults to true for pending and failed; false models a chapter whose retry is spent. */
  canImport?: boolean
}

export function chapterRow(
  index: number,
  seed: ChapterSeed
): Dto["ChapterRowDto"] {
  const state = seed.state ?? "ready"
  return {
    index,
    title: seed.title,
    startPage: seed.start,
    endPage: seed.end,
    kind: seed.kind ?? "chapter",
    state,
    status: seed.status ?? "not_read",
    objectives: state === "ready" ? 3 : 0,
    error: seed.error ?? null,
    truncated: false,
    canImport: seed.canImport ?? (state === "pending" || state === "failed"),
  }
}

export const READY_CHAPTERS: ChapterSeed[] = [
  { title: "Futures and polling", start: 9, end: 40, status: "due" },
  { title: "Wakers and executors", start: 41, end: 88, status: "checked" },
  { title: "Pinning", start: 89, end: 120 },
]

function itemsFor(chapter: number): MockItem[] {
  return [
    {
      id: `ti_${chapter}_1`,
      objectiveId: `to_${chapter}_1`,
      kind: "mcq",
      q: `Chapter ${chapter}: what does a future do until it is polled?`,
      options: ["Nothing", "Runs on its own thread", "Blocks the caller"],
      answer: "Nothing",
      why: "A future is inert until something polls it.",
    },
    {
      id: `ti_${chapter}_2`,
      objectiveId: `to_${chapter}_2`,
      kind: "recall",
      q: `Chapter ${chapter}: name the method an executor calls on a future.`,
      options: [],
      answer: "poll",
      why: "Executors call `poll` with a waker.",
    },
  ]
}

export function newBook(
  id: string,
  title: string,
  chapters: ChapterSeed[],
  now: number,
  job: Dto["JobItemDto"] | null = null
): MockBook {
  const rows = chapters.map((c, i) => chapterRow(i, c))
  return {
    createdAt: now,
    job,
    items: new Map(rows.map((r) => [r.index, itemsFor(r.index)])),
    checks: new Map(),
    book: {
      id,
      title,
      fileName: `${id}.pdf`,
      totalPages: Math.max(...chapters.map((c) => c.end)) + 4,
      pageOffset: 8,
      mapSource: "outline",
      jobId: job?.id ?? null,
      autoCalls: rows.filter((r) => r.state === "ready").length,
      requestedCalls: 0,
      hourUsed: 0,
      hourCap: 8,
      chapters: rows,
    },
  }
}

export function summary(b: MockBook): Dto["TextbookSummaryDto"] {
  const chapters = b.book.chapters
  const ready = chapters.filter((c) => c.state === "ready").length
  const running = b.job?.status === "running"
  const failed = chapters.some((c) => c.state === "failed")
  return {
    id: b.book.id,
    title: b.book.title,
    totalPages: b.book.totalPages,
    chapters: chapters.length,
    ready,
    createdAt: b.createdAt,
    state: running
      ? "importing"
      : ready === chapters.length
        ? "ready"
        : ready === 0 && failed
          ? "failed"
          : "partial",
  }
}

/** The three chapters an upload turns into, finished one per poll. */
export const UPLOAD_CHAPTERS: ChapterSeed[] = [
  { title: "Introduction", start: 1, end: 12 },
  { title: "Ownership", start: 13, end: 30 },
  { title: "Traits", start: 31, end: 48 },
]
