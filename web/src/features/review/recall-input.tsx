import { CheckIcon, XIcon } from "lucide-react"
import { type FormEvent, useState } from "react"

import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"

type Props = {
  /** The answer is being checked by the server. */
  pending: boolean
  answered: boolean
  /** What the learner typed (kept once answered). */
  typed: string | null
  /** The server's verdict, once answered. */
  correct: boolean | null
  /** The expected answer, shown once answered. */
  expected: string | null
  onSubmit: (text: string) => void
}

/** Free recall: the learner types the answer; the server grades it by rubric in code. */
export function RecallInput({
  pending,
  answered,
  typed,
  correct,
  expected,
  onSubmit,
}: Props) {
  const [text, setText] = useState("")
  const submit = (event: FormEvent) => {
    event.preventDefault()
    const value = text.trim()
    if (value && !pending && !answered) onSubmit(value)
  }
  return (
    <form
      data-testid="review-recall"
      onSubmit={submit}
      className="flex flex-col gap-2"
    >
      <Textarea
        data-testid="review-recall-input"
        aria-label="Your answer"
        placeholder="Type the answer from memory"
        rows={3}
        value={answered ? (typed ?? text) : text}
        disabled={pending || answered}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (event.key === "Enter" && (event.metaKey || event.ctrlKey))
            submit(event)
        }}
      />
      {answered ? (
        <div
          data-testid="review-recall-verdict"
          data-correct={correct ? "" : undefined}
          className="flex items-start gap-2 text-[13px]"
        >
          {correct ? (
            <CheckIcon className="mt-0.5 size-4 shrink-0 text-success" />
          ) : (
            <XIcon className="mt-0.5 size-4 shrink-0 text-destructive" />
          )}
          <span>
            {correct ? "Correct." : "Not quite."}
            {expected ? ` Answer: ${expected}` : ""}
          </span>
        </div>
      ) : (
        <Button
          type="submit"
          data-testid="review-recall-submit"
          disabled={pending || !text.trim()}
          className="self-start"
        >
          Check answer
        </Button>
      )}
    </form>
  )
}
