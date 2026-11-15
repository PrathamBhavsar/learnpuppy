import { HelpCircleIcon } from "lucide-react"

import type { PendingQuestion } from "@/api/pending-types"

import { toolLabel } from "../copy"
import { EventCard } from "./EventCard"

/**
 * A question is waiting. Shows the prompt only: never the options, so nothing here can reveal the
 * right answer before it is submitted (the DTO leak rule). Answered questions use `QuestionRecap`.
 */
export function QuizAskedCard({
  question,
}: {
  question: Pick<PendingQuestion, "tool" | "prompt">
}) {
  return (
    <EventCard
      kind="quiz"
      icon={HelpCircleIcon}
      label={toolLabel(question.tool)}
      summary={question.prompt.replace(/\s+/g, " ").trim()}
      status="asked"
    />
  )
}
