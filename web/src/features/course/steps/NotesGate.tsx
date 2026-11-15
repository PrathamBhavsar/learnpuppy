import { useState } from "react"

import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"

/** Notes before the quiz. The gate is the server's; this only sends what was typed. */
export function NotesGate({
  title,
  busy,
  onSubmit,
}: {
  title: string
  busy: boolean
  onSubmit: (text: string) => void
}) {
  const [text, setText] = useState("")
  return (
    <section data-testid="step-notes" className="flex flex-col gap-3">
      <h2 className="font-heading text-[16px] font-medium">{title}</h2>
      <p className="text-[13px]">
        Write what you remember in your own words before the questions.
      </p>
      <Textarea
        value={text}
        onChange={(e) => setText(e.target.value)}
        aria-label="Your notes"
        rows={5}
      />
      <Button
        className="self-start"
        disabled={busy || text.trim() === ""}
        onClick={() => onSubmit(text.trim())}
      >
        Start the questions
      </Button>
    </section>
  )
}
