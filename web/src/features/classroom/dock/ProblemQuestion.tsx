import { useDraft } from "./drafts"
import { LightbulbIcon, SendIcon } from "lucide-react"
import { useState } from "react"
import { useHotkey } from "@/hooks/use-hotkey"

import type { AnswerInput, PendingQuestion } from "@/api/pending-types"
import { InfoTip } from "@/components/app/info/info"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"

import { classroomInfo } from "../info"
import { LazyMarkdown } from "../markdown/LazyMarkdown"

type ProblemQuestionProps = {
  pending: PendingQuestion
  busy: boolean
  onSubmit: (answer: AnswerInput) => void | Promise<void>
}

const MODE_LABEL: Record<string, string> = {
  recall: "Recall",
  discrimination: "Discrimination",
  composition: "Composition",
}

/** A practice problem: statement, a monospace answer editor, and a hint ladder (one rung per request). */
export function ProblemQuestion({
  pending,
  busy,
  onSubmit,
}: ProblemQuestionProps) {
  const problem = pending.problem
  const { text, setText, submit: submitDraft } = useDraft(pending.callId)
  const [hints, setHints] = useState(0)
  const rungs = problem?.rungs ?? []
  const canSubmit = !busy && text.trim() !== ""
  const submit = () => {
    if (canSubmit) void submitDraft(() => onSubmit({ text: text.trim() }))
  }
  useHotkey("mod+enter", submit, {
    id: "answer-submit",
    label: "Submit answer",
    group: "Learn",
    enabled: canSubmit,
  })
  return (
    <div className="flex flex-col gap-3">
      {problem ? (
        <div className="flex flex-col gap-2" data-testid="problem-statement">
          <Badge variant="outline" className="self-start font-normal">
            {MODE_LABEL[problem.mode] ?? problem.mode}
          </Badge>
          <LazyMarkdown>{problem.statement}</LazyMarkdown>
        </div>
      ) : null}
      {hints > 0 ? (
        <ol
          aria-label="Hints"
          data-testid="problem-hints"
          className="flex flex-col gap-1 border-s-2 border-warning ps-3 text-[13px]"
        >
          {Array.from({ length: hints }, (_, index) => (
            <li key={index} className="lp-rise">
              <span className="me-1.5 text-muted-foreground tabular-nums">
                Hint {index + 1}.
              </span>
              <LazyMarkdown>
                {rungs[index] ??
                  "Requested. The tutor answers in the conversation."}
              </LazyMarkdown>
            </li>
          ))}
        </ol>
      ) : null}
      <Textarea
        value={text}
        onChange={(event) => setText(event.target.value)}
        disabled={busy}
        dir="ltr"
        rows={5}
        spellCheck={false}
        aria-label="Your answer"
        placeholder="Write your answer"
        data-testid="problem-text"
        className="max-h-64 min-h-28 font-mono"
      />
      <div className="sticky bottom-0 z-10 flex items-center justify-between gap-2 bg-card py-2">
        <div className="flex items-center gap-1.5">
          <Button
            type="button"
            variant="outline"
            disabled={busy || (rungs.length > 0 && hints >= rungs.length)}
            onClick={() => {
              setHints((n) => n + 1)
              void Promise.resolve(
                onSubmit({ hint: true, text: text.trim() || undefined })
              ).catch(() => {
                // The caller presents the error; requesting a hint never clears the draft.
              })
            }}
            data-testid="problem-hint"
          >
            <LightbulbIcon data-icon="inline-start" />
            Give me a hint
          </Button>
          <InfoTip info={classroomInfo.hints} />
        </div>
        <Button
          onClick={submit}
          disabled={!canSubmit}
          data-testid="problem-submit"
        >
          {busy ? (
            <Spinner data-icon="inline-start" />
          ) : (
            <SendIcon data-icon="inline-start" className="rtl:-scale-x-100" />
          )}
          Submit
        </Button>
      </div>
    </div>
  )
}
