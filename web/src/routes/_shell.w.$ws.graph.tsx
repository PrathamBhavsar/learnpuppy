import { createFileRoute } from "@tanstack/react-router"
import { NetworkIcon } from "lucide-react"

import { PlaceholderScreen } from "@/features/screens/PlaceholderScreen"
import { useWorkspaceName } from "@/hooks/use-workspace"

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
      workspace={useWorkspaceName(ws)}
    />
  )
}

export const Route = createFileRoute("/_shell/w/$ws/graph")({
  component: Screen,
})
