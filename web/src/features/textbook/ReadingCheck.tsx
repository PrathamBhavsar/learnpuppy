import { useEffect, useRef, useState } from "react"

import { toApiError } from "@/api/errors"
import { Choice } from "@/components/app/choice"
import { ErrorState } from "@/components/app/error-state"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { toastError } from "@/lib/toast"

import {
  type Chapter,
  type Feedback,
  type ReadingCheck as Check,
  useCheckActions,
} from "./api"

const GRADES = ["again", "hard", "good", "easy"] as const

/**
 * One chapter's reading check, a question at a time. The questions arrive without answers; the server
 * grades each one and sends the verdict back. Everything here is plain text, nothing is evaluated.
 */
export function ReadingCheck({
  ws,
  book,
  chapter,
  onClose,
}: {
  ws: string
  book: string
  chapter: Chapter
  onClose: () => void
}) {
  const actions = useCheckActions(ws, book)
  const [check, setCheck] = useState<Check | null>(null)
  const [at, setAt] = useState(0)
  const [text, setText] = useState("")
  const [shownAt, setShownAt] = useState(() => Date.now())
  const [feedback, setFeedback] = useState<Feedback | null>(null)
  const [graded, setGraded] = useState(false)
  const [correct, setCorrect] = useState(0)

  const heading = useRef<HTMLElement>(null)
  // Move focus into the card whenever the question, feedback or summary changes, so the keyboard path continues.
  useEffect(() => {
    heading.current?.focus()
  }, [at, check, feedback])

  const start = actions.start
  const item = check?.items[at]
  const finished = check !== null && at >= check.items.length

  const begin = () =>
    start.mutate(chapter.index, {
      onSuccess: (c) => {
        setCheck(c)
        setAt(0)
        setText("")
        setCorrect(0)
        setFeedback(null)
        setShownAt(Date.now())
      },
    })

  if (!check) {
    return (
      <section
        ref={heading}
        tabIndex={-1}
        data-testid="reading-check"
        aria-label={`Reading check for chapter ${chapter.index + 1}`}
        className="flex flex-col gap-3 border p-4"
      >
        <h2 className="text-[15px] font-medium">
          Check I read chapter {chapter.index + 1}
        </h2>
        <p className="text-[13px]">{chapter.title}</p>
        {start.isError ? (
          <ErrorState
            message={toApiError(start.error).message}
            onRetry={begin}
          />
        ) : (
          <p className="text-[13px] text-muted-foreground">
            A few questions from this chapter, no model needed.
          </p>
        )}
        <div className="flex gap-2">
          <Button
            data-testid="reading-check-start"
            disabled={start.isPending}
            onClick={begin}
          >
            {start.isPending ? <Spinner /> : null}
            Start
          </Button>
          <Button variant="outline" onClick={onClose}>
            Back to chapters
          </Button>
        </div>
      </section>
    )
  }

  if (finished) {
    return (
      <section
        ref={heading}
        tabIndex={-1}
        data-testid="reading-check-done"
        className="flex flex-col gap-3 border p-4"
      >
        <h2 className="text-[15px] font-medium">Check finished</h2>
        <p role="status" className="text-[13px]">
          {correct} of {check.items.length} right. Missed items come back in
          review.
        </p>
        <Button className="self-start" onClick={onClose}>
          Back to chapters
        </Button>
      </section>
    )
  }

  if (!item) return null

  const submit = (gap: boolean) =>
    actions.answer.mutate(
      {
        check: check.checkId,
        body: {
          item: item.id,
          text: gap ? "" : text.trim(),
          ms: Date.now() - shownAt,
        },
      },
      {
        onSuccess: (f) => {
          setFeedback(f)
          setGraded(false)
          if (f.correct) setCorrect((n) => n + 1)
        },
        onError: (e) => toastError(e, "Could not check your answer"),
      }
    )

  const next = () => {
    setAt((n) => n + 1)
    setText("")
    setFeedback(null)
    setShownAt(Date.now())
  }

  const grade = (g: (typeof GRADES)[number]) =>
    actions.selfGrade.mutate(
      { check: check.checkId, body: { item: item.id, grade: g } },
      {
        onSuccess: () => {
          setGraded(true)
          if (g === "good" || g === "easy") setCorrect((n) => n + 1)
        },
        onError: (e) => toastError(e, "Could not save your grade"),
      }
    )

  const needsGrade = feedback?.close === true && !feedback.correct && !graded

  return (
    <section
      ref={heading}
      tabIndex={-1}
      data-testid="reading-check"
      aria-label={`Reading check for chapter ${chapter.index + 1}`}
      className="flex flex-col gap-3 border p-4"
    >
      <p className="text-[12px] text-muted-foreground">
        Question {at + 1} of {check.items.length}
      </p>
      <p
        data-testid="reading-check-q"
        className="text-[14px] whitespace-pre-wrap"
      >
        {item.q}
      </p>
      <Choice
        key={item.id}
        item={item}
        value={text}
        disabled={feedback !== null || actions.answer.isPending}
        onChange={setText}
      />
      {feedback ? (
        <div
          data-testid="reading-check-feedback"
          data-correct={feedback.correct}
          role="status"
          className="flex flex-col gap-1 border p-3 text-[13px]"
        >
          <p className="font-medium">
            {feedback.correct
              ? "Right"
              : feedback.close
                ? "Close"
                : "Not quite"}
          </p>
          {feedback.correct ? null : <p>Answer: {feedback.answer}</p>}
          <p className="whitespace-pre-wrap">{feedback.why}</p>
        </div>
      ) : null}
      {needsGrade ? (
        <div
          role="group"
          aria-label="How close was your answer?"
          className="flex flex-wrap gap-2"
        >
          {GRADES.map((g) => (
            <Button
              key={g}
              size="sm"
              variant="outline"
              data-testid={`reading-check-grade-${g}`}
              disabled={actions.selfGrade.isPending}
              onClick={() => grade(g)}
            >
              {g[0]?.toUpperCase()}
              {g.slice(1)}
            </Button>
          ))}
        </div>
      ) : null}
      <div className="flex gap-2">
        {feedback ? (
          <Button
            data-testid="reading-check-next"
            disabled={needsGrade}
            onClick={next}
          >
            {at + 1 === check.items.length ? "Finish" : "Next question"}
          </Button>
        ) : (
          <>
            <Button
              data-testid="reading-check-answer"
              disabled={actions.answer.isPending || text.trim() === ""}
              onClick={() => submit(false)}
            >
              Answer
            </Button>
            <Button
              variant="outline"
              disabled={actions.answer.isPending}
              onClick={() => submit(true)}
            >
              I don&apos;t know
            </Button>
          </>
        )}
        <Button variant="ghost" onClick={onClose}>
          Stop
        </Button>
      </div>
    </section>
  )
}
