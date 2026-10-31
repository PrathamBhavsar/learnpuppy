// URL state for the graph screen: ?node=<id>&q=<text>&src=claude,codex&cluster=<id> (docs/web/06).
import { getRouteApi } from "@tanstack/react-router"
import { useCallback, useMemo } from "react"

import type { NodeSource } from "@/api/pending-types"

import { parseSources, serializeSources } from "./model/filters"

export type GraphSearch = {
  node?: string
  q?: string
  src?: string
  cluster?: string
  dir?: string
}

const asString = (v: unknown): string | undefined =>
  typeof v === "string" && v.length > 0 ? v : undefined

export function validateGraphSearch(raw: Record<string, unknown>): GraphSearch {
  return {
    node: asString(raw["node"]),
    q: asString(raw["q"]),
    src: asString(raw["src"]),
    cluster: asString(raw["cluster"]),
    // Kept so `?dir=rtl` (tests, screenshots) survives in-app navigation.
    dir: asString(raw["dir"]),
  }
}

const route = getRouteApi("/_shell/w/$ws/graph")

export type GraphUrl = {
  node: string | null
  q: string
  sources: NodeSource[]
  cluster: string | null
  setNode: (id: string | null) => void
  setQuery: (q: string) => void
  setSources: (sources: readonly NodeSource[]) => void
  setCluster: (id: string | null) => void
  clearFilters: () => void
}

export function useGraphUrl(): GraphUrl {
  const search = route.useSearch()
  const navigate = route.useNavigate()

  const patch = useCallback(
    (next: Partial<GraphSearch>, replace: boolean) =>
      void navigate({
        search: (prev: GraphSearch) => ({ ...prev, ...next }),
        replace,
        resetScroll: false,
      }),
    [navigate]
  )

  const sources = useMemo(() => parseSources(search.src), [search.src])
  return {
    node: search.node ?? null,
    q: search.q ?? "",
    sources,
    cluster: search.cluster ?? null,
    // Picking a node adds a history entry so Back closes the drawer; filters replace.
    setNode: useCallback((id) => patch({ node: id ?? undefined }, false), [patch]),
    setQuery: useCallback((q) => patch({ q: q.trim() ? q : undefined }, true), [patch]),
    setSources: useCallback((s) => patch({ src: serializeSources(s) }, true), [patch]),
    setCluster: useCallback((id) => patch({ cluster: id ?? undefined }, true), [patch]),
    clearFilters: useCallback(() => patch({ src: undefined, cluster: undefined }, true), [patch]),
  }
}
