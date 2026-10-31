import { zodResolver } from "@hookform/resolvers/zod"
import { useForm } from "react-hook-form"
import { z } from "zod"

import { toApiError } from "@/api/errors"
import type { ChatSummary } from "@/api/pending-types"
import { useDeleteChat, useRenameChat } from "@/api/queries/chats"
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Input } from "@/components/ui/input"
import { Label } from "@/components/ui/label"
import { Spinner } from "@/components/ui/spinner"
import { toastError, toastSuccess } from "@/lib/toast"

const schema = z.object({
  title: z
    .string()
    .trim()
    .min(1, "Give the chat a name")
    .max(120, "Keep it under 120 characters"),
})
type Values = z.infer<typeof schema>

type RenameProps = {
  ws: string
  chat: ChatSummary | null
  onClose: () => void
}

export function RenameChatDialog({ ws, chat, onClose }: RenameProps) {
  return (
    <Dialog open={chat !== null} onOpenChange={(open) => !open && onClose()}>
      <DialogContent data-testid="rename-dialog">
        {chat ? (
          <RenameForm key={chat.id} ws={ws} chat={chat} onClose={onClose} />
        ) : null}
      </DialogContent>
    </Dialog>
  )
}

function RenameForm({
  ws,
  chat,
  onClose,
}: {
  ws: string
  chat: ChatSummary
  onClose: () => void
}) {
  const rename = useRenameChat(ws)
  const form = useForm<Values>({
    resolver: zodResolver(schema),
    defaultValues: { title: chat.title },
  })
  const submit = form.handleSubmit((values) =>
    rename.mutateAsync({ id: chat.id, title: values.title }).then(
      () => {
        toastSuccess("Chat renamed")
        onClose()
      },
      (error: unknown) => {
        const api = toApiError(error)
        const field = api.fields.find((f) => f.path === "title")
        if (field) form.setError("title", { message: field.message })
        else toastError(error, "Could not rename the chat")
      }
    )
  )
  const error = form.formState.errors.title?.message
  return (
    <form onSubmit={submit} noValidate className="flex flex-col gap-4">
      <DialogHeader>
        <DialogTitle>Rename chat</DialogTitle>
        <DialogDescription>
          The title only changes how it appears in your list.
        </DialogDescription>
      </DialogHeader>
      <div className="flex flex-col gap-1.5">
        <Label htmlFor="rename-title">Title</Label>
        <Input
          id="rename-title"
          aria-invalid={error ? true : undefined}
          aria-describedby={error ? "rename-title-error" : undefined}
          data-testid="rename-input"
          {...form.register("title")}
        />
        {error ? (
          <p
            id="rename-title-error"
            role="alert"
            className="text-[12px] text-destructive"
          >
            {error}
          </p>
        ) : null}
      </div>
      <DialogFooter>
        <Button type="button" variant="outline" onClick={onClose}>
          Cancel
        </Button>
        <Button
          type="submit"
          disabled={rename.isPending}
          data-testid="rename-submit"
        >
          {rename.isPending ? <Spinner data-icon="inline-start" /> : null}
          Save
        </Button>
      </DialogFooter>
    </form>
  )
}

type DeleteProps = {
  ws: string
  chat: ChatSummary | null
  onClose: () => void
  onDeleted: (chat: ChatSummary) => void
}

export function DeleteChatDialog({
  ws,
  chat,
  onClose,
  onDeleted,
}: DeleteProps) {
  const remove = useDeleteChat(ws)
  return (
    <AlertDialog
      open={chat !== null}
      onOpenChange={(open) => !open && onClose()}
    >
      <AlertDialogContent data-testid="delete-dialog">
        <AlertDialogHeader>
          <AlertDialogTitle>Delete this chat?</AlertDialogTitle>
          <AlertDialogDescription>
            “{chat?.title}” moves to this workspace&apos;s chat trash with its{" "}
            {chat?.messageCount ?? 0} messages. Knowledge already added to the
            graph stays.
          </AlertDialogDescription>
        </AlertDialogHeader>
        <AlertDialogFooter>
          <AlertDialogCancel>Cancel</AlertDialogCancel>
          <AlertDialogAction
            variant="destructive"
            disabled={remove.isPending}
            data-testid="delete-confirm"
            onClick={(event) => {
              event.preventDefault()
              if (!chat) return
              remove.mutate(chat.id, {
                onSuccess: () => {
                  toastSuccess("Chat deleted")
                  onDeleted(chat)
                  onClose()
                },
                onError: (e) => toastError(e, "Could not delete the chat"),
              })
            }}
          >
            {remove.isPending ? <Spinner data-icon="inline-start" /> : null}
            Delete
          </AlertDialogAction>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  )
}
