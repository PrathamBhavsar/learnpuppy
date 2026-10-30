import { createFileRoute } from "@tanstack/react-router"
import { UsersRoundIcon } from "lucide-react"

import { PlaceholderScreen } from "@/features/screens/PlaceholderScreen"

function Screen() {
  return (
    <PlaceholderScreen
      testId="screen-accounts"
      title="Accounts"
      description="Model accounts discovered on this machine and the roles they fill."
      icon={UsersRoundIcon}
      emptyTitle="No accounts scanned yet"
      emptyDescription="Detected Claude, Codex and Antigravity profiles appear here."
    />
  )
}

export const Route = createFileRoute("/_shell/accounts")({ component: Screen })
