import { useState } from "react"

import { PromptMarkdown } from "@/components/app/prompt-markdown"
import { Button } from "@/components/ui/button"

import { Choice } from "../Placement"

/** Guess before the explanation. The guess is stored; the server never marks it wrong. */
export function PredictStep({
  title,
  q,
  options,
  busy,
  onSubmit,
}: {
  title: string
  q: string
  options: string[]
  busy: boolean
  onSubmit: (guess: string) => void
}) {
  const [guess, setGuess] = useState("")
  return (
    <section data-testid="step-predict" className="flex flex-col gap-3">
      <h2 className="font-heading text-[18px] font-medium">{title}</h2>
      <p className="text-[13px] text-muted-foreground">
        Take a guess first. Nothing is marked.
      </p>
      <PromptMarkdown source={q} />
      <Choice
        item={{
          id: "predict",
          kind: options.length ? "mcq" : "recall",
          options,
        }}
        value={guess}
        disabled={busy}
        onChange={setGuess}
      />
      <div className="flex gap-2">
        <Button
          disabled={busy || guess.trim() === ""}
          onClick={() => onSubmit(guess.trim())}
        >
          Show the explanation
        </Button>
        <Button variant="ghost" disabled={busy} onClick={() => onSubmit("")}>
          Skip
        </Button>
      </div>
    </section>
  )
}
