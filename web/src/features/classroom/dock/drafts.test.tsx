import { fireEvent, screen, waitFor } from "@testing-library/react"
import { describe, expect, it, vi } from "vitest"

import { renderWithProviders } from "@/test/render"

import { Composer } from "../Composer"
import { DraftScope } from "./drafts"
import { QuestionDock } from "./QuestionDock"
import type { PendingQuestion } from "@/api/pending-types"

describe("scoped drafts", () => {
  it("restores after unmount, isolates workspaces/chats, and retains a rejected send", async () => {
    const onSend = vi.fn().mockRejectedValue(new Error("offline"))
    const ui = (ws: string, chat: string) => (
      <DraftScope ws={ws} chatId={chat}>
        <Composer
          running={false}
          sending={false}
          stopping={false}
          attaching={false}
          onSend={onSend}
          onSteer={vi.fn()}
          onStop={vi.fn()}
          onAttach={vi.fn()}
        />
      </DraftScope>
    )
    const view = renderWithProviders(ui("one", "chat"))
    fireEvent.change(screen.getByLabelText("Message"), {
      target: { value: "unfinished" },
    })
    view.unmount()
    const other = renderWithProviders(ui("two", "chat"))
    expect(screen.getByLabelText("Message")).toHaveValue("")
    other.unmount()
    const chat = renderWithProviders(ui("one", "other"))
    expect(screen.getByLabelText("Message")).toHaveValue("")
    chat.unmount()
    renderWithProviders(ui("one", "chat"))
    expect(screen.getByLabelText("Message")).toHaveValue("unfinished")
    fireEvent.click(screen.getByTestId("composer-send"))
    await waitFor(() => expect(onSend).toHaveBeenCalledOnce())
    expect(screen.getByLabelText("Message")).toHaveValue("unfinished")
    onSend.mockResolvedValue(undefined)
    fireEvent.click(screen.getByTestId("composer-send"))
    await waitFor(() =>
      expect(screen.getByLabelText("Message")).toHaveValue("")
    )
  })

  it("restores each question separately and keeps problem Enter as a newline", async () => {
    const onSubmit = vi.fn()
    const ui = (callId: string) => (
      <DraftScope ws="one" chatId="chat">
        <QuestionDock
          pending={
            {
              callId,
              tool: "problem",
              prompt: "Solve",
              allowFreeText: true,
              multiSelect: false,
            } satisfies PendingQuestion
          }
          probe={{ asked: 0, max: 8 }}
          busy={false}
          onSubmit={onSubmit}
        />
      </DraftScope>
    )
    let view = renderWithProviders(ui("one"))
    fireEvent.change(screen.getByLabelText("Your answer"), {
      target: { value: "fn answer()" },
    })
    fireEvent.keyDown(screen.getByLabelText("Your answer"), { key: "Enter" })
    expect(onSubmit).not.toHaveBeenCalled()
    view.unmount()
    view = renderWithProviders(ui("two"))
    expect(screen.getByLabelText("Your answer")).toHaveValue("")
    view.unmount()
    renderWithProviders(ui("one"))
    expect(screen.getByLabelText("Your answer")).toHaveValue("fn answer()")
    fireEvent.keyDown(screen.getByLabelText("Your answer"), {
      key: "Enter",
      ctrlKey: true,
    })
    await waitFor(() =>
      expect(onSubmit).toHaveBeenCalledWith({ text: "fn answer()" })
    )
    await waitFor(() =>
      expect(screen.getByLabelText("Your answer")).toHaveValue("")
    )
  })

  it("does not submit while an IME composes", () => {
    const onSubmit = vi.fn()
    renderWithProviders(
      <QuestionDock
        pending={{
          callId: "ime",
          tool: "probe",
          prompt: "Explain",
          allowFreeText: true,
          multiSelect: false,
        }}
        probe={{ asked: 1, max: 8 }}
        busy={false}
        onSubmit={onSubmit}
      />
    )
    const input = screen.getByLabelText("Your answer")
    fireEvent.change(input, { target: { value: "まだ" } })
    fireEvent.keyDown(input, { key: "Enter", isComposing: true })
    expect(onSubmit).not.toHaveBeenCalled()
  })

  it("rejects a PDF over 40 MB before upload", () => {
    const onAttach = vi.fn()
    renderWithProviders(
      <Composer
        running={false}
        sending={false}
        stopping={false}
        attaching={false}
        onSend={vi.fn()}
        onSteer={vi.fn()}
        onStop={vi.fn()}
        onAttach={onAttach}
      />
    )
    const file = new File(["%PDF-1.4"], "scanned.pdf", {
      type: "application/pdf",
    })
    Object.defineProperty(file, "size", { value: 40 * 1024 * 1024 + 1 })
    fireEvent.change(screen.getByTestId("composer-file"), {
      target: { files: [file] },
    })
    expect(onAttach).not.toHaveBeenCalled()
    expect(screen.getByRole("alert")).toHaveTextContent("over 40 MB")
  })
})
