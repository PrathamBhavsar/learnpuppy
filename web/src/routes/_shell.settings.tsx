import { createFileRoute } from "@tanstack/react-router"
import { SettingsIcon } from "lucide-react"

import { PlaceholderScreen } from "@/features/screens/PlaceholderScreen"

function Screen() {
  return (
    <PlaceholderScreen
      testId="screen-settings"
      title="Settings"
      description="Appearance, behavior, shortcuts and data."
      icon={SettingsIcon}
      emptyTitle="Settings load here"
      emptyDescription="Theme, direction, motion and concurrency controls arrive with the settings screen."
    />
  )
}

export const Route = createFileRoute("/_shell/settings")({ component: Screen })
