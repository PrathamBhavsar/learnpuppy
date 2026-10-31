import { ActivityIcon } from "lucide-react"

import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover"
import { Skeleton } from "@/components/ui/skeleton"

export type JobRow = {
  id: string
  label: string
  detail: string
  cancellable?: boolean
}

export function JobsPopover({
  jobs,
  loading = false,
  onCancel,
}: {
  jobs: readonly JobRow[]
  loading?: boolean
  onCancel?: (id: string) => void
}) {
  return (
    <Popover>
      <PopoverTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          data-testid="jobs-trigger"
          aria-label={
            jobs.length > 0
              ? `Jobs: ${jobs.length} running`
              : "Jobs: none running"
          }
          className="relative"
        >
          <ActivityIcon />
          {jobs.length > 0 ? (
            <Badge
              data-testid="jobs-count"
              className="absolute -end-1 -top-1 h-4 min-w-4 px-1 tabular-nums"
            >
              {jobs.length}
            </Badge>
          ) : null}
        </Button>
      </PopoverTrigger>
      <PopoverContent
        align="end"
        className="w-72 gap-2"
        data-testid="jobs-popover"
      >
        <p className="text-[12px] font-medium">Running jobs</p>
        {loading ? (
          <div className="flex flex-col gap-2" data-testid="jobs-loading">
            <Skeleton className="h-8 w-full" />
            <Skeleton className="h-8 w-full" />
          </div>
        ) : jobs.length === 0 ? (
          <p className="text-[13px] text-muted-foreground">
            Nothing is running in this workspace.
          </p>
        ) : (
          <ul className="flex flex-col gap-2">
            {jobs.map((job) => (
              <li
                key={job.id}
                data-testid={`job-${job.id}`}
                className="flex items-center gap-2 text-[13px]"
              >
                <span className="min-w-0 flex-1">
                  <span className="block truncate">{job.label}</span>
                  <span className="block truncate text-[11px] text-muted-foreground">
                    {job.detail}
                  </span>
                </span>
                {onCancel && job.cancellable ? (
                  <Button
                    variant="outline"
                    size="xs"
                    onClick={() => onCancel(job.id)}
                    aria-label={`Cancel ${job.label}`}
                  >
                    Cancel
                  </Button>
                ) : null}
              </li>
            ))}
          </ul>
        )}
      </PopoverContent>
    </Popover>
  )
}
