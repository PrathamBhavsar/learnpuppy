import { useState } from "react"

import type { components } from "@/api/schema"
import { Choice } from "@/components/app/choice"
import { PromptMarkdown } from "@/components/app/prompt-markdown"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Label } from "@/components/ui/label"
import { Spinner } from "@/components/ui/spinner"

type Item = components["schemas"]["PlacementItemDto"]
type Graded = components["schemas"]["GradedDto"]
type Answer = components["schemas"]["PlacementAnswerDto"]

/**
 * The strand probe: one question per strand, then a follow-up round for strands that are not
 * settled yet. `verdicts` is the previous round, one line each: right or wrong, nothing taught.
 */
export function Placement({
  items,
  pending,
  verdicts,
  onSubmit,
}: {
  items: Item[]
  pending: boolean
  verdicts?: Graded[]
  onSubmit: (answers: Answer[]) => void
}) {
  const [answers, setAnswers] = useState<Record<string, string>>({})
  const [flags, setFlags] = useState<
    Record<string, { guessed?: boolean; blocked?: boolean }>
  >({})
  const complete = items.every((i) => (answers[i.id] ?? "").trim() !== "")
  return (
    <form
      data-testid="course-placement"
      className="flex flex-col gap-5"
      onSubmit={(e) => {
        e.preventDefault()
        onSubmit(
          items.map((i) => ({
            item: i.id,
            text: (answers[i.id] ?? "").trim(),
            guessed: flags[i.id]?.guessed ?? false,
            blocked: flags[i.id]?.blocked ?? false,
          }))
        )
      }}
    >
      <h2 className="font-heading text-[16px] font-medium">
        {verdicts?.length
          ? "A few follow-up questions"
          : "A quick quiz to see where you are"}
      </h2>
      {verdicts?.length ? (
        <p
          data-testid="placement-verdict"
          className="text-[13px] text-muted-foreground"
        >
          Last round: {verdicts.filter((v) => v.correct).length} of{" "}
          {verdicts.length} right.
        </p>
      ) : null}
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
          <div className="flex flex-wrap gap-x-4 gap-y-1 text-[12px] text-muted-foreground">
            {(
              [
                ["guessed", "I'm guessing"],
                ["blocked", "I'm missing a prerequisite"],
              ] as const
            ).map(([key, label]) => (
              <span key={key} className="flex items-center gap-1.5">
                <Checkbox
                  id={`${item.id}-${key}`}
                  checked={flags[item.id]?.[key] ?? false}
                  disabled={pending}
                  onCheckedChange={(v) =>
                    setFlags((f) => ({
                      ...f,
                      [item.id]: { ...f[item.id], [key]: v === true },
                    }))
                  }
                />
                <Label htmlFor={`${item.id}-${key}`}>{label}</Label>
              </span>
            ))}
          </div>
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
