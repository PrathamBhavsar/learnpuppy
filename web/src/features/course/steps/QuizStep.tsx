import { useState } from "react"

import type { MethodItem } from "@/api/method-types"
import { PromptMarkdown } from "@/components/app/prompt-markdown"
import { Button } from "@/components/ui/button"
import { Textarea } from "@/components/ui/textarea"

import { Choice } from "@/components/app/choice"

export type AnswerDraft = {
  text: string
  confidence: number | null
  reasoning: string
  gap: boolean
  ms: number
}

const CONFIDENCE = [
  { value: 1, label: "Guessing" },
  { value: 2, label: "Fairly sure" },
  { value: 3, label: "Certain" },
] as const

/** Choose among the claims, say how sure you are, or admit you do not know. The payload holds no answer. */
export function QuizStep({
  item,
  busy,
  onSubmit,
}: {
  item: MethodItem
  busy: boolean
  onSubmit: (draft: AnswerDraft) => void
}) {
  const [text, setText] = useState("")
  const [confidence, setConfidence] = useState<number | null>(null)
  const [reasoning, setReasoning] = useState("")
  const [shownAt] = useState(() => Date.now())
  const send = (gap: boolean) =>
    onSubmit({
      text: gap ? "" : text.trim(),
      confidence: gap ? null : confidence,
      reasoning: reasoning.trim(),
      gap,
      ms: Date.now() - shownAt,
    })
  return (
    <section data-testid="step-quiz" className="flex flex-col gap-3">
      {item.angle ? (
        <div data-testid="step-angle" className="border p-3">
          <p className="mb-1 text-[13px] font-medium">Another way to see it</p>
          <PromptMarkdown source={item.angle} />
        </div>
      ) : null}
      <PromptMarkdown source={item.q} />
      <Choice item={item} value={text} disabled={busy} onChange={setText} />
      <Textarea
        value={reasoning}
        onChange={(e) => setReasoning(e.target.value)}
        aria-label="Why do you think so?"
        placeholder="Why do you think so? (optional)"
        rows={2}
      />
      <div
        role="radiogroup"
        aria-label="How sure are you?"
        className="flex flex-wrap items-center gap-3 text-[13px]"
      >
        <span className="text-muted-foreground">How sure are you?</span>
        {CONFIDENCE.map((c) => (
          <label key={c.value} className="flex items-center gap-1">
            <input
              type="radio"
              name={`conf-${item.id}`}
              checked={confidence === c.value}
              disabled={busy}
              onChange={() => setConfidence(c.value)}
            />
            {c.label}
          </label>
        ))}
      </div>
      <div className="flex gap-2">
        <Button
          disabled={busy || text.trim() === "" || confidence === null}
          onClick={() => send(false)}
        >
          Answer
        </Button>
        <Button variant="outline" disabled={busy} onClick={() => send(true)}>
          I don&apos;t know
        </Button>
      </div>
    </section>
  )
}
