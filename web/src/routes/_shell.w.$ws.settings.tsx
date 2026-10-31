import { createFileRoute } from "@tanstack/react-router"
import { SlidersHorizontalIcon } from "lucide-react"

import { PlaceholderScreen } from "@/features/screens/PlaceholderScreen"
import { useWorkspaceName } from "@/hooks/use-workspace"

function Screen() {
  const { ws } = Route.useParams()
  return (
    <PlaceholderScreen
      testId="screen-settings"
      title="Workspace settings"
      description="Name, sources and model overrides for this workspace."
      icon={SlidersHorizontalIcon}
      emptyTitle="Settings load with the workspace"
      emptyDescription="General, sources and the danger zone arrive with the workspace screens."
      actionLabel={undefined}
      workspace={useWorkspaceName(ws)}
    />
  )
}

export const Route = createFileRoute("/_shell/w/$ws/settings")({
  component: Screen,
})
