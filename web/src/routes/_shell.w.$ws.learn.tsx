import { Outlet, createFileRoute, useParams } from "@tanstack/react-router"

import { ChatList } from "@/features/classroom/ChatList"
import { usePageTitle } from "@/hooks/use-page-title"
import { useWorkspaceName } from "@/hooks/use-workspace"

function Learn() {
  const { ws } = Route.useParams()
  const { chat } = useParams({ strict: false })
  usePageTitle("Learn", useWorkspaceName(ws))
  return (
    <div data-testid="screen-learn" className="flex h-full min-h-0">
      <aside
        aria-label="Chats"
        className="lp-pane hidden w-70 shrink-0 flex-col gap-4 border-e p-4 md:flex"
      >
        <ChatList ws={ws} activeChatId={chat} />
      </aside>
      <div className="min-h-0 min-w-0 flex-1 overflow-hidden">
        <Outlet />
      </div>
    </div>
  )
}

export const Route = createFileRoute("/_shell/w/$ws/learn")({
  component: Learn,
})
