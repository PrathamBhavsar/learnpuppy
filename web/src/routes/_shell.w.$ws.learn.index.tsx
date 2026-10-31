import { createFileRoute } from "@tanstack/react-router"

import { StartLearning } from "@/features/classroom/StartLearning"

function LearnIndex() {
  const { ws } = Route.useParams()
  return <StartLearning ws={ws} />
}

export const Route = createFileRoute("/_shell/w/$ws/learn/")({
  component: LearnIndex,
})
