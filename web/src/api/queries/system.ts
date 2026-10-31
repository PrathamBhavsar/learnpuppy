import { useQuery } from "@tanstack/react-query"

import { api, unwrap } from "../client"
import { keys } from "../keys"

/** One call at app start (docs/web/03 §Bootstrap); also the auth probe for the shell. */
export function useBootstrap() {
  return useQuery({
    queryKey: keys.bootstrap(),
    queryFn: async ({ signal }) =>
      unwrap(await api.GET("/bootstrap", { signal })),
    staleTime: 60_000,
  })
}
