import { createFileRoute } from "@tanstack/react-router"

import { AccountsScreen } from "@/features/accounts/AccountsScreen"

export const Route = createFileRoute("/_shell/accounts")({
  component: AccountsScreen,
})
