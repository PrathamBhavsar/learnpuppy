import { createRouter } from "@tanstack/react-router"

import { createQueryClient } from "./api/query-client"
import { RouteError } from "./features/shell/RouteError"
import { routeTree } from "./routeTree.gen"

export const queryClient = createQueryClient()

export const router = createRouter({
  routeTree,
  context: { queryClient },
  defaultPreload: "intent",
  scrollRestoration: true,
  defaultErrorComponent: RouteError,
})

declare module "@tanstack/react-router" {
  interface Register {
    router: typeof router
  }
}
