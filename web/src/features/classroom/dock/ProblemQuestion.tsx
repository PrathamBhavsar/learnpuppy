import { LightbulbIcon, SendIcon } from "lucide-react"
import { useState } from "react"

import type { AnswerInput, PendingQuestion } from "@/api/pending-types"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"

import { LazyMarkdown } from "../markdown/LazyMarkdown"

type ProblemQuestionProps = {
  pending: PendingQuestion
  busy: boolean
  onSubmit: (answer: AnswerInput) => void
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
  const [text, setText] = useState("")
  const [hints, setHints] = useState(0)
  const rungs = problem?.rungs ?? []
  const canSubmit = !busy && text.trim() !== ""
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
              {rungs[index] ??
                "Requested. The tutor answers in the conversation."}
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
      <div className="flex items-center justify-between gap-2">
        <Button
          type="button"
          variant="outline"
          disabled={busy || (rungs.length > 0 && hints >= rungs.length)}
          onClick={() => {
            setHints((n) => n + 1)
            onSubmit({ hint: true, text: text.trim() || undefined })
          }}
          data-testid="problem-hint"
        >
          <LightbulbIcon data-icon="inline-start" />
          Give me a hint
        </Button>
        <Button
          onClick={() => canSubmit && onSubmit({ text: text.trim() })}
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
