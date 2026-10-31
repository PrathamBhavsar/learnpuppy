import { createFileRoute } from "@tanstack/react-router"

import { TrashScreen } from "@/features/workspaces/TrashScreen"

export const Route = createFileRoute("/_shell/trash")({
  component: TrashScreen,
})
