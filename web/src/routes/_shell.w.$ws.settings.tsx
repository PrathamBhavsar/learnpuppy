import { createFileRoute } from "@tanstack/react-router"

import { WorkspaceSettingsScreen } from "@/features/settings/WorkspaceSettingsScreen"

function Screen() {
  const { ws } = Route.useParams()
  return <WorkspaceSettingsScreen ws={ws} />
}

export const Route = createFileRoute("/_shell/w/$ws/settings")({
  component: Screen,
})
