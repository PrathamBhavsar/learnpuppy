import { useAbortJob, useJobs } from "@/api/queries/chats"
import { JobsPopover, type JobRow } from "@/components/app/jobs-popover"
import { toastError } from "@/lib/toast"
import { useLive } from "@/stores/live"

const RUNNING = new Set(["running", "queued", "pending"])

/** Running jobs (workbench sub-agents, PDF ingestion) plus any ingest run streaming progress. */
export function WorkspaceJobs({ workspaceId }: { workspaceId: string | null }) {
  const jobs = useJobs(workspaceId ?? "")
  const abort = useAbortJob(workspaceId ?? "")
  const ingest = useLive((s) => s.ingest)

  const rows: JobRow[] = [
    ...(jobs.data ?? [])
      .filter((job) => RUNNING.has(job.status))
      .map((job) => ({
        id: job.id,
        label: job.title,
        detail: job.kind,
        cancellable: true,
      })),
    ...Object.values(ingest).map((run) => ({
      id: `ingest-${run.runId}`,
      label: "Ingest",
      detail: `${run.phase} · ${run.done}/${run.total}`,
    })),
  ]

  return (
    <JobsPopover
      jobs={rows}
      loading={workspaceId !== null && jobs.isPending}
      onCancel={(id) =>
        abort.mutate(id, {
          onError: (e) => toastError(e, "Could not cancel the job"),
        })
      }
    />
  )
}
