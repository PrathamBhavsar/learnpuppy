import type { components } from "@/api/schema"
import type {
  AccountDetail,
  ClassroomView,
  DecisionDetail,
  DigestDay,
  IngestRun,
  IngestSourcesWire,
  SettingsDoc,
} from "@/api/pending-types"

import { chatFixtures, chatsForOtherWorkspaces } from "./fixtures/chats"
import { accountFixtures } from "./fixtures/accounts"
import type { MockCourse } from "./fixtures/courses"
import { digestFixtures } from "./fixtures/digest"
import { type GraphFixture, buildGraph } from "./fixtures/graph"
import { ingestFixtures } from "./fixtures/ingest"
import type { MockLearning } from "./fixtures/learning"
import { type CardFixture, cardFixtures, statsFor } from "./fixtures/review"
import { settingsFixture } from "./fixtures/settings"
import {
  type MockWorkspace,
  WS_DESIGN,
  WS_DIST,
  WS_RUST,
  workspaceFixtures,
} from "./fixtures/workspaces"

export type MockDb = {
  now: number
  /** Artificial latency per request; 0 under Vitest, a few hundred ms in `dev:mock`. */
  latencyMs: number
  /** Stream assistant replies as chat.delta events (mock mode only). */
  simulateTurns: boolean
  workspaces: Map<string, MockWorkspace>
  trash: components["schemas"]["TrashEntryDto"][]
  graphs: Map<string, GraphFixture>
  chats: Map<string, ClassroomView[]>
  cards: Map<string, CardFixture[]>
  courses: Map<string, MockCourse[]>
  learning: Map<string, MockLearning>
  digest: { days: DigestDay[]; details: Map<string, DecisionDetail> }
  accounts: AccountDetail[]
  settings: SettingsDoc
  ingest: Map<string, { sources: IngestSourcesWire; runs: IngestRun[] }>
  /** clientId → stored result, so retried mutations return the original response. */
  idempotent: Map<string, unknown>
  counters: { id: number }
}

function createDb(now = Date.now()): MockDb {
  const workspaces = new Map(workspaceFixtures(now).map((w) => [w.id, w]))
  const graphs = new Map<string, GraphFixture>([
    [WS_RUST, buildGraph(WS_RUST, 150, now, 7)],
    [WS_DIST, buildGraph(WS_DIST, 40, now, 11)],
    [WS_DESIGN, buildGraph(WS_DESIGN, 28, now, 13)],
  ])
  const chats = new Map<string, ClassroomView[]>([
    [WS_RUST, chatFixtures(now)],
    ...Object.entries(chatsForOtherWorkspaces(now)),
  ])
  const cards = new Map<string, CardFixture[]>([
    [WS_RUST, cardFixtures(WS_RUST, now, 7, 21)],
    [WS_DIST, cardFixtures(WS_DIST, now, 0, 22)],
    [WS_DESIGN, cardFixtures(WS_DESIGN, now, 12, 23)],
  ])
  const ingest = new Map(
    [WS_RUST, WS_DIST, WS_DESIGN].map((id) => [id, ingestFixtures(now)])
  )
  return {
    now,
    latencyMs: 0,
    simulateTurns: false,
    workspaces,
    trash: [],
    graphs,
    chats,
    cards,
    courses: new Map(),
    learning: new Map(),
    digest: digestFixtures(now),
    accounts: accountFixtures(now),
    settings: settingsFixture(),
    ingest,
    idempotent: new Map(),
    counters: { id: 100 },
  }
}

export const state: { db: MockDb } = { db: createDb() }

export function resetDb(overrides: Partial<MockDb> = {}): MockDb {
  state.db = { ...createDb(), ...overrides }
  return state.db
}

export function nextId(prefix: string): string {
  state.db.counters.id += 1
  return `${prefix}_${state.db.counters.id}`
}

function dueCount(ws: string): number {
  const cards = state.db.cards.get(ws) ?? []
  return cards.filter((c) => c.due <= state.db.now).length
}

/** Live stats derived from the stored graph/chats/cards so mutations show up in the lists. */
export function listItem(
  ws: MockWorkspace
): components["schemas"]["WorkspaceDto"] {
  const graph = state.db.graphs.get(ws.id)
  const chats = state.db.chats.get(ws.id) ?? []
  const wire: Omit<MockWorkspace, "damaged"> & { damaged?: boolean } = {
    ...ws,
  }
  delete wire.damaged // the wire has no such field; damaged workspaces are listed separately
  return {
    ...wire,
    stats: {
      nodes: graph?.snapshot.nodes.length ?? ws.stats.nodes,
      chains: graph?.snapshot.chains.length ?? ws.stats.chains,
      chats: chats.length,
      dueCards: dueCount(ws.id),
      lastActivityAt: ws.stats.lastActivityAt,
    },
  }
}

export function reviewStats(ws: string): ReturnType<typeof statsFor> {
  return statsFor(state.db.cards.get(ws) ?? [], state.db.now)
}
