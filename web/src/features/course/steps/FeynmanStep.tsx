import { useState } from "react"

import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"

/** Explain the lesson in your own words; the server saves it and says what it found. */
export function FeynmanStep({
  title,
  keyFacts,
  busy,
  note,
  onSubmit,
  onContinue,
}: {
  title: string
  keyFacts: string[]
  busy: boolean
  note: string | null
  onSubmit: (explanation: string) => void
  onContinue: () => void
}) {
  const [text, setText] = useState("")
  return (
    <section data-testid="step-feynman" className="flex flex-col gap-3">
      <h2 className="font-heading text-[16px] font-medium">{title}</h2>
      <p className="text-[13px]">
        Explain it as if to someone new, without looking back.
      </p>
      {keyFacts.length ? (
        <p className="text-[13px] text-muted-foreground">
          Cover: {keyFacts.join("; ")}
        </p>
      ) : null}
      <Textarea
        value={text}
        disabled={busy || note !== null}
        onChange={(e) => setText(e.target.value)}
        aria-label="Your explanation"
        rows={6}
      />
      {note === null ? (
        <Button
          className="self-start"
          disabled={busy || text.trim() === ""}
          onClick={() => onSubmit(text.trim())}
        >
          Submit
        </Button>
      ) : (
        <div className="flex flex-col gap-2">
          <p data-testid="step-feynman-note" className="text-[13px]">
            {note}
          </p>
          <Button className="self-start" onClick={onContinue}>
            Continue
          </Button>
        </div>
      )}
    </section>
  )
}
