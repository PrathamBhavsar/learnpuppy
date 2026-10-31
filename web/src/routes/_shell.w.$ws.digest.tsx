import { createFileRoute } from "@tanstack/react-router"
import { NewspaperIcon } from "lucide-react"

import { PlaceholderScreen } from "@/features/screens/PlaceholderScreen"
import { useWorkspaceName } from "@/hooks/use-workspace"

function Screen() {
  const { ws } = Route.useParams()
  return (
    <PlaceholderScreen
      testId="screen-digest"
      title="Digest"
      description="What your agents decided, by day."
      icon={NewspaperIcon}
      emptyTitle="Nothing to digest yet"
      emptyDescription="Run an ingest to see decisions from your recent sessions."
      actionLabel="Go to ingest"
      workspace={useWorkspaceName(ws)}
    />
  )
}

export const Route = createFileRoute("/_shell/w/$ws/digest")({
  component: Screen,
})
