import { createFileRoute } from "@tanstack/react-router"
import { NetworkIcon } from "lucide-react"

import { PlaceholderScreen } from "@/features/screens/PlaceholderScreen"
import { findWorkspace } from "@/lib/static-workspaces"

function Screen() {
  const { ws } = Route.useParams()
  return (
    <PlaceholderScreen
      testId="screen-graph"
      title="Graph"
      description="Propositions and the chains between them."
      icon={NetworkIcon}
      emptyTitle="No propositions yet"
      emptyDescription="The graph fills as you learn in chats and ingest sessions."
      actionLabel="Start learning"
      workspace={findWorkspace(ws)?.name}
    />
  )
}

export const Route = createFileRoute("/_shell/w/$ws/graph")({
  component: Screen,
})
