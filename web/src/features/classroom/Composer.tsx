import { PaperclipIcon, SendIcon, SquareIcon } from "lucide-react"
import { useEffect, useRef, useState } from "react"

import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { Toggle } from "@/components/ui/toggle"

import { COPY } from "./copy"

type ComposerProps = {
  running: boolean
  sending: boolean
  stopping: boolean
  attaching: boolean
  onSend: (text: string) => void
  onSteer: (text: string) => void
  onStop: () => void
  onAttach: (file: File) => void
}

/**
 * Message box. Enter or Cmd/Ctrl+Enter sends, Shift+Enter breaks the line. While the tutor replies
 * it only accepts guidance (Steer); Stop (or Esc) cancels the turn.
 */
export function Composer({
  running,
  sending,
  stopping,
  attaching,
  onSend,
  onSteer,
  onStop,
  onAttach,
}: ComposerProps) {
  const [text, setText] = useState("")
  const [steer, setSteer] = useState(false)
  const fileRef = useRef<HTMLInputElement>(null)
  const editable = !running || steer
  const canSend = editable && text.trim() !== "" && !sending

  // The shell binds Esc globally (blur), and its handler wins the shortcut registry, so Stop listens
  // first, in the capture phase, and skips while an overlay (dialog, menu, popover) owns Esc.
  const canStop = running && !stopping
  useEffect(() => {
    if (!canStop) return
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return
      if (
        document.querySelector(
          "[role=dialog][data-state=open], [role=menu], [data-radix-popper-content-wrapper]"
        )
      )
        return
      event.preventDefault()
      onStop()
    }
    window.addEventListener("keydown", onKeyDown, { capture: true })
    return () =>
      window.removeEventListener("keydown", onKeyDown, { capture: true })
  }, [canStop, onStop])

  const submit = () => {
    if (!canSend) return
    const value = text.trim()
    setText("")
    if (running) onSteer(value)
    else onSend(value)
  }

  return (
    <div
      className="mx-auto flex w-full max-w-3xl flex-col gap-2 border-t bg-background px-4 py-3"
      data-testid="composer"
    >
      <Textarea
        value={text}
        onChange={(event) => setText(event.target.value)}
        onKeyDown={(event) => {
          if (
            event.key === "Enter" &&
            !event.shiftKey &&
            !event.nativeEvent.isComposing
          ) {
            event.preventDefault()
            submit()
          }
        }}
        disabled={!editable}
        dir="auto"
        rows={1}
        aria-label="Message"
        placeholder={
          running
            ? steer
              ? "Steer the lesson, e.g. “go slower on pinning”"
              : "The tutor is replying…"
            : "Ask a question or reply"
        }
        data-testid="composer-input"
        className="max-h-[calc(8lh+1rem)] min-h-9 resize-none"
      />
      <div className="flex items-center justify-between gap-2">
        <div className="flex items-center gap-1">
          <input
            ref={fileRef}
            type="file"
            accept="application/pdf,.pdf"
            hidden
            data-testid="composer-file"
            aria-label="PDF file"
            onChange={(event) => {
              const file = event.target.files?.[0]
              event.target.value = ""
              if (file) onAttach(file)
            }}
          />
          <Button
            variant="ghost"
            size="sm"
            disabled={attaching}
            onClick={() => fileRef.current?.click()}
            aria-label="Attach PDF"
            data-testid="composer-attach"
          >
            {attaching ? (
              <Spinner data-icon="inline-start" />
            ) : (
              <PaperclipIcon data-icon="inline-start" />
            )}
            PDF
          </Button>
          {running ? (
            <Toggle
              size="sm"
              variant="outline"
              pressed={steer}
              onPressedChange={setSteer}
              data-testid="composer-steer"
            >
              {COPY.steerOn}
            </Toggle>
          ) : null}
        </div>
        <div className="flex items-center gap-2">
          {running ? (
            <Button
              variant="destructive"
              onClick={onStop}
              disabled={stopping}
              data-testid="composer-stop"
            >
              {stopping ? (
                <Spinner data-icon="inline-start" />
              ) : (
                <SquareIcon data-icon="inline-start" />
              )}
              {COPY.stop}
            </Button>
          ) : null}
          {!running || steer ? (
            <Button
              onClick={submit}
              disabled={!canSend}
              data-testid="composer-send"
            >
              {sending ? (
                <Spinner data-icon="inline-start" />
              ) : (
                <SendIcon
                  data-icon="inline-start"
                  className="rtl:-scale-x-100"
                />
              )}
              {running ? "Send steer" : COPY.send}
            </Button>
          ) : null}
        </div>
      </div>
    </div>
  )
}
