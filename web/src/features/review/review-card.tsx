import { Link } from "@tanstack/react-router"
import { ArrowUpRightIcon, CheckIcon, XIcon } from "lucide-react"
import { motion, useIsPresent } from "motion/react"
import { useEffect, useRef, useState } from "react"

import type { Rating, ReviewCard } from "@/api/pending-types"
import { InfoTip } from "@/components/app/info/info"
import { Spinner } from "@/components/ui/spinner"
import { useReducedMotion } from "@/hooks/use-reduced-motion"
import { isEditable } from "@/lib/keys"
import { toastError } from "@/lib/toast"
import { cn } from "@/lib/utils"

import { Choices } from "./choices"
import { reviewInfo } from "./info"
import { useAnswerCard, useGradeCard } from "./api"
import { PromptMarkdown } from "./prompt-markdown"
import { RatingBar } from "./rating-bar"
import { resolveSessionKey } from "./session-keys"
import type { Session } from "./use-session"

/** Keys typed inside an info dialog, or on an info button, belong to that control and never rate the card. */
function inOverlayOrInfo(target: EventTarget | null): boolean {
  return (
    target instanceof Element &&
    target.closest(
      '[data-testid="info-dialog"], [data-testid="screen-guide"], button[aria-label^="About "]'
    ) !== null
  )
}

type Props = {
  ws: string
  card: ReviewCard
  session: Session
  /** False while a dialog (exit confirm) owns the keyboard. */
  keysEnabled: boolean
  /** Ask the host to open its exit confirmation (Esc). */
  onRequestExit: () => void
}

export function ReviewCardView({
  ws,
  card,
  session,
  keysEnabled,
  onRequestExit,
}: Props) {
  const reduced = useReducedMotion()
  const answerCard = useAnswerCard(ws)
  const gradeCard = useGradeCard(ws)
  const [pressed, setPressed] = useState<Rating | null>(null)
  const present = useIsPresent()
  const submitting = useRef(false)
  const { state } = session
  const answered = state.phase === "answered"
  const result = state.result
  const checking = answerCard.isPending

  const choose = (key: string) => {
    if (submitting.current || checking || state.phase !== "answering") return
    submitting.current = true
    session.choosing(card.id, key)
    void answerCard
      .mutateAsync({ cardId: card.id, choiceKey: key })
      .then((res) => session.answer(res, key, card.id))
      .catch((err) => toastError(err, "Could not check your answer"))
      .finally(() => {
        submitting.current = false
      })
  }

  const rate = (rating: Rating) => {
    if (
      submitting.current ||
      !result ||
      !state.choiceKey ||
      state.phase !== "answered"
    )
      return
    submitting.current = true
    const failed = { card, result, choiceKey: state.choiceKey, rating }
    session.grade(rating)
    // The promise survives this card's exit/unmount; observer callbacks do not.
    void gradeCard
      .mutateAsync({ cardId: card.id, rating })
      .catch((err) => {
        session.revert(failed)
        toastError(err, "Could not save rating")
      })
      .finally(() => {
        submitting.current = false
      })
  }

  // One window listener that always sees the latest handlers.
  const latest = useRef({ choose, rate })
  useEffect(() => {
    latest.current = { choose, rate }
  })
  const phase = state.phase
  const optionCount = card.options?.length ?? 0
  const suggestion = result?.rating
  useEffect(() => {
    if (!keysEnabled || !present) return
    const onKey = (event: KeyboardEvent) => {
      if (
        event.repeat ||
        isEditable(event.target) ||
        inOverlayOrInfo(event.target)
      )
        return
      const action = resolveSessionKey(phase, optionCount, event)
      if (!action) return
      event.preventDefault()
      if (action.type === "exit") onRequestExit()
      else if (action.type === "choose") {
        const option = card.options?.[action.index]
        if (option) latest.current.choose(option.key)
      } else {
        const rating = action.type === "accept" ? suggestion : action.rating
        if (!rating) return
        setPressed(rating)
        window.setTimeout(() => setPressed(null), 140)
        latest.current.rate(rating)
      }
    }
    window.addEventListener("keydown", onKey)
    return () => window.removeEventListener("keydown", onKey)
  }, [
    keysEnabled,
    present,
    phase,
    optionCount,
    suggestion,
    card.options,
    onRequestExit,
  ])

  const correct = result?.correct ?? false

  return (
    <article
      data-testid="review-card"
      data-card-id={card.id}
      data-active={present}
      aria-hidden={!present || undefined}
      inert={!present || undefined}
      data-phase={phase}
      className="mx-auto flex w-full max-w-2xl flex-col gap-5"
    >
      <div className="flex flex-col gap-3">
        <div className="flex items-center gap-2 text-[12px] text-muted-foreground">
          {card.sourceDecisionId ? (
            <Link
              to="/w/$ws/digest"
              params={{ ws }}
              search={{ decision: card.sourceDecisionId }}
              data-testid="review-card-source"
              className="inline-flex items-center gap-1 text-primary hover:underline"
            >
              Source decision
              <ArrowUpRightIcon className="size-3 rtl:-scale-x-100" />
            </Link>
          ) : null}
          {card.sourceDecisionId ? <span aria-hidden>·</span> : null}
          <span data-testid="review-card-state">{card.state}</span>
          <InfoTip info={reviewInfo.card} />
        </div>
        <div
          data-testid="review-card-prompt"
          className="border bg-card p-5 text-[15px] leading-relaxed"
        >
          <PromptMarkdown source={card.front} />
        </div>
      </div>

      <Choices
        card={card}
        pending={checking}
        chosen={state.choiceKey}
        correctKey={result?.correctKey ?? null}
        answered={answered}
        onSelect={choose}
      />

      {checking ? (
        <div
          role="status"
          className="flex items-center gap-2 text-[12px] text-muted-foreground"
        >
          <Spinner data-icon="inline-start" />
          Checking answer…
        </div>
      ) : null}

      {answered && result ? (
        <motion.div
          data-testid="review-reveal"
          initial={reduced ? false : { opacity: 0, y: 12 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.22, ease: [0.22, 1, 0.36, 1] }}
          className="flex flex-col gap-4"
        >
          <div
            data-testid="review-verdict"
            data-correct={correct ? "true" : "false"}
            role="status"
            className={cn(
              "flex items-start gap-2.5 border p-4",
              correct
                ? "border-success bg-success/5"
                : "border-destructive bg-destructive/5"
            )}
          >
            {correct ? (
              <CheckIcon className="mt-0.5 size-4 shrink-0 text-success" />
            ) : (
              <XIcon className="mt-0.5 size-4 shrink-0 text-destructive" />
            )}
            <div className="flex min-w-0 flex-col gap-1">
              <p className="flex items-center gap-1.5 text-[13px] font-medium">
                {correct ? "Correct" : "Not quite"}
                <InfoTip info={reviewInfo.verdict} />
              </p>
              <PromptMarkdown
                source={result.explanation}
                className="text-[13px] text-muted-foreground"
              />
            </div>
          </div>

          <div className="flex flex-col gap-1.5">
            <p className="flex items-center gap-1.5 text-[12px] text-muted-foreground">
              How well did you recall it?
              <InfoTip info={reviewInfo.rating} />
            </p>
            <RatingBar
              preview={result.preview}
              items={result.previewItems}
              suggestion={result.rating}
              pressed={pressed}
              onRate={rate}
            />
          </div>
        </motion.div>
      ) : null}
    </article>
  )
}
