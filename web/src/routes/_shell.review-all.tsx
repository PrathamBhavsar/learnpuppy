import { createFileRoute } from "@tanstack/react-router"
import { LayersIcon } from "lucide-react"

import { PlaceholderScreen } from "@/features/screens/PlaceholderScreen"

function Screen() {
  return (
    <PlaceholderScreen
      testId="screen-review-all"
      title="Review all"
      description="Due cards across every workspace, one after another."
      icon={LayersIcon}
      emptyTitle="Nothing due anywhere"
      emptyDescription="Every workspace is caught up."
    />
  )
}

export const Route = createFileRoute("/_shell/review-all")({
  component: Screen,
})
