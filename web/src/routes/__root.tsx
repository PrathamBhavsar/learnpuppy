import {
  Link,
  Outlet,
  createRootRouteWithContext,
  useNavigate,
  useParams,
} from "@tanstack/react-router"
import { useMemo } from "react"
import type { QueryClient } from "@tanstack/react-query"
import { CompassIcon } from "lucide-react"

import { EmptyState } from "@/components/app/empty-state"
import { Button } from "@/components/ui/button"
import { Tour } from "@/components/app/tour/Tour"
import { CommandPalette } from "@/features/palette/CommandPalette"
import { AuthRequired } from "@/features/shell/AuthRequired"
import { ShortcutsSheet } from "@/features/shell/ShortcutsSheet"
import { useServerEvents } from "@/hooks/use-server-events"
import { notifyExportReady } from "@/features/workspaces/exports"
import { toastInfo } from "@/lib/toast"
import { useStatus } from "@/stores/status"

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

/** One event stream per tab, scoped to the workspace in the URL (docs/web/03 §SSE). */
function Root() {
  const { ws } = useParams({ strict: false })
  const navigate = useNavigate()
  const authRequired = useStatus((s) => s.authRequired)
  const ui = useMemo(
    () => ({
      workspaceDeleted: (id: string) => {
        if (id === ws) {
          toastInfo("This workspace was deleted")
          void navigate({ to: "/" })
        }
      },
      exportReady: notifyExportReady,
    }),
    [ws, navigate]
  )
  useServerEvents(ws ?? null, ui)
  return (
    <>
      <Outlet />
      <CommandPalette />
      <ShortcutsSheet />
      <Tour />
      {authRequired ? <AuthRequired /> : null}
    </>
  )
}

export const Route = createRootRouteWithContext<RouterContext>()({
  component: Root,
  notFoundComponent: NotFound,
})
