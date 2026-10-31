import { XIcon } from "lucide-react"

import { cn } from "@/lib/utils"

import { LazyMarkdown } from "./markdown/LazyMarkdown"

type AnswerVerdictProps = {
  callId: string
  correct: boolean
  explanation?: string | null
}

/**
 * The server's grade of an answered quiz or probe (docs/web/05 #30). Correct: success tint, check
 * draws and a sweep passes. Wrong: a three-cycle shake (a static red border under reduced motion).
 */
export function AnswerVerdict({
  callId,
  correct,
  explanation,
}: AnswerVerdictProps) {
  return (
    <div
      role="status"
      data-testid={`verdict-${callId}`}
      data-correct={correct}
      className={cn(
        "mt-1 flex max-w-[85%] flex-col gap-1 border px-3 py-2 text-[13px]",
        correct
          ? "lp-sweep border-success/50 bg-success/10"
          : "lp-shake-x border-destructive/50 bg-destructive/10"
      )}
    >
      <div className="flex items-center gap-1.5 font-medium">
        {correct ? (
          <svg
            aria-hidden
            viewBox="0 0 16 16"
            className="size-4 text-success"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
          >
            <path pathLength={1} className="lp-draw" d="M3 8.5l3.2 3L13 4.5" />
          </svg>
        ) : (
          <XIcon aria-hidden className="size-4 text-destructive" />
        )}
        <span>{correct ? "Correct" : "Not quite"}</span>
      </div>
      {explanation ? (
        <LazyMarkdown className="text-[13px] text-muted-foreground">
          {explanation}
        </LazyMarkdown>
      ) : null}
    </div>
  )
}
