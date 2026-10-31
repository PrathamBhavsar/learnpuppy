import {
  AlertTriangleIcon,
  CheckIcon,
  FileTextIcon,
  FlaskConicalIcon,
  GlobeIcon,
  PresentationIcon,
  ShapesIcon,
  type LucideIcon,
} from "lucide-react"
import { AnimatePresence, motion } from "motion/react"

import type { ClassroomJob } from "@/api/pending-types"
import { useAbortJob, useJobs } from "@/api/queries/chats"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { dur } from "@/lib/motion"
import { toastError } from "@/lib/toast"
import { cn } from "@/lib/utils"

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
        const KindIcon = meta.icon
        return (
          <li
            key={job.id}
            data-testid={`job-${job.id}`}
            data-status={job.status}
            className="flex flex-col gap-2 border p-2.5"
          >
            <div className="flex items-start gap-2">
              <KindIcon
                aria-hidden
                className="mt-0.5 size-3.5 shrink-0 text-muted-foreground"
              />
              <div className="min-w-0 flex-1">
                <p className="truncate text-[13px]" title={job.title}>
                  {job.title}
                </p>
                <p className="flex items-center gap-1.5 text-[11px] text-muted-foreground tabular-nums">
                  <StatusIcon status={job.status} />
                  <span>
                    {meta.label} ·{" "}
                    {job.status === "running"
                      ? "Running"
                      : job.status === "done"
                        ? "Done"
                        : "Failed"}
                    {elapsed ? ` · ${elapsed}` : ""}
                  </span>
                </p>
              </div>
              {job.status === "running" ? (
                <Button
                  variant="outline"
                  size="xs"
                  disabled={abort.isPending && abort.variables === job.id}
                  onClick={() =>
                    abort.mutate(job.id, {
                      onError: (e) => toastError(e, "Could not abort the job"),
                    })
                  }
                  aria-label={`Abort ${job.title}`}
                  data-testid={`job-abort-${job.id}`}
                >
                  Abort
                </Button>
              ) : null}
            </div>
            {job.status === "failed" && item?.error ? (
              <p
                className="text-[12px] text-destructive"
                data-testid={`job-error-${job.id}`}
              >
                {item.error}
              </p>
            ) : null}
            {job.status === "done" && job.artifactId ? (
              <Button
                variant="secondary"
                size="sm"
                className={cn("lp-pop self-start")}
                onClick={() => onOpenArtifact(job)}
                data-testid={`job-open-${job.id}`}
              >
                <FileTextIcon data-icon="inline-start" />
                Open result
              </Button>
            ) : null}
          </li>
        )
      })}
    </ul>
  )
}
