import { useState } from "react"

import type { components } from "@/api/schema"
import { PromptMarkdown } from "@/components/app/prompt-markdown"
import { Button } from "@/components/ui/button"
import { Input } from "@/components/ui/input"
import { Spinner } from "@/components/ui/spinner"

type Item = components["schemas"]["PlacementItemDto"]

/** Options as radio buttons (mcq) or a text box (recall). The server grades; nothing is checked here. */
export function Choice({
  item,
  value,
  disabled,
  onChange,
}: {
  item: { id: string; kind: "mcq" | "recall"; options: string[] }
  value: string
  disabled?: boolean
  onChange: (text: string) => void
}) {
  if (item.kind === "recall")
    return (
      <Input
        value={value}
        disabled={disabled}
        onChange={(e) => onChange(e.target.value)}
        aria-label="Your answer"
        placeholder="Type your answer, or write: I don't know"
      />
    )
  return (
    <div
      role="radiogroup"
      aria-label="Options"
      className="flex flex-col gap-1.5"
    >
      {item.options.map((option) => (
        <label
          key={option}
          className="flex cursor-pointer items-start gap-2 border px-3 py-2 text-[13px] has-checked:border-primary has-checked:bg-primary/10"
        >
          <input
            type="radio"
            name={item.id}
            checked={value === option}
            disabled={disabled}
            onChange={() => onChange(option)}
          />
          {option}
        </label>
      ))}
    </div>
  )
}

export function Placement({
  items,
  pending,
  onSubmit,
}: {
  items: Item[]
  pending: boolean
  onSubmit: (answers: { item: string; text: string }[]) => void
}) {
  const [answers, setAnswers] = useState<Record<string, string>>({})
  const complete = items.every((i) => (answers[i.id] ?? "").trim() !== "")
  return (
    <form
      data-testid="course-placement"
      className="flex flex-col gap-5"
      onSubmit={(e) => {
        e.preventDefault()
        onSubmit(
          items.map((i) => ({ item: i.id, text: (answers[i.id] ?? "").trim() }))
        )
      }}
    >
      <h2 className="font-heading text-[16px] font-medium">
        A quick quiz to see where you are
      </h2>
      {items.map((item, n) => (
        <fieldset key={item.id} className="flex flex-col gap-2">
          <legend className="mb-1 text-[13px] font-medium">
            {n + 1}. <PromptMarkdown source={item.q} className="inline" />
          </legend>
          <Choice
            item={item}
            value={answers[item.id] ?? ""}
            disabled={pending}
            onChange={(text) => setAnswers((a) => ({ ...a, [item.id]: text }))}
          />
        </fieldset>
      ))}
      <Button
        type="submit"
        disabled={!complete || pending}
        className="self-start"
      >
        {pending ? <Spinner /> : null}
        Check my answers
      </Button>
    </form>
  )
}
