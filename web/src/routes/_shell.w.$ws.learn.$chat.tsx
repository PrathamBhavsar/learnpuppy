import { createFileRoute } from "@tanstack/react-router"

import { Classroom } from "@/features/classroom/Classroom"

function Chat() {
  const { ws, chat } = Route.useParams()
  return <Classroom ws={ws} chatId={chat} />
}

export const Route = createFileRoute("/_shell/w/$ws/learn/$chat")({
  component: Chat,
})
