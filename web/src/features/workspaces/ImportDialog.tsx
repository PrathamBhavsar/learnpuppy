import { UploadIcon } from "lucide-react"
import { useRef, useState } from "react"

import { toApiError } from "@/api/errors"
import { useImportWorkspace } from "@/api/queries/workspaces"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Spinner } from "@/components/ui/spinner"
import { toastError, toastSuccess } from "@/lib/toast"
import { cn } from "@/lib/utils"

import { formatBytes } from "./model"

const MAX_IMPORT_BYTES = 512 * 1024 * 1024

/** Client-side checks that save a round trip; the server re-validates manifest, checksums and schema. */
export function validateImportFile(file: File): string | null {
  if (!file.name.toLowerCase().endsWith(".zip"))
    return "Choose the .zip file produced by Export."
  if (file.size === 0) return "That file is empty."
  if (file.size > MAX_IMPORT_BYTES)
    return `That file is ${formatBytes(file.size)}; the limit is ${formatBytes(MAX_IMPORT_BYTES)}.`
  return null
}

type Props = {
  open: boolean
  onOpenChange: (open: boolean) => void
  onImported?: (workspaceId: string) => void
}

function ImportBody({ onOpenChange, onImported }: Omit<Props, "open">) {
  const importer = useImportWorkspace()
  const input = useRef<HTMLInputElement>(null)
  const [file, setFile] = useState<File | null>(null)
  const [error, setError] = useState<string | null>(null)
  const [dragging, setDragging] = useState(false)

  const choose = (next: File | undefined) => {
    if (!next) return
    const problem = validateImportFile(next)
    setError(problem)
    setFile(problem ? null : next)
  }

  const submit = async () => {
    if (!file || importer.isPending) return
    setError(null)
    try {
      const { workspaceId } = await importer.mutateAsync(file)
      toastSuccess("Workspace imported")
      onOpenChange(false)
      onImported?.(workspaceId)
    } catch (caught) {
      const api = toApiError(caught)
      const field = api.fields.find((f) => f.path === "file")
      if (field) setError(field.message)
      else toastError(api, "Import failed")
    }
  }

  return (
    <DialogContent data-testid="import-dialog">
      <DialogHeader>
        <DialogTitle>Import workspace</DialogTitle>
        <DialogDescription>
          Drop a LearnPuppy export. It becomes a new workspace and never
          overwrites an existing one.
        </DialogDescription>
      </DialogHeader>
      <div
        data-testid="import-dropzone"
        data-dragging={dragging || undefined}
        onDragOver={(event) => {
          event.preventDefault()
          setDragging(true)
        }}
        onDragLeave={() => setDragging(false)}
        onDrop={(event) => {
          event.preventDefault()
          setDragging(false)
          choose(event.dataTransfer.files[0])
        }}
        className={cn(
          "flex flex-col items-center gap-2 border border-dashed px-4 py-8 text-center transition-colors duration-[var(--dur-fast)]",
          dragging && "border-ring bg-muted",
          error && "border-destructive"
        )}
      >
        <UploadIcon aria-hidden className="size-6 text-muted-foreground" />
        <p className="text-[13px]">
          {file ? (
            <>
              <strong className="font-medium">{file.name}</strong>{" "}
              <span className="text-muted-foreground tabular-nums">
                {formatBytes(file.size)}
              </span>
            </>
          ) : (
            "Drop the .zip here or"
          )}
        </p>
        <Button
          variant="outline"
          size="sm"
          type="button"
          onClick={() => input.current?.click()}
          data-testid="import-choose"
        >
          Choose file
        </Button>
        <input
          ref={input}
          type="file"
          accept=".zip,application/zip"
          hidden
          data-testid="import-file-input"
          onChange={(event) => {
            choose(event.target.files?.[0])
            event.target.value = ""
          }}
        />
      </div>
      {error ? (
        <p
          role="alert"
          data-testid="import-error"
          className="text-[12px] text-destructive"
        >
          {error}
        </p>
      ) : null}
      <DialogFooter>
        <Button
          variant="ghost"
          onClick={() => onOpenChange(false)}
          data-testid="import-cancel"
        >
          Cancel
        </Button>
        <Button
          disabled={!file || importer.isPending}
          onClick={() => void submit()}
          data-testid="import-submit"
        >
          {importer.isPending ? <Spinner data-icon="inline-start" /> : null}
          Import
        </Button>
      </DialogFooter>
    </DialogContent>
  )
}

export function ImportDialog({ open, onOpenChange, onImported }: Props) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      {open ? (
        <ImportBody onOpenChange={onOpenChange} onImported={onImported} />
      ) : null}
    </Dialog>
  )
}
