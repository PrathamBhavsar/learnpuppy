import { MicIcon, SquareIcon } from "lucide-react"
import { useEffect, useState } from "react"

import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"

import { useDraft } from "./drafts"
import { useDictation } from "./use-dictation"

type ThinkingFieldProps = {
  /** Draft is keyed per question so it survives reloads like the answer. */
  callId: string
  busy: boolean
  value: string
  onChange: (text: string) => void
}

/** Draft state for the optional "explain your thinking" field. */
export function useThinking(callId: string) {
  return useDraft(`${callId}:think`)
}

/** Optional notes under a question; never blocks submit. Mic only where the browser supports it. */
export function ThinkingField({
  callId,
  busy,
  value,
  onChange,
}: ThinkingFieldProps) {
  const dictation = useDictation((spoken) =>
    onChange(value ? `${value.replace(/\s+$/, "")} ${spoken}` : spoken)
  )
  const [notice, setNotice] = useState(false)
  const { listening, stop } = dictation
  useEffect(() => {
    if (busy) stop()
  }, [busy, stop])

  return (
    <div
      className="flex flex-col gap-1.5"
      onBlur={(event) => {
        if (!event.currentTarget.contains(event.relatedTarget)) stop()
      }}
    >
      <div className="flex items-start gap-2">
        <Textarea
          value={value}
          onChange={(event) => onChange(event.target.value)}
          disabled={busy}
          dir="auto"
          rows={2}
          placeholder="Explain your thinking (optional)"
          aria-label="Explain your thinking"
          data-testid={`think-${callId}`}
          className="min-h-0"
        />
        {dictation.supported ? (
          <Button
            type="button"
            variant={listening ? "secondary" : "ghost"}
            size="icon"
            disabled={busy}
            aria-pressed={listening}
            aria-label={listening ? "Stop dictation" : "Dictate"}
            data-testid="think-mic"
            onClick={() => {
              if (listening) stop()
              else if (!dictation.enabled) setNotice(true)
              else dictation.start()
            }}
          >
            {listening ? <SquareIcon /> : <MicIcon />}
          </Button>
        ) : null}
      </div>
      {notice && !dictation.enabled ? (
        <div
          role="note"
          className="flex items-center justify-between gap-2 text-[12px] text-muted-foreground"
          data-testid="think-notice"
        >
          <span>
            Dictation uses your browser&apos;s speech service (in Chrome, audio
            goes to Google), not LearnPuppy.
          </span>
          <Button
            type="button"
            size="sm"
            data-testid="think-enable"
            onClick={() => {
              dictation.enable()
              setNotice(false)
              dictation.start()
            }}
          >
            Turn on
          </Button>
        </div>
      ) : null}
    </div>
  )
}
