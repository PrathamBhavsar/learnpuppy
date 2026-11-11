import { UploadIcon } from "lucide-react"
import { useEffect, useRef, useState } from "react"

import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"

import { copy } from "./copy"

const MAX_BYTES = 50 * 1024 * 1024

/** Choose or drop one PDF. Size and type are checked here only to save a round trip; the server decides. */
export function UploadCard({
  pending,
  focus,
  onFile,
}: {
  pending: boolean
  focus?: boolean
  onFile: (file: File) => void
}) {
  const input = useRef<HTMLInputElement>(null)
  const button = useRef<HTMLButtonElement>(null)
  const [problem, setProblem] = useState<string | null>(null)
  const [over, setOver] = useState(false)

  useEffect(() => {
    if (!focus) return
    // The palette hands focus back to the page when it closes; wait for that, then take it.
    const t = window.setTimeout(() => button.current?.focus(), 300)
    return () => window.clearTimeout(t)
  }, [focus])

  const take = (file: File | undefined) => {
    if (!file) return
    if (!file.name.toLowerCase().endsWith(".pdf"))
      return setProblem(copy.notPdf)
    if (file.size > MAX_BYTES) return setProblem(copy.tooBig)
    setProblem(null)
    onFile(file)
  }

  return (
    <div
      data-testid="textbook-upload"
      data-over={over}
      onDragOver={(e) => {
        e.preventDefault()
        setOver(true)
      }}
      onDragLeave={() => setOver(false)}
      onDrop={(e) => {
        e.preventDefault()
        setOver(false)
        take(e.dataTransfer.files[0])
      }}
      className="flex flex-col items-start gap-2 border border-dashed p-4 data-[over=true]:border-primary data-[over=true]:bg-primary/5"
    >
      <input
        ref={input}
        type="file"
        accept="application/pdf,.pdf"
        hidden
        data-testid="textbook-file"
        aria-label="PDF file"
        onChange={(e) => {
          take(e.target.files?.[0])
          e.target.value = ""
        }}
      />
      <Button
        ref={button}
        data-testid="textbook-upload-button"
        disabled={pending}
        onClick={() => input.current?.click()}
      >
        {pending ? <Spinner /> : <UploadIcon />}
        {pending ? copy.uploading : copy.upload}
      </Button>
      <p
        id="textbook-upload-hint"
        className="text-[13px] text-muted-foreground"
      >
        {copy.uploadHint}
      </p>
      {problem ? (
        <p
          role="alert"
          data-testid="textbook-upload-problem"
          className="text-[13px] text-destructive"
        >
          {problem}
        </p>
      ) : null}
    </div>
  )
}
