import { screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { describe, expect, it, vi } from "vitest"

import type { PendingQuestion } from "@/api/pending-types"
import { renderWithProviders } from "@/test/render"

import { QuestionDock } from "./dock/QuestionDock"

const PROBE = { asked: 3, max: 8 }

function pending(overrides: Partial<PendingQuestion>): PendingQuestion {
  return {
    callId: "call_1",
    tool: "quiz",
    prompt: "What does poll return?",
    allowFreeText: false,
    multiSelect: false,
    ...overrides,
  }
}

const QUIZ = pending({
  options: [
    { key: "pending", text: "Poll::Pending" },
    { key: "ready", text: "Poll::Ready(None)" },
    { key: "blocks", text: "It blocks" },
    { key: "panics", text: "It panics" },
  ],
  confidenceScale: 5,
  idkKey: "__idk__",
})

function setup(question: PendingQuestion, busy = false) {
  const onSubmit = vi.fn()
  renderWithProviders(
    <QuestionDock
      pending={question}
      probe={PROBE}
      busy={busy}
      onSubmit={onSubmit}
    />
  )
  return { onSubmit, user: userEvent.setup() }
}

describe("QuestionDock: quiz", () => {
  it("submits the picked option key with the confidence", async () => {
    const { onSubmit, user } = setup(QUIZ)
    expect(screen.getByTestId("quiz-submit")).toBeDisabled()
    await user.click(screen.getByTestId("quiz-option-B"))
    expect(screen.getByTestId("quiz-option-B")).toHaveAttribute(
      "aria-checked",
      "true"
    )
    await user.click(screen.getByTestId("quiz-submit"))
    expect(onSubmit).toHaveBeenCalledWith({
      choice: ["ready"],
      text: undefined,
      confidence: 3,
    })
  })

  it("picks with a-d and 1-4, and submits with Enter", async () => {
    const { onSubmit, user } = setup(QUIZ)
    await user.keyboard("c")
    expect(screen.getByTestId("quiz-option-C")).toHaveAttribute(
      "aria-checked",
      "true"
    )
    await user.keyboard("1")
    expect(screen.getByTestId("quiz-option-A")).toHaveAttribute(
      "aria-checked",
      "true"
    )
    expect(screen.getByTestId("quiz-option-C")).toHaveAttribute(
      "aria-checked",
      "false"
    )
    await user.keyboard("{Enter}")
    expect(onSubmit).toHaveBeenCalledTimes(1)
    expect(onSubmit.mock.calls[0]?.[0]).toMatchObject({ choice: ["pending"] })
  })

  it("moves the confidence slider with the keyboard", async () => {
    const { onSubmit, user } = setup(QUIZ)
    expect(screen.getByTestId("confidence-value")).toHaveTextContent("3 / 5")
    screen.getByRole("slider").focus()
    await user.keyboard("{ArrowRight}{ArrowRight}")
    expect(screen.getByTestId("confidence-value")).toHaveTextContent("5 / 5")
    await user.click(screen.getByTestId("quiz-option-A"))
    await user.click(screen.getByTestId("quiz-submit"))
    expect(onSubmit.mock.calls[0]?.[0]).toMatchObject({ confidence: 5 })
  })

  it("sends the question's idk key for I don't know", async () => {
    const { onSubmit, user } = setup(QUIZ)
    await user.click(screen.getByTestId("quiz-idk"))
    await user.click(screen.getByTestId("quiz-submit"))
    expect(onSubmit.mock.calls[0]?.[0]).toMatchObject({ choice: ["__idk__"] })
  })

  it("allows several picks only when the question is multi-select", async () => {
    const { onSubmit, user } = setup({ ...QUIZ, multiSelect: true })
    await user.click(screen.getByTestId("quiz-option-A"))
    await user.click(screen.getByTestId("quiz-option-C"))
    await user.click(screen.getByTestId("quiz-idk"))
    // I don't know is exclusive: it replaces the picks.
    expect(screen.getByTestId("quiz-option-A")).toHaveAttribute(
      "aria-checked",
      "false"
    )
    await user.click(screen.getByTestId("quiz-option-A"))
    await user.click(screen.getByTestId("quiz-option-C"))
    await user.click(screen.getByTestId("quiz-submit"))
    expect(onSubmit.mock.calls[0]?.[0]).toMatchObject({
      choice: ["pending", "blocks"],
    })
  })

  it("disables every control while an answer is in flight", () => {
    setup(QUIZ, true)
    for (const letter of ["A", "B", "C", "D"])
      expect(screen.getByTestId(`quiz-option-${letter}`)).toBeDisabled()
    expect(screen.getByTestId("quiz-submit")).toBeDisabled()
  })
})

describe("QuestionDock: free-text tools", () => {
  it("probe shows progress and submits free text with Enter (Shift+Enter breaks the line)", async () => {
    const { onSubmit, user } = setup(
      pending({
        tool: "probe",
        allowFreeText: true,
        prompt: "What is a future?",
      })
    )
    expect(screen.getByTestId("probe-count")).toHaveTextContent(
      "Question 3 of ≤8"
    )
    expect(screen.getAllByTestId("probe-segment-filled")).toHaveLength(2)
    await user.type(screen.getByTestId("probe-text"), "a lazy value")
    await user.keyboard("{Shift>}{Enter}{/Shift}more")
    expect(screen.getByTestId("probe-text")).toHaveValue("a lazy value\nmore")
    await user.keyboard("{Enter}")
    expect(onSubmit).toHaveBeenCalledWith({ text: "a lazy value\nmore" })
  })

  it("probe with options also accepts own words", async () => {
    const { onSubmit, user } = setup(
      pending({
        tool: "probe",
        allowFreeText: true,
        options: [
          { key: "a", text: "Eager" },
          { key: "b", text: "Lazy" },
        ],
        idkKey: "__idk__",
      })
    )
    await user.type(screen.getByTestId("probe-text"), "depends")
    await user.click(screen.getByTestId("probe-submit"))
    expect(onSubmit).toHaveBeenCalledWith({
      choice: [],
      text: "depends",
      confidence: undefined,
    })
    expect(screen.getByTestId("probe-text")).toHaveValue("")
    await user.type(screen.getByTestId("probe-text"), "another answer{Enter}")
    expect(onSubmit).toHaveBeenCalledTimes(2)
  })

  it("recall offers I don't know as a plain answer", async () => {
    const { onSubmit, user } = setup(
      pending({ tool: "recall", allowFreeText: true })
    )
    await user.click(screen.getByTestId("recall-idk"))
    expect(onSubmit).toHaveBeenCalledWith({ text: "I don't know" })
  })

  it("explain counts words and only needs some text to submit", async () => {
    const { onSubmit, user } = setup(
      pending({ tool: "explain", allowFreeText: true })
    )
    expect(screen.getByTestId("explain-submit")).toBeDisabled()
    await user.type(screen.getByTestId("explain-text"), "a future is lazy")
    expect(screen.getByTestId("explain-words")).toHaveTextContent(
      "4 / 12 words"
    )
    await user.click(screen.getByTestId("explain-submit"))
    expect(onSubmit).toHaveBeenCalledWith({ text: "a future is lazy" })
  })

  it("ask_user_question renders its options and submits the key", async () => {
    const { onSubmit, user } = setup(
      pending({
        tool: "ask_user_question",
        allowFreeText: true,
        options: [
          { key: "rust", text: "Rust" },
          { key: "go", text: "Go" },
        ],
      })
    )
    await user.click(screen.getByTestId("ask_user_question-option-B"))
    await user.click(screen.getByTestId("ask_user_question-submit"))
    expect(onSubmit.mock.calls[0]?.[0]).toMatchObject({ choice: ["go"] })
  })
})

describe("QuestionDock: problem", () => {
  const PROBLEM = pending({
    tool: "problem",
    allowFreeText: true,
    prompt: "Solve the problem below.",
    problem: {
      mode: "composition",
      statement: "Write `run` that drives a future.",
      rungs: ["Look at poll's receiver.", "Use pin!."],
    },
  })

  it("shows the statement and mode, and submits the attempt", async () => {
    const { onSubmit, user } = setup(PROBLEM)
    expect(screen.getByText("Composition")).toBeInTheDocument()
    await waitFor(() =>
      expect(screen.getByTestId("problem-statement")).toHaveTextContent(
        "Write run that drives a future."
      )
    )
    await user.type(screen.getByTestId("problem-text"), "fn run() {{}")
    await user.click(screen.getByTestId("problem-submit"))
    expect(onSubmit).toHaveBeenCalledWith({ text: "fn run() {}" })
  })

  it("reveals one rung per hint request and stops when the ladder ends", async () => {
    const { onSubmit, user } = setup(PROBLEM)
    await user.click(screen.getByTestId("problem-hint"))
    expect(onSubmit).toHaveBeenLastCalledWith({ hint: true, text: undefined })
    expect(screen.getByTestId("problem-hints")).toHaveTextContent(
      "Look at poll's receiver."
    )
    await user.click(screen.getByTestId("problem-hint"))
    expect(screen.getByTestId("problem-hints")).toHaveTextContent("Use pin!.")
    expect(screen.getByTestId("problem-hint")).toBeDisabled()
  })
})
