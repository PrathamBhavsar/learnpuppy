import { createFileRoute } from "@tanstack/react-router"
import { Trash2Icon } from "lucide-react"

import { PlaceholderScreen } from "@/features/screens/PlaceholderScreen"

function Screen() {
  return (
    <PlaceholderScreen
      testId="screen-trash"
      title="Trash"
      description="Deleted workspaces you can still restore."
      icon={Trash2Icon}
      emptyTitle="Trash is empty"
      emptyDescription="Deleted workspaces stay here until you purge them."
    />
  )
}

export const Route = createFileRoute("/_shell/trash")({ component: Screen })
