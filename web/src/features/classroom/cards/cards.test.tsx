import { render, screen } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { FlaskConicalIcon } from "lucide-react"
import { describe, expect, it, vi } from "vitest"

import type { ClassroomJob } from "@/api/pending-types"
import { renderApp, resetUi } from "@/test/render"

import { JobCard } from "./JobCard"
import { ProbeDoneCard } from "./ProbeDoneCard"
import { QuizAskedCard } from "./QuizAskedCard"
import { SteerCard } from "./SteerCard"
import { VisualReadyCard } from "./VisualReadyCard"

const job = (over: Partial<ClassroomJob> = {}): ClassroomJob => ({
  id: "j1",
  kind: "research",
  status: "running",
  title: "Wakers deep dive",
  ...over,
})

const jobCard = (j: ClassroomJob, onAbort = vi.fn(), onOpen = vi.fn()) =>
  render(
    <JobCard
      job={j}
      kindLabel="Research"
      icon={FlaskConicalIcon}
      onAbort={onAbort}
      onOpen={onOpen}
    />
  )

describe("event cards", () => {
  it("quiz asked: one line summary, focusable, no options", () => {
    render(
      <QuizAskedCard
        question={{ tool: "quiz", prompt: "What does\n poll do?" }}
      />
    )
    const card = screen.getByTestId("card-quiz")
    expect(card).toHaveAttribute("tabindex", "0")
    expect(card).toHaveTextContent("What does poll do?")
    expect(card).toHaveTextContent("Quiz asked")
  })

  it("job: running shows abort, done shows open result", async () => {
    const onAbort = vi.fn()
    const { rerender } = jobCard(job(), onAbort)
    const card = screen.getByTestId("job-j1")
    expect(card).toHaveAttribute("data-status", "running")
    expect(card).toHaveTextContent("Running")
    await userEvent.click(screen.getByTestId("job-abort-j1"))
    expect(onAbort).toHaveBeenCalled()
    rerender(
      <JobCard
        job={job({ status: "done", artifactId: "a1" })}
        kindLabel="Research"
        icon={FlaskConicalIcon}
        onAbort={onAbort}
        onOpen={vi.fn()}
      />
    )
    expect(screen.getByTestId("job-j1")).toHaveAttribute("data-status", "done")
    expect(screen.getByTestId("job-open-j1")).toBeInTheDocument()
    expect(screen.queryByTestId("job-abort-j1")).toBeNull()
  })

  it("job: failed shows the error", () => {
    render(
      <JobCard
        job={job({ status: "failed" })}
        kindLabel="Research"
        icon={FlaskConicalIcon}
        error="boom"
        onAbort={vi.fn()}
        onOpen={vi.fn()}
      />
    )
    expect(screen.getByTestId("job-error-j1")).toHaveTextContent("boom")
  })

  it("visual ready opens the artifact", async () => {
    const onOpen = vi.fn()
    const j = job({ kind: "visual", status: "done", artifactId: "a1" })
    render(<VisualReadyCard job={j} onOpen={onOpen} />)
    expect(screen.getByTestId("job-j1")).toHaveTextContent("Diagram ready")
    await userEvent.click(screen.getByTestId("job-open-j1"))
    expect(onOpen).toHaveBeenCalledWith(j)
  })

  it("steer card shows the text", () => {
    render(<SteerCard text="focus on lifetimes" />)
    expect(screen.getByTestId("card-steer")).toHaveTextContent(
      "focus on lifetimes"
    )
  })

  it("probe done summarises progress", () => {
    render(<ProbeDoneCard asked={3} max={3} />)
    expect(screen.getByTestId("card-probe")).toHaveTextContent("3 of 3")
  })

  it("leak: an unanswered quiz renders no option text in the cards", async () => {
    resetUi()
    renderApp("/w/ws_rustasync/learn/c_teach_quiz")
    const card = await screen.findByTestId("card-quiz")
    const dock = await screen.findByTestId("question-dock")
    // Correct option text, as shown in the dock, must not be inside the card.
    const options = dock.querySelectorAll("[data-testid^='quiz-option-']")
    expect(options.length).toBeGreaterThan(0)
    for (const o of options) {
      const text = (o.textContent ?? "").trim()
      if (text) expect(card.textContent).not.toContain(text)
    }
  })
})
