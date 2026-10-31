// Delete flow: preview the blast radius, require an explicit acknowledgement when chains would be
// orphaned, then shrink the nodes out before the refetch removes them (docs/web/06, 05 #22).
import { useEffect, useRef, useState } from "react"

import { toApiError } from "@/api/errors"
import { useDeleteNodes, useDeletePreview } from "@/api/queries/graph"
import {
  AlertDialog,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { Button } from "@/components/ui/button"
import { Checkbox } from "@/components/ui/checkbox"
import { Spinner } from "@/components/ui/spinner"
import { toastError, toastSuccess } from "@/lib/toast"

import { copy } from "../copy"

const LIST_LIMIT = 8

type Props = {
  ws: string
  /** Node ids to delete; null keeps the dialog closed. */
  nodeIds: readonly string[] | null
  /** Plays the shrink-out animation for these nodes; resolves when it has finished. */
  onAnimateOut: (nodeIds: readonly string[]) => Promise<void>
  onDeleted: () => void
  onClose: () => void
}

export function DeleteDialog({ ws, nodeIds, onAnimateOut, onDeleted, onClose }: Props) {
  const preview = useDeletePreview(ws)
  const remove = useDeleteNodes(ws)
  const [understood, setUnderstood] = useState(false)
  const [stale, setStale] = useState(false)
  const [busy, setBusy] = useState(false)
  const key = nodeIds?.join(",") ?? ""
  const requested = useRef("")

  // Run (or re-run) the preview whenever the dialog opens for a new set of nodes.
  const { mutate: runPreview, reset: resetPreview } = preview
  useEffect(() => {
    if (!nodeIds || nodeIds.length === 0) {
      requested.current = ""
      resetPreview()
      return
    }
    if (requested.current === key) return
    requested.current = key
    setUnderstood(false)
    setStale(false)
    runPreview([...nodeIds])
  }, [key, nodeIds, runPreview, resetPreview])

  const data = preview.data
  const orphaned = data?.orphanedChains ?? 0
  const needsAck = orphaned > 0
  const ready = Boolean(data) && (!needsAck || understood) && !busy

  const confirm = async () => {
    if (!nodeIds || !data) return
    setBusy(true)
    try {
      await onAnimateOut(nodeIds)
      await remove.mutateAsync({ nodeIds: [...nodeIds], previewToken: data.previewToken })
      toastSuccess(copy.del.done(nodeIds.length))
      onDeleted()
    } catch (error) {
      const api = toApiError(error)
      if (api.code === "conflict") {
        setStale(true)
        setUnderstood(false)
        runPreview([...nodeIds])
      } else toastError(error, copy.del.failed)
    } finally {
      setBusy(false)
    }
  }

  return (
    <AlertDialog open={nodeIds !== null && nodeIds.length > 0} onOpenChange={(open) => !open && !busy && onClose()}>
      <AlertDialogContent data-testid="graph-delete-dialog" className="data-[size=default]:sm:max-w-md">
        <AlertDialogHeader>
          <AlertDialogTitle>{copy.del.title(nodeIds?.length ?? 0)}</AlertDialogTitle>
          <AlertDialogDescription>{copy.del.description}</AlertDialogDescription>
        </AlertDialogHeader>

        <div aria-live="polite" className="flex min-h-16 flex-col gap-3 text-[12px]">
          {preview.isPending && !data ? (
            <p className="flex items-center gap-2 text-muted-foreground">
              <Spinner /> {copy.del.loading}
            </p>
          ) : preview.isError ? (
            <p role="alert" className="text-destructive">
              {copy.del.previewFailed}: {toApiError(preview.error).message}
            </p>
          ) : data ? (
            <>
              {stale ? <p role="status" className="text-warning">{copy.del.stale}</p> : null}
              <p data-testid="graph-delete-summary" className={needsAck ? "font-medium text-destructive" : "text-muted-foreground"}>
                {needsAck ? copy.del.orphaned(orphaned) : copy.del.none}
              </p>
              {data.blastRadius.length > 0 ? (
                <ul data-testid="graph-delete-blast" className="flex max-h-44 flex-col gap-1 overflow-y-auto border border-border p-2">
                  {data.blastRadius.slice(0, LIST_LIMIT).map((item) => (
                    <li key={item.id} className="flex flex-col">
                      <span dir="auto" className="truncate">{item.text}</span>
                      <span className="text-[11px] text-muted-foreground">{item.reason}</span>
                    </li>
                  ))}
                  {data.blastRadius.length > LIST_LIMIT ? (
                    <li className="text-muted-foreground">{copy.del.more(data.blastRadius.length - LIST_LIMIT)}</li>
                  ) : null}
                </ul>
              ) : null}
              {needsAck ? (
                <label className="flex items-center gap-2">
                  <Checkbox
                    checked={understood}
                    onCheckedChange={(v) => setUnderstood(v === true)}
                    data-testid="graph-delete-ack"
                  />
                  {copy.del.understand}
                </label>
              ) : null}
            </>
          ) : null}
        </div>

        <AlertDialogFooter>
          <AlertDialogCancel disabled={busy} data-testid="graph-delete-cancel">{copy.del.cancel}</AlertDialogCancel>
          <Button
            variant="destructive"
            disabled={!ready}
            onClick={() => void confirm()}
            data-testid="graph-delete-confirm"
            className="transition-colors duration-[var(--dur-fast)]"
          >
            {busy ? <Spinner data-icon="inline-start" /> : null}
            {busy ? copy.del.working : copy.del.confirm}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
