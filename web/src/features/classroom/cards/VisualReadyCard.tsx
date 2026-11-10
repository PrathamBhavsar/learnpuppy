import { ShapesIcon } from "lucide-react"

import type { ClassroomJob } from "@/api/pending-types"
import { Button } from "@/components/ui/button"

import { EventCard } from "./EventCard"

export function VisualReadyCard({
  job,
  onOpen,
}: {
  job: ClassroomJob
  onOpen: (job: ClassroomJob) => void
}) {
  return (
    <EventCard
      kind="visual"
      icon={ShapesIcon}
      label="Diagram ready"
      summary={job.title}
      status="done"
      testId={`job-${job.id}`}
    >
      <Button
        variant="secondary"
        size="sm"
        className="lp-pop self-start"
        onClick={() => onOpen(job)}
        data-testid={`job-open-${job.id}`}
      >
        Open result
      </Button>
    </EventCard>
  )
}
