import { useDraft } from "./drafts"
import { SendIcon } from "lucide-react"
import { useState } from "react"

import type { AnswerInput, PendingQuestion } from "@/api/pending-types"
import { InfoTip } from "@/components/app/info/info"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { useHotkey, useHotkeys } from "@/hooks/use-hotkey"

import { classroomInfo } from "../info"
import { ConfidenceSlider } from "./ConfidenceSlider"
import { OptionList } from "./OptionList"
import { ThinkingField, useThinking } from "./ThinkingField"

type ChoiceQuestionProps = {
  pending: PendingQuestion
  busy: boolean
  onSubmit: (answer: AnswerInput) => void | Promise<void>
}

/** Quiz, option-style probes and `ask_user_question` with options. Keys a-d / 1-9 pick, Enter submits. */
export function ChoiceQuestion({
  pending,
  busy,
  onSubmit,
}: ChoiceQuestionProps) {
  const options = pending.options ?? []
  const idk = pending.idkKey ?? null
  const scale = pending.confidenceScale ?? null
  const [picked, setPicked] = useState<string[]>([])
  const { text, setText, submit: submitDraft } = useDraft(pending.callId)
  const think = useThinking(pending.callId)
  const [confidence, setConfidence] = useState(3)

  const toggle = (key: string) =>
    setPicked((prev) => {
      if (!pending.multiSelect) return [key]
      const without = prev.filter((k) => k !== key && k !== idk)
      return prev.includes(key) ? without : [...without, key]
    })

  const canSubmit =
    !busy &&
    (picked.length > 0 || (pending.allowFreeText && text.trim() !== ""))
  const submit = () => {
    if (!canSubmit) return
    void submitDraft(async () => {
      const notes = think.text.trim()
      await think.submit(() =>
        onSubmit({
          choice: picked,
          text: text.trim() || undefined,
          readingNotes: notes || undefined,
          confidence: scale === null ? undefined : confidence,
        })
      )
    })
  }

  useHotkey("mod+enter", submit, {
    id: "answer-submit",
    label: "Submit answer",
    group: "Learn",
    enabled: canSubmit,
  })

  // Enter submits unless it is already activating a button (Submit, "I don't know").
  useHotkey(
    "enter",
    (event) => {
      const target = event.target
      if (
        target instanceof HTMLButtonElement &&
        target.getAttribute("role") === null
      )
        return
      event.preventDefault()
      submit()
    },
    { enabled: !busy }
  )

  useHotkeys(
    options.slice(0, 9).flatMap((option, index) => {
      const handler = () => !busy && toggle(option.key)
      const bindings = [
        { id: `choice-n-${index}`, keys: String(index + 1), handler },
      ]
      return index < 4
        ? [
            ...bindings,
            {
              id: `choice-l-${index}`,
              keys: String.fromCharCode(97 + index),
              handler,
            },
          ]
        : bindings
    })
  )

  return (
    <div className="flex flex-col gap-3">
      <OptionList
        tool={pending.tool}
        options={options}
        picked={picked}
        multiple={pending.multiSelect}
        disabled={busy}
        onToggle={toggle}
        label={pending.multiSelect ? "Choose all that apply" : "Choose one"}
      />
      {idk ? (
        <div className="flex items-center gap-1.5 self-start">
          <Button
            type="button"
            variant="ghost"
            size="sm"
            className="text-muted-foreground"
            disabled={busy}
            aria-pressed={picked.includes(idk)}
            onClick={() => setPicked([idk])}
            data-testid={`${pending.tool}-idk`}
          >
            I don&apos;t know
          </Button>
          <InfoTip info={classroomInfo.idk} testId="info-idk" />
        </div>
      ) : null}
      {pending.allowFreeText ? (
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
          rows={2}
          placeholder="Or answer in your own words"
          aria-label="Your own words"
          data-testid={`${pending.tool}-text`}
          className="min-h-0"
        />
      ) : null}
      <ThinkingField
        callId={pending.callId}
        busy={busy}
        value={think.text}
        onChange={think.setText}
      />
      {scale !== null ? (
        <ConfidenceSlider
          value={confidence}
          max={scale}
          onChange={setConfidence}
          disabled={busy}
        />
      ) : null}
      <div className="sticky bottom-0 z-10 flex justify-end bg-card py-2">
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
  )
}
