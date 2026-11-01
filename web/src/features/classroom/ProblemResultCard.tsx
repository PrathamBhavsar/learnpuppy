import { FlagIcon } from "lucide-react"

import type { ProblemResult } from "@/api/pending-types"
import { InfoTip } from "@/components/app/info/info"
import { Badge } from "@/components/ui/badge"

import { classroomInfo } from "./info"
import { LazyMarkdown } from "./markdown/LazyMarkdown"

const OUTCOME_LABEL: Record<string, string> = {
  solved: "Solved",
  solved_with_hints: "Solved with hints",
  missed: "Missed",
  gave_up: "Gave up",
}

function humanize(value: string): string {
  return (
    OUTCOME_LABEL[value] ??
    value.replace(/_/g, " ").replace(/^./, (c) => c.toUpperCase())
  )
}

/** Feedback committed when a Practice problem concludes: outcome, miss class and the model verdict. */
export function ProblemResultCard({ result }: { result: ProblemResult }) {
  return (
    <section
      aria-label="Problem result"
      data-testid="problem-result"
      data-outcome={result.outcome}
      className="lp-rise flex flex-col gap-2 border bg-card p-3"
    >
      <div className="flex flex-wrap items-center gap-2">
        <FlagIcon aria-hidden className="size-3.5 text-muted-foreground" />
        <Badge variant="secondary">{humanize(result.outcome)}</Badge>
        {result.missClass ? (
          <Badge variant="outline" className="font-normal">
            Miss: {humanize(result.missClass)}
          </Badge>
        ) : null}
        {result.rung ? (
          <Badge variant="outline" className="font-normal">
            Rung {result.rung}
          </Badge>
        ) : null}
        {result.retest ? (
          <Badge variant="outline" className="font-normal text-warning">
            Retest later
          </Badge>
        ) : null}
        <InfoTip info={classroomInfo.problemResult} />
      </div>
      <LazyMarkdown className="text-[13px]">{result.verdict}</LazyMarkdown>
    </section>
  )
}
