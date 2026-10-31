import { createFileRoute } from "@tanstack/react-router"
import { LayersIcon } from "lucide-react"

import { PlaceholderScreen } from "@/features/screens/PlaceholderScreen"
import { useWorkspaceName } from "@/hooks/use-workspace"

function Screen() {
  const { ws } = Route.useParams()
  return (
    <PlaceholderScreen
      testId="screen-review"
      title="Review"
      description="Spaced repetition cards due today."
      icon={LayersIcon}
      emptyTitle="All caught up"
      emptyDescription="Cards appear here once decisions are ingested or a chat produces them."
      actionLabel="Browse cards"
      workspace={useWorkspaceName(ws)}
    />
  )
}

export const Route = createFileRoute("/_shell/w/$ws/review")({
  component: Screen,
})
