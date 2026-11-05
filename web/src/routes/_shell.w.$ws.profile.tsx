import { createFileRoute } from "@tanstack/react-router"

import { ProfileScreen } from "@/features/learning/ProfileScreen"

function Screen() {
  const { ws } = Route.useParams()
  return <ProfileScreen ws={ws} />
}

export const Route = createFileRoute("/_shell/w/$ws/profile")({
  component: Screen,
})
