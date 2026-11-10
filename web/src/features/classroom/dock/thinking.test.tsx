import { act, fireEvent, screen, waitFor } from "@testing-library/react"
import userEvent from "@testing-library/user-event"
import { afterEach, describe, expect, it, vi } from "vitest"

import type { PendingQuestion } from "@/api/pending-types"
import { renderWithProviders } from "@/test/render"

import { TooltipProvider } from "@/components/ui/tooltip"
import { DraftScope } from "./drafts"
import { QuestionDock } from "./QuestionDock"
import type { Recognition } from "./use-dictation"

const QUIZ: PendingQuestion = {
  callId: "c1",
  tool: "quiz",
  prompt: "Pick",
  allowFreeText: false,
  multiSelect: false,
  options: [
    { key: "a", text: "A" },
    { key: "b", text: "B" },
  ],
}
const RECALL: PendingQuestion = { ...QUIZ, tool: "recall", options: undefined }

let instances: (Recognition & { started: number; stopped: number })[] = []
function mockSpeech() {
  class Fake {
    continuous = false
    interimResults = false
    lang = ""
    onresult: Recognition["onresult"] = null
    onend: Recognition["onend"] = null
    onerror: Recognition["onerror"] = null
    started = 0
    stopped = 0
    constructor() {
      instances.push(this)
    }
    start() {
      this.started++
    }
    stop() {
      this.stopped++
    }
  }
  vi.stubGlobal("webkitSpeechRecognition", Fake)
}

function setup(q: PendingQuestion, busy = false) {
  const onSubmit = vi.fn()
  const ui = (b: boolean) => (
    <TooltipProvider>
      <DraftScope ws="w" chatId="chat">
        <QuestionDock
          pending={q}
          probe={{ asked: 1, max: 8 }}
          busy={b}
          onSubmit={onSubmit}
        />
      </DraftScope>
    </TooltipProvider>
  )
  const view = renderWithProviders(ui(busy))
  return { onSubmit, view, ui }
}

afterEach(() => {
  vi.unstubAllGlobals()
  localStorage.clear()
  instances = []
})

describe("think-out-loud field", () => {
  it("is optional and sent as readingNotes", async () => {
    const user = userEvent.setup()
    const { onSubmit } = setup(QUIZ)
    await user.click(screen.getByTestId("quiz-option-A"))
    await user.click(screen.getByTestId("quiz-submit"))
    expect(onSubmit).toHaveBeenLastCalledWith(
      expect.objectContaining({ choice: ["a"], readingNotes: undefined })
    )
  })

  it("saves notes with the answer and persists the draft", async () => {
    const user = userEvent.setup()
    const { view } = setup(RECALL)
    fireEvent.change(screen.getByLabelText("Explain your thinking"), {
      target: { value: "because" },
    })
    view.unmount()
    const { onSubmit } = setup(RECALL)
    expect(screen.getByLabelText("Explain your thinking")).toHaveValue(
      "because"
    )
    await user.type(screen.getByLabelText("Your answer"), "x")
    await user.click(screen.getByTestId("recall-submit"))
    expect(onSubmit).toHaveBeenCalledWith({ text: "x", readingNotes: "because" })
    await waitFor(() =>
      expect(screen.getByLabelText("Explain your thinking")).toHaveValue("")
    )
  })

  it("hides the mic when SpeechRecognition is missing", () => {
    setup(RECALL)
    expect(screen.queryByTestId("think-mic")).toBeNull()
  })

  it("shows the notice first, then dictates, appends and stops on submit", async () => {
    mockSpeech()
    const user = userEvent.setup()
    const fetchSpy = vi.fn()
    vi.stubGlobal("fetch", fetchSpy)
    const { onSubmit, view, ui } = setup(RECALL)
    await user.click(screen.getByTestId("think-mic"))
    expect(screen.getByTestId("think-notice")).toBeInTheDocument()
    expect(instances).toHaveLength(0)
    await user.click(screen.getByTestId("think-enable"))
    expect(instances[0]!.started).toBe(1)
    act(() =>
      instances[0]!.onresult?.({
        resultIndex: 0,
        results: [{ isFinal: true, 0: { transcript: " hello " } }],
      })
    )
    expect(screen.getByLabelText("Explain your thinking")).toHaveValue("hello")
    expect(onSubmit).not.toHaveBeenCalled()
    view.rerender(ui(true))
    expect(instances[0]!.stopped).toBe(1)
    expect(fetchSpy).not.toHaveBeenCalled()
  })

  it("stops on blur", async () => {
    mockSpeech()
    localStorage.setItem("lp-dictation-on", "1")
    const user = userEvent.setup()
    setup(RECALL)
    await user.click(screen.getByTestId("think-mic"))
    expect(instances[0]!.started).toBe(1)
    await user.click(screen.getByLabelText("Your answer"))
    expect(instances[0]!.stopped).toBe(1)
  })
})
