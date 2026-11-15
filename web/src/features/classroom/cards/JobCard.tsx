import { AlertTriangleIcon, CheckIcon, FileTextIcon } from "lucide-react"
import { AnimatePresence, motion } from "motion/react"
import type { LucideIcon } from "lucide-react"

import type { ClassroomJob } from "@/api/pending-types"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { dur } from "@/lib/motion"

import { EventCard } from "./EventCard"

function StatusIcon({ status }: { status: ClassroomJob["status"] }) {
  return (
    <AnimatePresence mode="popLayout" initial={false}>
      <motion.span
        key={status}
        initial={{ opacity: 0, scale: 0.6 }}
        animate={{ opacity: 1, scale: 1 }}
        exit={{ opacity: 0, scale: 0.6 }}
        transition={{ duration: dur.fast }}
        className="flex size-4 items-center justify-center"
      >
        {status === "running" ? (
          <Spinner className="size-3.5" />
        ) : status === "done" ? (
          <CheckIcon aria-hidden className="size-3.5 text-success" />
        ) : (
          <AlertTriangleIcon
            aria-hidden
            className="size-3.5 text-destructive"
          />
        )}
      </motion.span>
    </AnimatePresence>
  )
}

type JobCardProps = {
  job: ClassroomJob
  kindLabel: string
  icon: LucideIcon
  elapsed?: string | null
  error?: string | null
  aborting?: boolean
  onAbort: (job: ClassroomJob) => void
  onOpen: (job: ClassroomJob) => void
}

/** A running job shows a live spinner and an Abort button, then settles into done (or failed). */
export function JobCard({
  job,
  kindLabel,
  icon,
  elapsed,
  error,
  aborting,
  onAbort,
  onOpen,
}: JobCardProps) {
  const state =
    job.status === "running"
      ? "Running"
      : job.status === "done"
        ? "Done"
        : "Failed"
  return (
    <EventCard
      kind="job"
      icon={icon}
      label={`${kindLabel} · ${state}${elapsed ? ` · ${elapsed}` : ""}`}
      summary={job.title}
      status={job.status}
      testId={`job-${job.id}`}
      trailing={
        <>
          <StatusIcon status={job.status} />
          {job.status === "running" ? (
            <Button
              variant="outline"
              size="xs"
              disabled={aborting}
              onClick={() => onAbort(job)}
              aria-label={`Abort ${job.title}`}
              data-testid={`job-abort-${job.id}`}
            >
              Abort
            </Button>
          ) : null}
        </>
      }
    >
      {job.status === "failed" && error ? (
        <p
          className="text-[12px] text-destructive"
          data-testid={`job-error-${job.id}`}
        >
          {error}
        </p>
      ) : null}
      {job.status === "done" && job.artifactId ? (
        <Button
          variant="secondary"
          size="sm"
          className="lp-pop self-start"
          onClick={() => onOpen(job)}
          data-testid={`job-open-${job.id}`}
        >
          <FileTextIcon data-icon="inline-start" />
          Open result
        </Button>
      ) : null}
    </EventCard>
  )
}
