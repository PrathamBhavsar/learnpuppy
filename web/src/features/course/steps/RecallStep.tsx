import { useState } from "react"

import type { MethodItem } from "@/api/method-types"
import { PromptMarkdown } from "@/components/app/prompt-markdown"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"

import type { AnswerDraft } from "./QuizStep"

/** Type the answer from memory. The server matches it against its own rubric. */
export function RecallStep({
  item,
  busy,
  onSubmit,
}: {
  item: MethodItem
  busy: boolean
  onSubmit: (draft: AnswerDraft) => void
}) {
  const [text, setText] = useState("")
  const [shownAt] = useState(() => Date.now())
  const send = (gap: boolean) =>
    onSubmit({
      text: gap ? "" : text.trim(),
      confidence: null,
      reasoning: "",
      gap,
      ms: Date.now() - shownAt,
    })
  return (
    <section data-testid="step-recall" className="flex flex-col gap-3">
      {item.angle ? (
        <div data-testid="step-angle" className="border p-3">
          <PromptMarkdown source={item.angle} />
        </div>
      ) : null}
      <PromptMarkdown source={item.q} />
      <Input
        value={text}
        disabled={busy}
        onChange={(e) => setText(e.target.value)}
        aria-label="Your answer"
        placeholder="Type your answer"
      />
      <div className="flex gap-2">
        <Button
          disabled={busy || text.trim() === ""}
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
