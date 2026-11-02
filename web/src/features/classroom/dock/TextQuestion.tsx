import { useDraft } from "./drafts"
import { SendIcon } from "lucide-react"
import { useHotkey } from "@/hooks/use-hotkey"

import type { AnswerInput, PendingQuestion } from "@/api/pending-types"
import { InfoTip } from "@/components/app/info/info"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"

import { classroomInfo } from "../info"

type TextQuestionProps = {
  pending: PendingQuestion
  busy: boolean
  onSubmit: (answer: AnswerInput) => void | Promise<void>
}

const MIN_WORDS = 12

const HINTS: Record<string, string> = {
  probe: "Answer in your own words. A guess is fine; it shapes the plan.",
  recall: "Say what you remember. Skip it if nothing comes to mind.",
  explain: `Explain it in your own words, as if to a colleague. Around ${MIN_WORDS} words or more works well.`,
  ask_user_question: "Type your answer.",
}

function wordCount(text: string): number {
  const trimmed = text.trim()
  return trimmed === "" ? 0 : trimmed.split(/\s+/).length
}

/** Free-text questions: probe, recall (with "I don't know"), explain and `ask_user_question`. */
export function TextQuestion({ pending, busy, onSubmit }: TextQuestionProps) {
  const { text, setText, submit: submitDraft } = useDraft(pending.callId)
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
  const words = wordCount(text)
  return (
    <div className="flex flex-col gap-2">
      <Textarea
        value={text}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (
            event.key === "Enter" &&
            !event.shiftKey &&
            !event.nativeEvent.isComposing
          ) {
            event.preventDefault()
            submit()
          }
        }}
        disabled={busy}
        dir="auto"
        rows={3}
        aria-label="Your answer"
        placeholder={
          pending.tool === "explain"
            ? "Explain in your own words"
            : "Your answer"
        }
        data-testid={`${pending.tool}-text`}
        className="max-h-48 min-h-16"
      />
      <div className="sticky bottom-0 z-10 flex items-center justify-between gap-2 bg-card py-2">
        <p className="text-[12px] text-muted-foreground">
          {HINTS[pending.tool] ?? HINTS["ask_user_question"]}
          {pending.tool === "explain" ? (
            <span className="ms-1 tabular-nums" data-testid="explain-words">
              {words} / {MIN_WORDS} words
            </span>
          ) : null}
        </p>
        <div className="flex shrink-0 items-center gap-2">
          {pending.tool === "recall" ? (
            <>
              <InfoTip info={classroomInfo.idk} testId="info-idk" />
              <Button
                type="button"
                variant="ghost"
                disabled={busy}
                onClick={() => {
                  void submitDraft(() => onSubmit({ text: "I don't know" }))
                }}
                data-testid="recall-idk"
              >
                I don&apos;t know
              </Button>
            </>
          ) : null}
          <Button
            onClick={submit}
            disabled={!canSubmit}
            data-testid={`${pending.tool}-submit`}
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
    </div>
  )
}
