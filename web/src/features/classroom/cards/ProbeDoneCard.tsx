import { TargetIcon } from "lucide-react"

import { EventCard } from "./EventCard"

export function ProbeDoneCard({ asked, max }: { asked: number; max: number }) {
  return (
    <EventCard
      kind="probe"
      icon={TargetIcon}
      label="Probe done"
      summary={`Probe finished after ${asked} of ${max} questions`}
      status="done"
    />
  )
}
