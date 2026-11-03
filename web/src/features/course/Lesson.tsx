import { useEffect, useRef, useState } from "react"

import type { components } from "@/api/schema"
import { PromptMarkdown } from "@/components/app/prompt-markdown"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"
import { toastError } from "@/lib/toast"

import { type Step, type useCourseActions, useUsage } from "./api"
import { Choice } from "./Placement"

type Dto = components["schemas"]
type Actions = ReturnType<typeof useCourseActions>

const GRADES = ["again", "hard", "good", "easy"] as const

/** Runs stored steps (explain / ask / waiting / done). Only `ask` and `check` can need a model. */
export function Lesson({
  ws,
  id,
  actions,
}: {
  ws: string
  id: string
  actions: Actions
}) {
  const [step, setStep] = useState<Step | null>(null)
  const started = useRef(false)
  const { next } = actions

  const advance = () =>
    next.mutate(undefined, {
      onSuccess: setStep,
      onError: (e) => toastError(e, "Could not load the next step"),
    })

  useEffect(() => {
    if (started.current) return
    started.current = true
    advance()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // More lessons are being written: look again every few seconds.
  const waiting = step?.step === "waiting"
  useEffect(() => {
    if (!waiting) return
    const timer = setTimeout(advance, 4000)
    return () => clearTimeout(timer)
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [waiting, next.isPending])

  return (
    <div data-testid="course-lesson" className="flex flex-col gap-4">
      {step === null ? <Spinner /> : null}
      {step?.step === "explain" ? (
        <Explain
          lesson={step.lesson}
          onSeen={() =>
            actions.seen.mutate(step.lesson.id, {
              onSuccess: advance,
              onError: (e) => toastError(e, "Could not save progress"),
            })
          }
        />
      ) : null}
      {step?.step === "ask" ? (
        <Ask
          key={step.item.id}
          item={step.item}
          actions={actions}
          onNext={advance}
        />
      ) : null}
      {step?.step === "waiting" ? (
        <p role="status" className="text-[13px]">
          The next lessons are still being written. This page will move on by
          itself.
        </p>
      ) : null}
      {step?.step === "done" ? <Done ws={ws} id={id} /> : null}
      {step && step.step !== "done" ? <AskBox actions={actions} /> : null}
    </div>
  )
}

function Explain({
  lesson,
  onSeen,
}: {
  lesson: Dto["LessonDto"]
  onSeen: () => void
}) {
  return (
    <article data-testid="course-explain" className="flex flex-col gap-3">
      <h2 className="font-heading text-[18px] font-medium">{lesson.title}</h2>
      <p className="text-[13px] text-muted-foreground">{lesson.objective}</p>
      <PromptMarkdown source={lesson.explain} />
      {lesson.keyFacts.length ? (
        <ul className="list-disc ps-5 text-[13px]">
          {lesson.keyFacts.map((f) => (
            <li key={f}>{f}</li>
          ))}
        </ul>
      ) : null}
      <Button onClick={onSeen} className="self-start">
        Got it
      </Button>
    </article>
  )
}

function Ask({
  item,
  actions,
  onNext,
}: {
  item: Dto["ItemDto"]
  actions: Actions
  onNext: () => void
}) {
  const [text, setText] = useState("")
  const [shownAt] = useState(() => Date.now())
  const [reason, setReason] = useState<string | null>(null)
  const [note, setNote] = useState<string | null>(null)
  const [flagged, setFlagged] = useState(false)
  const [result, setResult] = useState<Dto["FeedbackDto"] | null>(null)

  return (
    <section data-testid="course-question" className="flex flex-col gap-3">
      <PromptMarkdown source={item.q} />
      <Choice
        item={item}
        value={text}
        disabled={Boolean(result)}
        onChange={setText}
      />
      {!result ? (
        <Button
          className="self-start"
          disabled={text.trim() === "" || actions.answer.isPending}
          onClick={() =>
            actions.answer.mutate(
              { item: item.id, text: text.trim(), ms: Date.now() - shownAt },
              {
                onSuccess: setResult,
                onError: (e) => toastError(e, "Could not save your answer"),
              }
            )
          }
        >
          Answer
        </Button>
      ) : (
        <div
          data-testid="course-feedback"
          className="flex flex-col gap-3 border p-3"
        >
          <p className="text-[13px] font-medium">
            {result.correct ? "Correct" : result.close ? "Close" : "Not quite"}
          </p>
          <p className="text-[13px]">Answer: {result.answer}</p>
          <PromptMarkdown source={result.why} />
          {note ? <p className="text-[13px]">{note}</p> : null}
          <div className="flex flex-wrap items-center gap-2">
            <span className="text-[13px] text-muted-foreground">
              How did that feel?
            </span>
            {GRADES.map((grade) => (
              <Button
                key={grade}
                size="sm"
                variant="outline"
                onClick={() =>
                  actions.selfGrade.mutate(
                    { item: item.id, grade },
                    {
                      onSuccess: onNext,
                      onError: (e) => toastError(e, "Could not save"),
                    }
                  )
                }
              >
                {grade}
              </Button>
            ))}
            <Button size="sm" onClick={onNext}>
              Continue
            </Button>
          </div>
          <div className="flex flex-wrap items-center gap-2">
            {!result.correct && item.kind === "recall" ? (
              <Button
                size="sm"
                variant="ghost"
                disabled={actions.check.isPending}
                onClick={() =>
                  actions.check.mutate(
                    { item: item.id, text: text.trim() },
                    {
                      onSuccess: (c) => setNote(c.note),
                      onError: (e) =>
                        toastError(e, "Could not check your answer"),
                    }
                  )
                }
              >
                Check my answer
              </Button>
            ) : null}
            {flagged ? (
              <span className="text-[13px] text-muted-foreground">
                Thanks, this question is hidden from now on.
              </span>
            ) : reason === null ? (
              <Button size="sm" variant="ghost" onClick={() => setReason("")}>
                This looks wrong
              </Button>
            ) : (
              <>
                <Input
                  value={reason}
                  onChange={(e) => setReason(e.target.value)}
                  aria-label="What looks wrong?"
                  placeholder="What looks wrong?"
                  className="max-w-xs"
                />
                <Button
                  size="sm"
                  disabled={reason.trim() === ""}
                  onClick={() =>
                    actions.flag.mutate(
                      { item: item.id, reason: reason.trim() },
                      {
                        onSuccess: () => setFlagged(true),
                        onError: (e) => toastError(e, "Could not send"),
                      }
                    )
                  }
                >
                  Send
                </Button>
              </>
            )}
          </div>
        </div>
      )}
    </section>
  )
}

/** Ask-a-question; the only model call besides check. A 503 leaves the lesson untouched. */
function AskBox({ actions }: { actions: Actions }) {
  const [question, setQuestion] = useState("")
  const [reply, setReply] = useState<string | null>(null)
  const send = () => {
    const q = question.trim()
    if (q === "" || actions.ask.isPending) return
    actions.ask.mutate(q, {
      onSuccess: (r) => {
        setReply(r)
        setQuestion("")
      },
      onError: (e) => toastError(e, "Could not answer that right now"),
    })
  }
  return (
    <div data-testid="course-ask" className="flex flex-col gap-2 border-t pt-3">
      <div className="flex gap-2">
        <Input
          value={question}
          onChange={(e) => setQuestion(e.target.value)}
          onKeyDown={(e) => e.key === "Enter" && send()}
          maxLength={2000}
          aria-label="Ask a question"
          placeholder="Ask a question about this"
        />
        <Button
          variant="outline"
          onClick={send}
          disabled={actions.ask.isPending}
        >
          Ask
        </Button>
      </div>
      {reply ? <PromptMarkdown source={reply} /> : null}
    </div>
  )
}

function Done({ ws, id }: { ws: string; id: string }) {
  const usage = useUsage(ws, id, true)
  const u = usage.data
  return (
    <section data-testid="course-done" className="flex flex-col gap-2">
      <h2 className="font-heading text-[18px] font-medium">Session finished</h2>
      <p className="text-[13px]">
        Come back for your next lesson and any reviews that are due.
      </p>
      {u ? (
        <p
          data-testid="course-usage"
          className="text-[13px] text-muted-foreground"
        >
          AI used: strong {u.strongIn} in / {u.strongOut} out, cheap {u.cheapIn}{" "}
          in / {u.cheapOut} out, {u.strongCallsThisHour} strong calls this hour.
        </p>
      ) : null}
    </section>
  )
}
