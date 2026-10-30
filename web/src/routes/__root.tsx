import {
  Link,
  Outlet,
  createRootRouteWithContext,
} from "@tanstack/react-router"
import type { QueryClient } from "@tanstack/react-query"
import { CompassIcon } from "lucide-react"

import { EmptyState } from "@/components/app/empty-state"
import { Button } from "@/components/ui/button"
import { CommandPalette } from "@/features/palette/CommandPalette"

export type RouterContext = { queryClient: QueryClient }

function NotFound() {
  return (
    <div className="grid h-dvh place-items-center">
      <EmptyState
        icon={CompassIcon}
        title="Page not found"
        description="That address does not match any screen."
        action={
          <Button asChild variant="outline">
            <Link to="/">Back to workspaces</Link>
          </Button>
        }
      />
    </div>
  )
}

export const Route = createRootRouteWithContext<RouterContext>()({
  component: () => (
    <>
      <Outlet />
      <CommandPalette />
    </>
  ),
  notFoundComponent: NotFound,
})
