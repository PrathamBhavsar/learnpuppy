import { CheckIcon } from "lucide-react"

import type { AnswerRecap } from "@/api/pending-types"
import { cn } from "@/lib/utils"

import { LazyMarkdown } from "./markdown/LazyMarkdown"

/** An option of an answered question: the right one in the success tint, a wrong pick in destructive. */
function RecapOption({
  callId,
  option,
}: {
  callId: string
  option: AnswerRecap["options"][number]
}) {
  return (
    <li
      data-testid={`recap-option-${option.letter}`}
      data-picked={option.picked}
      data-right={option.right}
      className={cn(
        "flex items-start gap-2 border px-2 py-1 text-[13px]",
        option.right
          ? "border-success/50 bg-success/10"
          : option.picked
            ? "border-destructive/50 bg-destructive/10"
            : "border-transparent"
      )}
    >
      <span
        aria-hidden
        className="mt-0.5 font-medium text-muted-foreground tabular-nums"
      >
        {option.letter}.
      </span>
      <div dir="auto" className="min-w-0 flex-1">
        <LazyMarkdown className="inline text-[13px] [&_p]:m-0 [&_p]:inline">
          {option.text}
        </LazyMarkdown>
      </div>
      {option.right ? (
        <CheckIcon
          aria-label="Right answer"
          className="mt-0.5 size-3.5 text-success"
        />
      ) : null}
      {option.picked ? (
        <span className="mt-0.5 shrink-0 text-[11px] text-muted-foreground">
          your pick
        </span>
      ) : null}
      <span className="sr-only">{`option ${option.letter} of ${callId}`}</span>
    </li>
  )
}

/**
 * What was asked, kept under the learner's answer so the prompt does not vanish after the verdict
 * (docs/web/06 §Learn). The grade is the server's: `picked`/`right` arrive already decided, options
 * already shuffled.
 */
export function QuestionRecap({ recap }: { recap: AnswerRecap }) {
  return (
    <div
      data-testid={`recap-${recap.callId}`}
      className="mt-1 flex max-w-[85%] flex-col gap-1.5 border bg-secondary/40 px-3 py-2"
    >
      <p className="text-[11px] text-muted-foreground">Question</p>
      <LazyMarkdown className="text-[13px]">{recap.question}</LazyMarkdown>
      {recap.options.length > 0 ? (
        <ul
          aria-label={
            recap.multiSelect ? "Options, several may be right" : "Options"
          }
          className="flex flex-col gap-0.5"
        >
          {recap.options.map((option) => (
            <RecapOption
              key={option.letter}
              callId={recap.callId}
              option={option}
            />
          ))}
        </ul>
      ) : null}
      {recap.freeText ? (
        <p className="text-[12px] text-muted-foreground">
          <span className="text-foreground/70">Your answer:</span>{" "}
          <span dir="auto">{recap.freeText}</span>
        </p>
      ) : null}
      {recap.confidence != null ? (
        <p className="text-[11px] text-muted-foreground">
          Confidence {recap.confidence}/5
        </p>
      ) : null}
    </div>
  )
}
