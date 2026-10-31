import type { CardState } from "./pending-types"

/**
 * Typed query-key factory (docs/web/04 §Data layer). Every workspace-scoped key starts with
 * `['ws', id]`, so one `invalidateQueries({ queryKey: keys.ws(id).all })` covers the lot and
 * switching workspaces never shows another workspace's data.
 */
export const keys = {
  bootstrap: () => ["bootstrap"] as const,
  accounts: () => ["accounts"] as const,
  settings: () => ["settings"] as const,
  settingsCatalog: () => ["settings", "catalog"] as const,
  trash: () => ["trash"] as const,
  workspaces: (archived = false) => ["workspaces", { archived }] as const,
  /** Prefix covering both archived and live lists. */
  workspacesAll: () => ["workspaces"] as const,
  search: (q: string) => ["search", q] as const,
  ws: (id: string) => {
    const base = ["ws", id] as const
    return {
      all: base,
      detail: () => [...base, "detail"] as const,
      graph: () => [...base, "graph"] as const,
      graphSnapshot: () => [...base, "graph", "snapshot"] as const,
      graphNode: (nodeId: string) =>
        [...base, "graph", "node", nodeId] as const,
      graphSearch: (q: string) => [...base, "graph", "search", q] as const,
      graphChains: (selection: readonly string[]) =>
        [...base, "graph", "chains", [...selection].sort()] as const,
      chats: () => [...base, "chats"] as const,
      chat: (chatId: string) => [...base, "chat", chatId] as const,
      jobs: () => [...base, "jobs"] as const,
      review: () => [...base, "review"] as const,
      reviewQueue: (limit?: number) =>
        [...base, "review", "queue", { limit: limit ?? null }] as const,
      reviewStats: () => [...base, "review", "stats"] as const,
      reviewPreview: (cardId: string) =>
        [...base, "review", "preview", cardId] as const,
      reviewCards: (filter: { state?: CardState; q?: string } = {}) =>
        [
          ...base,
          "review",
          "cards",
          { state: filter.state ?? null, q: filter.q ?? "" },
        ] as const,
      digest: (range: { from?: string; to?: string; source?: string } = {}) =>
        [
          ...base,
          "digest",
          {
            from: range.from ?? null,
            to: range.to ?? null,
            source: range.source ?? null,
          },
        ] as const,
      digestAll: () => [...base, "digest"] as const,
      decision: (id: string) => [...base, "digest", "decision", id] as const,
      ingestSources: () => [...base, "ingest", "sources"] as const,
      ingestRuns: () => [...base, "ingest", "runs"] as const,
      ingest: () => [...base, "ingest"] as const,
    }
  },
}
