import { ChevronRightIcon, TriangleAlertIcon } from "lucide-react"
import { useState } from "react"

import type { IngestRun } from "@/api/pending-types"
import { Badge } from "@/components/ui/badge"
import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"
import { useGoto } from "@/hooks/use-goto"
import { cn } from "@/lib/utils"

import { actionFor, digestPath, formatDuration, isRunning } from "./model"

type RunHistoryProps = { ws: string; runs: IngestRun[] }

export function RunHistory({ ws, runs }: RunHistoryProps) {
  return (
    <section
      aria-labelledby="history-heading"
      data-testid="ingest-history"
      className="flex flex-col gap-3"
    >
      <h2 id="history-heading" className="font-heading text-[16px] font-medium">
        History
      </h2>
      {runs.length === 0 ? (
        <p
          data-testid="history-empty"
          className="border border-dashed px-3 py-6 text-center text-[13px] text-muted-foreground"
        >
          No runs yet. The first one appears here with its report.
        </p>
      ) : (
        <ul role="list" className="flex flex-col border">
          {runs.map((run) => (
            <RunRow key={run.id} ws={ws} run={run} />
          ))}
        </ul>
      )}
    </section>
  )
}

function RunRow({ ws, run }: { ws: string; run: IngestRun }) {
  const [open, setOpen] = useState(false)
  const goto = useGoto()
  const running = isRunning(run)
  const errors = run.report.errors
  const href = digestPath(ws, run)

  return (
    <li
      data-testid={`run-${run.id}`}
      data-running={running}
      className="border-b last:border-b-0"
    >
      <Collapsible open={open} onOpenChange={setOpen}>
        <div className="flex items-center gap-3 px-3 py-2.5 text-[13px]">
          <CollapsibleTrigger
            data-testid={`run-toggle-${run.id}`}
            aria-label={`Details for the run on ${new Date(run.startedAt).toLocaleString()}`}
            className="grid size-6 shrink-0 place-items-center text-muted-foreground hover:text-foreground"
          >
            <ChevronRightIcon
              className={cn(
                "size-4 transition-transform rtl:-scale-x-100",
                open && "rotate-90 rtl:rotate-90"
              )}
              aria-hidden
            />
          </CollapsibleTrigger>
          <time
            dateTime={new Date(run.startedAt).toISOString()}
            className="min-w-0 flex-1 truncate tabular-nums"
          >
            {new Date(run.startedAt).toLocaleString()}
          </time>
          {running ? (
            <Badge variant="secondary">Running</Badge>
          ) : (
            <span className="text-muted-foreground tabular-nums">
              {formatDuration(
                (run.finishedAt ?? run.startedAt) - run.startedAt
              )}
            </span>
          )}
          <span
            className="hidden w-40 text-end text-muted-foreground tabular-nums sm:inline"
            data-testid={`run-summary-${run.id}`}
          >
            {run.report.sessions} sessions · {run.report.newDecisions} decisions
            · {run.report.newCards} cards
          </span>
          {errors.length > 0 ? (
            <Badge variant="destructive" data-testid={`run-errors-${run.id}`}>
              <TriangleAlertIcon data-icon="inline-start" />
              {errors.length}
            </Badge>
          ) : null}
        </div>
        <CollapsibleContent>
          <div
            data-testid={`run-details-${run.id}`}
            className="flex flex-col gap-3 border-t bg-muted/30 px-4 py-3 text-[12px]"
          >
            <dl className="grid grid-cols-2 gap-2 sm:grid-cols-4">
              <Detail label="Sessions" value={run.report.sessions} />
              <Detail label="New decisions" value={run.report.newDecisions} />
              <Detail label="New cards" value={run.report.newCards} />
              <Detail label="Skipped" value={run.report.skipped} />
            </dl>
            {errors.length > 0 ? (
              <ul role="list" className="flex flex-col gap-2">
                {errors.map((error) => {
                  const action = actionFor(error)
                  return (
                    <li
                      key={error}
                      className="border-s-2 border-destructive ps-3"
                    >
                      <p className="font-medium text-destructive">{error}</p>
                      {action ? (
                        <p className="text-muted-foreground">{action}</p>
                      ) : null}
                    </li>
                  )
                })}
              </ul>
            ) : null}
            {!running && run.report.newDecisions > 0 ? (
              <a
                href={href}
                data-testid={`run-digest-${run.id}`}
                onClick={(e) => {
                  e.preventDefault()
                  goto(href)
                }}
                className="w-fit underline underline-offset-2"
              >
                View new decisions
              </a>
            ) : null}
          </div>
        </CollapsibleContent>
      </Collapsible>
    </li>
  )
}

function Detail({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex flex-col">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="font-medium tabular-nums">{value}</dd>
    </div>
  )
}
