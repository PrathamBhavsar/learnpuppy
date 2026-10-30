import { createFileRoute } from "@tanstack/react-router"
import { MessageSquareIcon } from "lucide-react"

import { EmptyState } from "@/components/app/empty-state"

function Chat() {
  const { chat } = Route.useParams()
  return (
    <EmptyState
      icon={MessageSquareIcon}
      title="Chat not loaded"
      description={`The classroom for chat ${chat} opens here.`}
      className="h-full"
    />
  )
}

export const Route = createFileRoute("/_shell/w/$ws/learn/$chat")({
  component: Chat,
})
