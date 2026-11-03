import { useState } from "react"

import { toApiError } from "@/api/errors"
import type { components } from "@/api/schema"
import { ErrorState } from "@/components/app/error-state"
import { PageHeader } from "@/components/app/page-header"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { toastError } from "@/lib/toast"

import { useCourses, useCreateCourse } from "./api"
import { CourseView } from "./CourseView"

type Placement = components["schemas"]["CreatedCourseDto"]

type Props = {
  ws: string
  /** The open course, kept in the URL. */
  id: string | undefined
  onOpen: (id: string | undefined) => void
}

/** Course list with create-from-goal; opening a course hands over to the session. */
export function CourseScreen({ ws, id, onOpen }: Props) {
  const courses = useCourses(ws)
  const create = useCreateCourse(ws)
  const [goal, setGoal] = useState("")
  // Placement questions only come back from create, so they live here, not in the cache.
  const [placement, setPlacement] = useState<Placement | null>(null)

  if (id)
    return (
      <CourseView
        ws={ws}
        id={id}
        placement={placement?.id === id ? placement.items : undefined}
        onPlaced={() => setPlacement(null)}
        onBack={() => onOpen(undefined)}
      />
    )

  const start = () => {
    const value = goal.trim()
    if (value === "" || create.isPending) return
    create.mutate(value, {
      onSuccess: (created) => {
        setGoal("")
        setPlacement(created)
        onOpen(created.id)
      },
      onError: (e) => toastError(e, "Could not start the course"),
    })
  }

  return (
    <div
      data-testid="screen-course"
      className="mx-auto flex max-w-2xl flex-col gap-6 p-6"
    >
      <PageHeader
        title="Courses"
        description="Say what you want to learn. A short quiz finds where you are, then the lessons are written for you."
      />
      <div className="flex flex-col gap-2">
        <Textarea
          value={goal}
          onChange={(e) => setGoal(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && !e.shiftKey) {
              e.preventDefault()
              start()
            }
          }}
          rows={2}
          maxLength={500}
          disabled={create.isPending}
          aria-label="Course goal"
          placeholder="e.g. Read Rust async code with confidence"
        />
        <Button
          onClick={start}
          disabled={goal.trim() === "" || create.isPending}
          className="self-start"
        >
          {create.isPending ? <Spinner /> : null}
          {create.isPending ? "Writing your quiz" : "Start course"}
        </Button>
      </div>
      {courses.isError ? (
        <ErrorState
          message={toApiError(courses.error).message}
          onRetry={() => courses.refetch()}
        />
      ) : (
        <ul data-testid="course-list" className="flex flex-col gap-1">
          {courses.data?.map((c) => (
            <li key={c.id}>
              <button
                type="button"
                onClick={() => onOpen(c.id)}
                className="w-full border px-3 py-2 text-start text-[13px] hover:bg-muted/50"
              >
                {c.title}
              </button>
            </li>
          ))}
          {courses.data?.length === 0 ? (
            <li className="text-[13px] text-muted-foreground">
              No courses yet.
            </li>
          ) : null}
        </ul>
      )}
    </div>
  )
}
