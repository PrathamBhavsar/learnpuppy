import { PromptMarkdown } from "@/components/app/prompt-markdown"
import { Button } from "@/components/ui/button"

import { RungStep } from "./RungStep"

type Lesson = {
  id: string
  title: string
  hook: string
  explain: string
  keyFacts: string[]
  rungText?: string | null
}

export function ExplainStep({
  lesson,
  busy,
  onSeen,
}: {
  lesson: Lesson
  busy: boolean
  onSeen: () => void
}) {
  return (
    <article data-testid="step-explain" className="flex flex-col gap-3">
      <h2 className="font-heading text-[18px] font-medium">{lesson.title}</h2>
      {lesson.hook ? (
        <p className="text-[13px] text-muted-foreground">{lesson.hook}</p>
      ) : null}
      <PromptMarkdown source={lesson.explain} />
      {lesson.rungText ? <RungStep text={lesson.rungText} /> : null}
      {lesson.keyFacts.length ? (
        <ul className="list-disc ps-5 text-[13px]">
          {lesson.keyFacts.map((f) => (
            <li key={f}>{f}</li>
          ))}
        </ul>
      ) : null}
      <Button onClick={onSeen} disabled={busy} className="self-start">
        Got it
      </Button>
    </article>
  )
}
