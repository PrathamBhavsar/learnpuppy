import { Button } from "@/components/ui/button"
import { Progress } from "@/components/ui/progress"

import type { Job } from "./api"

/** Chapter-level progress of the running import, read from the job the server reports. */
export function ImportProgress({
  job,
  onCancel,
  cancelling,
}: {
  job: Job | undefined
  onCancel: () => void
  cancelling: boolean
}) {
  const done = job?.chaptersDone ?? 0
  const total = job?.chaptersTotal ?? 0
  const pagesRead = job?.pagesRead
  const totalPages = job?.totalPages
  const label =
    total > 0
      ? `${done} of ${total} chapters imported`
      : pagesRead != null && totalPages
        ? `Read ${pagesRead} of ${totalPages} pages`
        : "Starting the import"
  return (
    <section
      data-testid="textbook-import"
      aria-label="Import progress"
      className="flex flex-col gap-2 border p-3"
    >
      <div className="flex items-center justify-between gap-3">
        <p role="status" className="text-[13px]">
          {label}
        </p>
        <Button
          size="sm"
          variant="outline"
          data-testid="textbook-import-cancel"
          disabled={!job || cancelling}
          onClick={onCancel}
        >
          Cancel import
        </Button>
      </div>
      <Progress
        aria-label="Import progress"
        value={total > 0 ? (done / total) * 100 : null}
      />
    </section>
  )
}
