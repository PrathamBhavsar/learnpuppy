import { useQuery } from "@tanstack/react-query"

import { api, unwrap } from "@/api/client"

/** `GET /health`: server version, engine mode and data directory. */
export function useHealth() {
  return useQuery({
    queryKey: ["health"] as const,
    queryFn: async ({ signal }) => unwrap(await api.GET("/health", { signal })),
    staleTime: 60_000,
  })
}
