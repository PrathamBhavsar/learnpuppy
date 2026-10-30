import { Outlet, createFileRoute } from "@tanstack/react-router"

import { PageHeader } from "@/components/app/page-header"
import { usePageTitle } from "@/hooks/use-page-title"
import { findWorkspace } from "@/lib/static-workspaces"

function Learn() {
  const { ws } = Route.useParams()
  usePageTitle("Learn", findWorkspace(ws)?.name)
  return (
    <div data-testid="screen-learn" className="flex h-full min-h-0">
      <aside
        aria-label="Chats"
        className="lp-pane hidden w-70 shrink-0 flex-col gap-4 border-e p-4 md:flex"
      >
        <PageHeader title="Learn" description="Teach and practice chats." />
        <p className="text-[13px] text-muted-foreground">No chats yet.</p>
      </aside>
      <div className="lp-pane min-w-0 flex-1">
        <Outlet />
      </div>
    </div>
  )
}

export const Route = createFileRoute("/_shell/w/$ws/learn")({
  component: Learn,
})
