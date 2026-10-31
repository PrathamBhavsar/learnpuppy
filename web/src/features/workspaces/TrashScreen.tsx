import { RotateCcwIcon, Trash2Icon, XIcon } from "lucide-react"
import { useState } from "react"

import { toApiError } from "@/api/errors"
import type { TrashItem } from "@/api/pending-types"
import {
  usePurgeFromTrash,
  useRestoreFromTrash,
  useTrash,
} from "@/api/queries/workspaces"
import { EmptyState } from "@/components/app/empty-state"
import { ErrorState } from "@/components/app/error-state"
import { PageHeader } from "@/components/app/page-header"
import { SkeletonBlock } from "@/components/app/skeleton-block"
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
import { Spinner } from "@/components/ui/spinner"
import { usePageTitle } from "@/hooks/use-page-title"
import { toastError, toastSuccess } from "@/lib/toast"

import { formatBytes, relativeTime } from "./model"

function TrashRow({
  item,
  onPurge,
}: {
  item: TrashItem
  onPurge: (item: TrashItem) => void
}) {
  const restore = useRestoreFromTrash()
  return (
    <li
      data-testid={`trash-item-${item.id}`}
      className="flex items-center gap-3 border-b px-3 py-2.5 last:border-b-0"
    >
      <div className="flex min-w-0 flex-1 flex-col">
        <span className="truncate text-[13px] font-medium">{item.name}</span>
        <span className="text-[11px] text-muted-foreground tabular-nums">
          Deleted{" "}
          <time
            dateTime={new Date(item.deletedAt).toISOString()}
            title={new Date(item.deletedAt).toLocaleString()}
          >
            {relativeTime(item.deletedAt)}
          </time>{" "}
          · {formatBytes(item.sizeBytes)}
        </span>
      </div>
      <Button
        variant="outline"
        size="sm"
        disabled={restore.isPending}
        onClick={() =>
          restore.mutate(item.id, {
            onSuccess: (ws) => toastSuccess(`Restored ${ws.name}`),
            onError: (error) => toastError(error, "Could not restore"),
          })
        }
        data-testid={`trash-restore-${item.id}`}
      >
        {restore.isPending ? (
          <Spinner data-icon="inline-start" />
        ) : (
          <RotateCcwIcon data-icon="inline-start" />
        )}
        Restore
      </Button>
      <Button
        variant="destructive"
        size="sm"
        onClick={() => onPurge(item)}
        data-testid={`trash-purge-${item.id}`}
      >
        <XIcon data-icon="inline-start" />
        Purge
      </Button>
    </li>
  )
}

function PurgeDialog({
  item,
  onOpenChange,
}: {
  item: TrashItem | null
  onOpenChange: (open: boolean) => void
}) {
  const purge = usePurgeFromTrash()
  const confirm = () => {
    if (!item) return
    purge.mutate(item.id, {
      onSuccess: () => {
        toastSuccess(`Purged ${item.name}`)
        onOpenChange(false)
      },
      onError: (error) => toastError(error, "Could not purge"),
    })
  }
  return (
    <AlertDialog open={item !== null} onOpenChange={onOpenChange}>
      <AlertDialogContent data-testid="purge-dialog">
        <AlertDialogHeader>
          <AlertDialogTitle>Purge {item?.name}?</AlertDialogTitle>
          <AlertDialogDescription>
            This permanently deletes the workspace
            {item ? ` and its ${formatBytes(item.sizeBytes)} of data` : ""}. It
            cannot be restored afterwards.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel data-testid="purge-cancel">
            Cancel
          </AlertDialogCancel>
          <Button
            variant="destructive"
            disabled={purge.isPending}
            onClick={confirm}
            data-testid="purge-confirm"
          >
            {purge.isPending ? <Spinner data-icon="inline-start" /> : null}
            Purge forever
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}

/** `/trash`: deleted workspaces with restore and purge (docs/web/06 §Settings). */
export function TrashScreen() {
  usePageTitle("Trash")
  const trash = useTrash()
  const [purging, setPurging] = useState<TrashItem | null>(null)
  const items = trash.data ?? []
  const error = trash.isError ? toApiError(trash.error) : null

  return (
    <section
      data-testid="screen-trash"
      aria-busy={trash.isPending}
      className="flex min-h-full flex-col gap-6 p-6"
    >
      <PageHeader
        title="Trash"
        description="Deleted workspaces you can still restore."
      />
      {trash.isPending ? (
        <div
          role="status"
          aria-label="Loading"
          className="flex max-w-3xl flex-col gap-px border"
        >
          {Array.from({ length: 3 }, (_, i) => (
            <SkeletonBlock key={i} className="h-[58px] w-full" />
          ))}
        </div>
      ) : error ? (
        <ErrorState
          title="Could not load the trash"
          message={error.message}
          {...(error.requestId ? { requestId: error.requestId } : {})}
          onRetry={() => trash.refetch()}
        />
      ) : items.length === 0 ? (
        <EmptyState
          icon={Trash2Icon}
          title="Trash is empty"
          description="Deleted workspaces stay here until you purge them."
          className="flex-1"
          focusAction={false}
        />
      ) : (
        <ul
          role="list"
          aria-label="Deleted workspaces"
          data-testid="trash-list"
          className="max-w-3xl border"
        >
          {items.map((item) => (
            <TrashRow key={item.id} item={item} onPurge={setPurging} />
          ))}
        </ul>
      )}
      <PurgeDialog
        item={purging}
        onOpenChange={(open) => {
          if (!open) setPurging(null)
        }}
      />
    </section>
  )
}
