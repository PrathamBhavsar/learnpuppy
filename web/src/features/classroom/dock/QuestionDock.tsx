import type { AnswerInput, PendingQuestion } from "@/api/pending-types"
import { InfoTip } from "@/components/app/info/info"
import { Badge } from "@/components/ui/badge"

import { LazyMarkdown } from "../markdown/LazyMarkdown"
import { questionInfo } from "../info"
import { ChoiceQuestion } from "./ChoiceQuestion"
import { ProblemQuestion } from "./ProblemQuestion"
import { ProbeProgress } from "./ProbeProgress"
import { TextQuestion } from "./TextQuestion"

type QuestionDockProps = {
  pending: PendingQuestion
  probe: { asked: number; max: number }
  busy: boolean
  onSubmit: (answer: AnswerInput) => void | Promise<void>
}

const TOOL_NAME: Record<string, string> = {
  probe: "Probe",
  recall: "Recall",
  quiz: "Quiz",
  explain: "Explain",
  ask_user_question: "Question",
  problem: "Problem",
}

/** Replaces the composer while a question is pending. One surface per tool, all sharing the submit contract. */
export function QuestionDock({
  pending,
  probe,
  busy,
  onSubmit,
}: QuestionDockProps) {
  const hasOptions = (pending.options?.length ?? 0) > 0
  return (
    <section
      aria-label="Question"
      data-testid="question-dock"
      data-tool={pending.tool}
      data-call-id={pending.callId}
      className="lp-dock-in mx-auto flex max-h-[55dvh] w-full max-w-3xl shrink-0 flex-col gap-3 overflow-y-auto overscroll-contain border-t bg-card px-4 py-3"
    >
      <div className="flex items-center justify-between gap-3">
        <div className="flex items-center gap-1.5">
          <Badge variant="secondary">
            {TOOL_NAME[pending.tool] ?? pending.tool}
          </Badge>
          <InfoTip info={questionInfo(pending.tool)} />
        </div>
        {pending.tool === "probe" ? (
          <div className="flex flex-1 items-center justify-end gap-3">
            <ProbeProgress
              done={Math.max(0, probe.asked - 1)}
              max={probe.max}
              className="max-w-40 flex-1"
            />
            <span
              data-testid="probe-count"
              className="text-[12px] whitespace-nowrap text-muted-foreground tabular-nums"
            >
              Question {probe.asked} of ≤{probe.max}
            </span>
          </div>
        ) : null}
      </div>
      {pending.tool !== "problem" ? (
        <LazyMarkdown className="text-[14px] font-medium">
          {pending.prompt}
        </LazyMarkdown>
      ) : null}
      {pending.tool === "problem" ? (
        <ProblemQuestion pending={pending} busy={busy} onSubmit={onSubmit} />
      ) : hasOptions ? (
        <ChoiceQuestion pending={pending} busy={busy} onSubmit={onSubmit} />
      ) : (
        <TextQuestion pending={pending} busy={busy} onSubmit={onSubmit} />
      )}
    </section>
  )
}
