import { useState } from "react"

import { toApiError } from "@/api/errors"
import type { components } from "@/api/schema"
import { ErrorState } from "@/components/app/error-state"
import { PromptMarkdown } from "@/components/app/prompt-markdown"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { toastError } from "@/lib/toast"

import {
  useCourseActions,
  useCourseStatus,
  useMisses,
  usePlacementItems,
} from "./api"
import { Lesson } from "./Lesson"
import { useCoursePlan, useMethodActions } from "./method-api"
import { MethodLesson } from "./MethodLesson"
import { Placement } from "./Placement"

type Dto = components["schemas"]
type Graded = Dto["GradedDto"]

type Props = {
  ws: string
  id: string
  onBack: () => void
}

/** Placement, the wait while the course is written, then the lesson loop. */
export function CourseView({ ws, id, onBack }: Props) {
  const actions = useCourseActions(ws, id)
  const methodActions = useMethodActions(ws, id)
  const plan = useCoursePlan(ws, id)
  const [graded, setGraded] = useState<Graded[] | null>(null)
  const items = usePlacementItems(ws, id)
  const placement = items.data
  const placing = (placement?.length ?? 0) > 0 && graded === null
  const status = useCourseStatus(ws, id, placement !== undefined && !placing)
  const state = status.data?.state
  const [started, setStarted] = useState(false)

  const back = (
    <Button variant="ghost" size="sm" onClick={onBack} className="self-start">
      All courses
    </Button>
  )

  if (placement === undefined)
    return (
      <Shell>
        {back}
        {items.isError ? (
          <ErrorState
            message={toApiError(items.error).message}
            onRetry={() => items.refetch()}
          />
        ) : (
          <Spinner />
        )}
      </Shell>
    )

  if (placing)
    return (
      <Shell>
        {back}
        <Placement
          items={placement}
          pending={actions.placement.isPending}
          onSubmit={(answers) =>
            actions.placement.mutate(answers, {
              onSuccess: (g) => {
                setGraded(g)
              },
              onError: (e) => toastError(e, "Could not check your answers"),
            })
          }
        />
      </Shell>
    )

  if (started && (state === "ready" || state === "refilling"))
    return (
      <Shell>
        {back}
        {plan.isPending ? (
          <Spinner />
        ) : plan.data ? (
          <MethodLesson
            ws={ws}
            id={id}
            plan={plan.data}
            actions={methodActions}
          />
        ) : (
          <Lesson ws={ws} id={id} actions={actions} />
        )}
      </Shell>
    )

  return (
    <Shell>
      {back}
      <Wait
        ws={ws}
        id={id}
        graded={graded}
        state={state}
        reason={status.data?.reason ?? undefined}
        onRetry={() =>
          actions.retry.mutate(undefined, {
            onError: (e) => toastError(e, "Could not retry"),
          })
        }
        onStart={() => setStarted(true)}
      />
    </Shell>
  )
}

function Shell({ children }: { children: React.ReactNode }) {
  return (
    <div
      data-testid="course-view"
      className="mx-auto flex max-w-2xl flex-col gap-4 p-6"
    >
      {children}
    </div>
  )
}

/** While the Author writes, the learner reads what placement already taught: their misses and why. */
function Wait({
  ws,
  id,
  graded,
  state,
  reason,
  onRetry,
  onStart,
}: {
  ws: string
  id: string
  graded: Graded[] | null
  state: string | undefined
  reason: string | undefined
  onRetry: () => void
  onStart: () => void
}) {
  const misses = useMisses(ws, id, true)
  const right = graded?.filter((g) => g.correct).length
  const ready = state === "ready" || state === "refilling"
  return (
    <section data-testid="course-wait" className="flex flex-col gap-4">
      {graded ? (
        <p className="text-[13px] text-muted-foreground">
          You got {right} of {graded.length} right.
        </p>
      ) : null}
      <div className="flex items-center gap-2" role="status">
        {state === "failed" ? (
          <>
            <span className="text-[13px] text-destructive">
              The course could not be written{reason ? `: ${reason}` : "."}
            </span>
            <Button size="sm" onClick={onRetry}>
              Try again
            </Button>
          </>
        ) : ready ? (
          <>
            <span className="text-[13px]">Your course is ready.</span>
            <Button size="sm" onClick={onStart}>
              Start lesson
            </Button>
          </>
        ) : (
          <>
            <Spinner />
            <span className="text-[13px]">
              Writing your lessons. Read what you missed meanwhile.
            </span>
          </>
        )}
      </div>
      {misses.data?.length ? (
        <ul data-testid="course-misses" className="flex flex-col gap-3">
          {misses.data.map((m) => (
            <li key={m.q} className="border p-3 text-[13px]">
              <p className="font-medium">{m.q}</p>
              <p className="mt-1">Answer: {m.answer}</p>
              <PromptMarkdown source={m.why} className="mt-1" />
            </li>
          ))}
        </ul>
      ) : null}
    </section>
  )
}
