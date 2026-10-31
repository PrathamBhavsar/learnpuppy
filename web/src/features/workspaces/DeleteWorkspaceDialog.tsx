import { useState } from "react"

import { toApiError } from "@/api/errors"
import type { WorkspaceListItem } from "@/api/pending-types"
import { useDeleteWorkspace } from "@/api/queries/workspaces"
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
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Spinner } from "@/components/ui/spinner"
import { toastError, toastSuccess } from "@/lib/toast"

type Props = {
  workspace: WorkspaceListItem | null
  onOpenChange: (open: boolean) => void
  onDeleted?: (workspace: WorkspaceListItem) => void
}

function DeleteBody({
  workspace,
  onOpenChange,
  onDeleted,
}: Props & { workspace: WorkspaceListItem }) {
  const remove = useDeleteWorkspace()
  const [typed, setTyped] = useState("")
  const [serverError, setServerError] = useState<string | null>(null)
  const matches = typed === workspace.name
  const { nodes, chains, chats } = workspace.stats

  const confirm = async () => {
    if (!matches || remove.isPending) return
    setServerError(null)
    try {
      await remove.mutateAsync({ ws: workspace.id, confirmName: typed })
      toastSuccess(`Moved ${workspace.name} to the trash`)
      onOpenChange(false)
      onDeleted?.(workspace)
    } catch (error) {
      const api = toApiError(error)
      const field = api.fields.find((f) => f.path === "confirmName")
      if (field) setServerError(field.message)
      else toastError(api, "Could not delete")
    }
  }

  return (
    <AlertDialogContent data-testid="delete-dialog">
      <AlertDialogHeader>
        <AlertDialogTitle>Delete {workspace.name}?</AlertDialogTitle>
        <AlertDialogDescription>
          This moves the workspace with its {nodes} nodes, {chains} chains and{" "}
          {chats} chats to the trash and stops anything it is running. You can
          restore it from the trash until you purge it.
        </AlertDialogDescription>
      </AlertDialogHeader>
      <form
        className="flex flex-col gap-1.5"
        onSubmit={(event) => {
          event.preventDefault()
          void confirm()
        }}
      >
        <Label htmlFor="delete-confirm">
          Type <strong className="font-semibold">{workspace.name}</strong> to
          confirm
        </Label>
        <Input
          id="delete-confirm"
          data-testid="delete-confirm-input"
          autoComplete="off"
          value={typed}
          onChange={(event) => {
            setTyped(event.target.value)
            setServerError(null)
          }}
          aria-invalid={serverError ? true : undefined}
          aria-describedby={serverError ? "delete-confirm-error" : undefined}
        />
        {serverError ? (
          <p
            id="delete-confirm-error"
            role="alert"
            className="text-[12px] text-destructive"
          >
            {serverError}
          </p>
        ) : null}
      </form>
      <AlertDialogFooter>
        <AlertDialogCancel data-testid="delete-cancel">
          Cancel
        </AlertDialogCancel>
        <Button
          variant="destructive"
          disabled={!matches || remove.isPending}
          onClick={() => void confirm()}
          data-testid="delete-confirm"
          className="transition-colors duration-[var(--dur-fast)]"
        >
          {remove.isPending ? <Spinner data-icon="inline-start" /> : null}
          Delete workspace
        </Button>
      </AlertDialogFooter>
    </AlertDialogContent>
  )
}

/** Typed-name confirmation (docs/web/06 §Cross-cutting): the destructive button stays disabled until the name matches. */
export function DeleteWorkspaceDialog({
  workspace,
  onOpenChange,
  onDeleted,
}: Props) {
  return (
    <AlertDialog open={workspace !== null} onOpenChange={onOpenChange}>
      {workspace ? (
        <DeleteBody
          key={workspace.id}
          workspace={workspace}
          onOpenChange={onOpenChange}
          onDeleted={onDeleted}
        />
      ) : null}
    </AlertDialog>
  )
}
