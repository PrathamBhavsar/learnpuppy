import {
  FileTextIcon,
  FlaskConicalIcon,
  GlobeIcon,
  PresentationIcon,
  ShapesIcon,
  type LucideIcon,
} from "lucide-react"

import type { ClassroomJob } from "@/api/pending-types"
import { useAbortJob, useJobs } from "@/api/queries/chats"
import { toastError } from "@/lib/toast"

import { JobCard } from "./cards/JobCard"
import { VisualReadyCard } from "./cards/VisualReadyCard"
import { formatClock } from "./copy"
import { useNow } from "./use-now"
import "./styles.css"

const KIND: Record<string, { label: string; icon: LucideIcon }> = {
  research: { label: "Research", icon: GlobeIcon },
  pdf: { label: "PDF", icon: FileTextIcon },
  visual: { label: "Diagram", icon: ShapesIcon },
  demo: { label: "Demo", icon: PresentationIcon },
}

type JobsPanelProps = {
  ws: string
  jobs: readonly ClassroomJob[]
  onOpenArtifact: (job: ClassroomJob) => void
}

/** Workbench sub-agent jobs of this chat: status, elapsed time, abort, and the finished artifact. */
export function JobsPanel({ ws, jobs, onOpenArtifact }: JobsPanelProps) {
  const list = useJobs(ws)
  const abort = useAbortJob(ws)
  const now = useNow(jobs.some((j) => j.status === "running"))
  const extra = new Map((list.data ?? []).map((item) => [item.id, item]))

  if (jobs.length === 0) {
    return (
      <div
        data-testid="jobs-empty"
        className="flex flex-col items-center gap-2 px-4 py-10 text-center"
      >
        <FlaskConicalIcon
          aria-hidden
          className="size-6 text-muted-foreground"
          strokeWidth={1.5}
        />
        <p className="text-[13px] text-muted-foreground">
          No background work yet. Research, PDF reading and diagrams show up
          here while they run.
        </p>
      </div>
    )
  }

  return (
    <ul
      className="flex flex-col gap-2 p-3"
      data-testid="jobs-panel"
      aria-label="Jobs"
    >
      {jobs.map((job) => {
        const meta = KIND[job.kind] ?? {
          label: job.kind,
          icon: FlaskConicalIcon,
        }
        const item = extra.get(job.id)
        const elapsed =
          job.status === "running" && item
            ? formatClock((now - item.createdAt) / 1000)
            : null
        return (
          <li key={job.id}>
            {job.kind === "visual" &&
            job.status === "done" &&
            job.artifactId ? (
              <VisualReadyCard job={job} onOpen={onOpenArtifact} />
            ) : (
              <JobCard
                job={job}
                kindLabel={meta.label}
                icon={meta.icon}
                elapsed={elapsed}
                error={item?.error}
                aborting={abort.isPending && abort.variables === job.id}
                onOpen={onOpenArtifact}
                onAbort={(j) =>
                  abort.mutate(j.id, {
                    onError: (e) => toastError(e, "Could not abort the job"),
                  })
                }
              />
            )}
          </li>
        )
      })}
    </ul>
  )
}
