import { useEffect, useRef, useState } from "react"

import type {
  CloseReport,
  CoursePlan,
  MethodAnswerResult,
  MethodStep,
} from "@/api/method-types"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { toastError } from "@/lib/toast"

import { Close } from "./Close"
import { MasteryBar } from "./MasteryBar"
import { type MethodActions } from "./method-api"
import { PlanReview } from "./PlanReview"
import { RegisterPanel } from "./RegisterPanel"
import { BreakPrompt } from "./steps/BreakPrompt"
import { ExplainStep } from "./steps/ExplainStep"
import { FeynmanStep } from "./steps/FeynmanStep"
import { NotesGate } from "./steps/NotesGate"
import { PredictStep } from "./steps/PredictStep"
import { type AnswerDraft, QuizStep } from "./steps/QuizStep"
import { RecallStep } from "./steps/RecallStep"

/**
 * Renders whatever step the server hands out. Which step is next, what is allowed and how anything
 * is graded are the server's; this component only shows a step, sends what the learner did, and asks again.
 */
export function MethodLesson({
  ws,
  id,
  plan,
  actions,
}: {
  ws: string
  id: string
  plan: CoursePlan
  actions: MethodActions
}) {
  const [step, setStep] = useState<MethodStep | null>(null)
  const [version, setVersion] = useState(0)
  const [showRegister, setShowRegister] = useState(false)
  const [report, setReport] = useState<CloseReport | null>(null)
  const started = useRef(false)
  const { step: next } = actions

  const advance = () =>
    next.mutate(undefined, {
      onSuccess: (s) => {
        setStep(s)
        setVersion((v) => v + 1)
      },
      onError: (e) => toastError(e, "Could not load the next step"),
    })

  useEffect(() => {
    if (started.current) return
    started.current = true
    advance()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [])

  // Every action: send, then ask the server what comes next.
  const then = (message: string) => ({
    onSuccess: advance,
    onError: (e: unknown) => toastError(e, message),
  })

  const busy = next.isPending

  return (
    <div data-testid="course-lesson" className="flex flex-col gap-4">
      <MasteryBar ws={ws} id={id} version={version} />
      {step === null ? <Spinner /> : null}
      {step?.step === "confirm_plan" ? (
        <PlanReview
          plan={plan}
          busy={busy || actions.confirmPlan.isPending}
          onConfirm={() =>
            actions.confirmPlan.mutate(undefined, then("Could not confirm"))
          }
        />
      ) : null}
      {step?.step === "predict" ? (
        <PredictStep
          key={step.lesson}
          title={step.title}
          q={step.q}
          options={step.options}
          busy={busy || actions.predict.isPending}
          onSubmit={(guess) =>
            actions.predict.mutate(
              { lesson: step.lesson, guess },
              then("Could not save your guess")
            )
          }
        />
      ) : null}
      {step?.step === "explain" ? (
        <ExplainStep
          key={`${step.id}-${step.rungText ?? ""}`}
          lesson={step}
          busy={busy || actions.explainSeen.isPending}
          onSeen={() =>
            actions.explainSeen.mutate(step.id, then("Could not save progress"))
          }
        />
      ) : null}
      {step?.step === "notes" ? (
        <NotesGate
          key={step.lesson}
          title={step.title}
          busy={busy || actions.note.isPending}
          onSubmit={(text) =>
            actions.note.mutate(
              { lesson: step.lesson, text },
              then("Could not save your notes")
            )
          }
        />
      ) : null}
      {step?.step === "ask" ? (
        <AskStep
          key={`${step.id}-${step.ask}`}
          step={step}
          actions={actions}
          onNext={advance}
        />
      ) : null}
      {step?.step === "break" ? (
        <BreakPrompt
          title={step.title}
          busy={busy || actions.rest.isPending}
          onRested={() =>
            actions.rest.mutate(step.lesson, then("Could not save"))
          }
        />
      ) : null}
      {step?.step === "feynman" ? (
        <Feynman
          key={step.lesson}
          step={step}
          actions={actions}
          onNext={advance}
        />
      ) : null}
      {step?.step === "close" ? (
        <Close actions={actions} report={report} onReport={setReport} />
      ) : null}
      {step ? (
        <div className="border-t pt-3">
          <Button
            variant="ghost"
            size="sm"
            onClick={() => setShowRegister((v) => !v)}
          >
            {showRegister ? "Hide misconceptions" : "My misconceptions"}
          </Button>
          {showRegister ? <RegisterPanel ws={ws} id={id} /> : null}
        </div>
      ) : null}
    </div>
  )
}

function AskStep({
  step,
  actions,
  onNext,
}: {
  step: Extract<MethodStep, { step: "ask" }>
  actions: MethodActions
  onNext: () => void
}) {
  const [result, setResult] = useState<MethodAnswerResult | null>(null)
  const submit = (d: AnswerDraft) =>
    actions.answer.mutate(
      {
        item: step.id,
        lesson: step.lesson,
        text: d.text,
        confidence: d.confidence,
        ms: d.ms,
        reasoning: d.reasoning,
        gap: d.gap,
      },
      {
        onSuccess: setResult,
        onError: (e) => toastError(e, "Could not save your answer"),
      }
    )
  if (result)
    return (
      <section data-testid="step-feedback" className="flex flex-col gap-3">
        <p className="text-[13px] font-medium">
          {result.status === "pending"
            ? "Saved. This one will be checked later."
            : result.correct
              ? "Correct"
              : "Not quite"}
        </p>
        <Button className="self-start" onClick={onNext}>
          Continue
        </Button>
      </section>
    )
  const common = {
    item: step,
    busy: actions.answer.isPending,
    onSubmit: submit,
  }
  return step.kind === "recall" ? (
    <RecallStep {...common} />
  ) : (
    <QuizStep {...common} />
  )
}

function Feynman({
  step,
  actions,
  onNext,
}: {
  step: Extract<MethodStep, { step: "feynman" }>
  actions: MethodActions
  onNext: () => void
}) {
  const [note, setNote] = useState<string | null>(null)
  return (
    <FeynmanStep
      title={step.title}
      keyFacts={step.keyFacts}
      busy={actions.feynman.isPending}
      note={note}
      onSubmit={(explanation) =>
        actions.feynman.mutate(
          { lesson: step.lesson, explanation },
          {
            onSuccess: (r) => setNote(r.note),
            onError: (e) => toastError(e, "Could not save your explanation"),
          }
        )
      }
      onContinue={onNext}
    />
  )
}
