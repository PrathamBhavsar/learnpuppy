import { createFileRoute } from "@tanstack/react-router"
import { MessagesSquareIcon } from "lucide-react"

import { EmptyState } from "@/components/app/empty-state"

function LearnIndex() {
  return (
    <EmptyState
      icon={MessagesSquareIcon}
      title="Start learning something"
      description="Chats appear here. Pick one, or begin a new topic."
      className="h-full"
    />
  )
}

export const Route = createFileRoute("/_shell/w/$ws/learn/")({
  component: LearnIndex,
})
