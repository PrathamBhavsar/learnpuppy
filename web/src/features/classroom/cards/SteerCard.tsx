import { MessageSquareQuoteIcon } from "lucide-react"

import { EventCard } from "./EventCard"

export function SteerCard({ text }: { text: string }) {
  return (
    <EventCard
      kind="steer"
      icon={MessageSquareQuoteIcon}
      label="Steer"
      summary={text.replace(/\s+/g, " ").trim()}
      testId="card-steer"
    />
  )
}
