import { CheckIcon, PlayIcon, SquareIcon } from "lucide-react"

import type { IngestRun } from "@/api/pending-types"
import { CountUp } from "@/components/app/count-up"
import { InfoTip } from "@/components/app/info/info"
import { Button } from "@/components/ui/button"
import { Progress } from "@/components/ui/progress"
import { Spinner } from "@/components/ui/spinner"
import { useLive } from "@/stores/live"

import { ingestInfo } from "./info"
import { formatDuration, percent, phaseLabel } from "./model"

import "./ingest.css"

type RunPanelProps = {
  runId: string | null
  /** Run is allowed: sources saved and the allow-list is not empty. */
  canRun: boolean
  /** Why Run is disabled, shown under the button. */
  blockedReason: string | null
  starting: boolean
  cancelling: boolean
  onRun: () => void
  onCancel: () => void
  last: IngestRun | undefined
}

export function RunPanel({
  runId,
  canRun,
  blockedReason,
  starting,
  cancelling,
  onRun,
  onCancel,
  last,
}: RunPanelProps) {
  const progress = useLive((s) => (runId ? s.ingest[runId] : undefined))
  const running = runId !== null

  return (
    <section
      aria-labelledby="run-heading"
      data-testid="ingest-run-panel"
      data-state={running ? "running" : "idle"}
      className="flex flex-col gap-4 border p-4"
    >
      <h2
        id="run-heading"
        className="flex items-center gap-1.5 font-heading text-[16px] font-medium"
      >
        Run
        <InfoTip info={ingestInfo.run} />
      </h2>

      {running ? (
        <div className="flex flex-col gap-3" data-testid="run-progress">
          <div className="flex items-baseline justify-between gap-2 text-[13px]">
            <span
              key={progress?.phase ?? "starting"}
              className="lp-fade-swap font-medium"
              data-testid="run-phase"
            >
              {progress ? phaseLabel(progress.phase) : "Starting"}
            </span>
            <InfoTip info={ingestInfo.progress} />
            <span
              className="text-muted-foreground tabular-nums"
              data-testid="run-counts"
            >
              <CountUp value={progress?.done ?? 0} /> /{" "}
              <CountUp value={progress?.total ?? 0} /> sessions
            </span>
          </div>
          <Progress
            value={progress ? percent(progress.done, progress.total) : 0}
            aria-label="Ingest progress"
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={
              progress ? percent(progress.done, progress.total) : 0
            }
            data-testid="run-bar"
            className="lp-sheen h-2"
          />
          <Button
            variant="outline"
            disabled={cancelling}
            onClick={onCancel}
            data-testid="run-cancel"
          >
            {cancelling ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <SquareIcon data-icon="inline-start" />
            )}
            Cancel run
          </Button>
        </div>
      ) : (
        <div className="flex flex-col gap-2">
          <Button
            size="lg"
            disabled={!canRun || starting}
            onClick={onRun}
            data-testid="run-start"
            className="h-12 text-[14px]"
          >
            {starting ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <PlayIcon data-icon="inline-start" />
            )}
            Run ingest
          </Button>
          {blockedReason ? (
            <p
              data-testid="run-blocked"
              className="text-[12px] text-muted-foreground"
            >
              {blockedReason}
            </p>
          ) : null}
        </div>
      )}

      {last && !running ? <LastRun run={last} /> : null}
    </section>
  )
}

function LastRun({ run }: { run: IngestRun }) {
  const failed = run.report.errors.length
  const duration = run.finishedAt ? run.finishedAt - run.startedAt : 0
  return (
    <div
      key={run.id}
      data-testid="last-run"
      className="lp-slide-down flex flex-col gap-2 border-t pt-3"
    >
      <div className="flex items-center gap-2 text-[13px]">
        {failed === 0 ? (
          <svg
            viewBox="0 0 24 24"
            className="size-4 text-success"
            fill="none"
            stroke="currentColor"
            strokeWidth="2.5"
            aria-hidden
          >
            <path d="M5 12.5l4.5 4.5L19 7.5" className="lp-check-draw" />
          </svg>
        ) : (
          <CheckIcon className="size-4 text-warning" aria-hidden />
        )}
        <span className="font-medium">
          {failed === 0 ? "Last run finished" : "Last run finished with errors"}
        </span>
        <InfoTip info={ingestInfo.lastRun} />
        <span className="text-muted-foreground tabular-nums">
          · {formatDuration(duration)}
        </span>
      </div>
      <dl className="grid grid-cols-3 gap-2 text-[12px]">
        <Stat label="Sessions" value={run.report.sessions} />
        <Stat label="New decisions" value={run.report.newDecisions} />
        <Stat label="New cards" value={run.report.newCards} />
      </dl>
    </div>
  )
}

function Stat({ label, value }: { label: string; value: number }) {
  return (
    <div className="flex flex-col">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="text-[16px] font-medium">
        <CountUp value={value} />
      </dd>
    </div>
  )
}
