import { createFileRoute } from "@tanstack/react-router"

import { ScheduleScreen } from "@/features/learning/ScheduleScreen"

function Screen() {
  const { ws } = Route.useParams()
  return <ScheduleScreen ws={ws} />
}

export const Route = createFileRoute("/_shell/w/$ws/schedule")({
  component: Screen,
})
