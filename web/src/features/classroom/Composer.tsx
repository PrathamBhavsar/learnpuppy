import { PaperclipIcon, SendIcon, SquareIcon } from "lucide-react"
import { useEffect, useRef, useState } from "react"

import { InfoTip } from "@/components/app/info/info"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { Toggle } from "@/components/ui/toggle"
import { useHotkey } from "@/hooks/use-hotkey"

import { COPY } from "./copy"
import { classroomInfo } from "./info"
import { useDraft } from "./dock/drafts"

type ComposerProps = {
  running: boolean
  sending: boolean
  stopping: boolean
  attaching: boolean
  onSend: (text: string) => void | Promise<void>
  onSteer: (text: string) => void | Promise<void>
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
  const { text, setText, submit: submitDraft } = useDraft()
  const [steer, setSteer] = useState(false)
  const [attachmentError, setAttachmentError] = useState<string | null>(null)
  const fileRef = useRef<HTMLInputElement>(null)
  const noteRef = useRef<HTMLInputElement>(null)
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
    void submitDraft(() => (running ? onSteer(value) : onSend(value)))
  }

  useHotkey("mod+enter", submit, {
    id: "composer-submit",
    label: "Send message",
    group: "Learn",
    enabled: canSend,
  })

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
              ? "Steer the lesson, e.g. “go slower on verb tenses”"
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
              setAttachmentError(null)
              if (file && file.size > 40 * 1024 * 1024) {
                setAttachmentError(
                  "This PDF is over 40 MB. Split it into smaller page ranges or compress scanned images, then attach it again."
                )
              } else if (file) onAttach(file)
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
          <input
            ref={noteRef}
            type="file"
            accept=".md,.txt,text/plain,text/markdown"
            hidden
            data-testid="composer-note-file"
            aria-label="Note file"
            onChange={(event) => {
              const file = event.target.files?.[0]
              event.target.value = ""
              setAttachmentError(null)
              if (!file) return
              if (!/\.(md|txt)$/i.test(file.name)) {
                setAttachmentError("Choose a .md or .txt note.")
              } else if (file.size > 1024 * 1024) {
                setAttachmentError(
                  "This note is over 1 MB. Split it into smaller notes, then import it again."
                )
              } else onAttach(file)
            }}
          />
          <Button
            variant="ghost"
            size="sm"
            disabled={attaching || running}
            onClick={() => noteRef.current?.click()}
            aria-label="Import note"
            data-testid="composer-note"
          >
            Note
          </Button>
          <InfoTip info={classroomInfo.pdf} />
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
          {running ? <InfoTip info={classroomInfo.steer} /> : null}
          {running ? <InfoTip info={classroomInfo.stop} /> : null}
          {!running ? <InfoTip info={classroomInfo.composer} /> : null}
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
      {attachmentError ? (
        <p role="alert" className="text-[13px] text-destructive">
          {attachmentError}
        </p>
      ) : null}
    </div>
  )
}
