import { MutationCache, QueryCache, QueryClient } from "@tanstack/react-query"

import { useStatus } from "@/stores/status"

import { shouldRetry, toApiError } from "./errors"

/** Routes global failure modes (expired session, no engine) into the status store. */
function noteGlobalFailure(error: unknown): void {
  const { code } = toApiError(error)
  if (code === "auth_required") useStatus.getState().setAuthRequired(true)
  if (code === "engine_unavailable")
    useStatus.getState().setEngineUnavailable(true)
}

/** Defaults from docs/web/04 §Data layer: SSE is the freshness mechanism, so no focus refetch. */
export function createQueryClient(): QueryClient {
  return new QueryClient({
    queryCache: new QueryCache({ onError: noteGlobalFailure }),
    mutationCache: new MutationCache({ onError: noteGlobalFailure }),
    defaultOptions: {
      queries: {
        staleTime: 30_000,
        gcTime: 5 * 60_000,
        retry: shouldRetry,
        refetchOnWindowFocus: false,
        networkMode: "always",
      },
      mutations: { networkMode: "always" },
    },
  })
}
