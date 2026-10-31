import { zodResolver } from "@hookform/resolvers/zod"
import { useNavigate } from "@tanstack/react-router"
import { TriangleAlertIcon } from "lucide-react"
import { useState } from "react"
import { Controller, useForm, useWatch } from "react-hook-form"
import { z } from "zod"

import { toApiError } from "@/api/errors"
import type { ChatKind } from "@/api/pending-types"
import { useAccounts } from "@/api/queries/accounts"
import { useCreateChat } from "@/api/queries/chats"
import { useSettings } from "@/api/queries/settings"
import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { Label } from "@/components/ui/label"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { toastError } from "@/lib/toast"

import { NodePicker } from "./NodePicker"

const schema = z.object({
  kind: z.enum(["teach", "practice"]),
  topic: z.string().trim().max(2000, "Keep the topic under 2000 characters"),
})
type Values = z.infer<typeof schema>

const ROLES: Record<ChatKind, string[]> = {
  teach: ["prober", "teacher"],
  practice: ["practice"],
}

type NewChatDialogProps = {
  ws: string
  open: boolean
  onOpenChange: (open: boolean) => void
  /** Pre-filled topic (the empty state's one-step input). */
  initialTopic?: string
}

export function NewChatDialog({
  ws,
  open,
  onOpenChange,
  initialTopic,
}: NewChatDialogProps) {
  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent
        className="sm:max-w-lg"
        data-testid="new-chat-dialog"
        onOpenAutoFocus={(event) => {
          event.preventDefault()
          document.getElementById("new-chat-topic")?.focus()
        }}
      >
        {open ? (
          <NewChatForm
            ws={ws}
            initialTopic={initialTopic ?? ""}
            onClose={() => onOpenChange(false)}
          />
        ) : null}
      </DialogContent>
    </Dialog>
  )
}

function NewChatForm({
  ws,
  initialTopic,
  onClose,
}: {
  ws: string
  initialTopic: string
  onClose: () => void
}) {
  const navigate = useNavigate()
  const create = useCreateChat(ws)
  const settings = useSettings()
  const accounts = useAccounts()
  const [seeds, setSeeds] = useState<string[]>([])
  const [picker, setPicker] = useState(false)
  const form = useForm<Values>({
    resolver: zodResolver(schema),
    defaultValues: { kind: "teach", topic: initialTopic },
  })
  const kind = useWatch({ control: form.control, name: "kind" })
  const ready = (accounts.data ?? []).filter((a) => a.status === "ready").length
  const noAccount = accounts.isSuccess && ready === 0

  const submit = form.handleSubmit((values) => {
    if (values.kind === "teach" && values.topic === "") {
      form.setError("topic", { message: "Say what you want to learn" })
      return
    }
    if (
      values.kind === "practice" &&
      values.topic === "" &&
      seeds.length === 0
    ) {
      form.setError("topic", {
        message: "Add a focus or pick at least one graph node",
      })
      return
    }
    return create
      .mutateAsync({
        kind: values.kind,
        topic: values.topic,
        seedNodeIds: seeds,
      })
      .then(
        ({ chatId }) => {
          onClose()
          void navigate({
            to: "/w/$ws/learn/$chat",
            params: { ws, chat: chatId },
          })
        },
        (error: unknown) => {
          const field = toApiError(error).fields.find((f) => f.path === "topic")
          if (field) form.setError("topic", { message: field.message })
          else toastError(error, "Could not start the chat")
        }
      )
  })

  const topicError = form.formState.errors.topic?.message
  const profiles = settings.data?.profiles ?? {}

  return (
    <form onSubmit={submit} noValidate className="flex flex-col gap-4">
      <DialogHeader>
        <DialogTitle>New chat</DialogTitle>
        <DialogDescription>
          Teach builds a lesson from a topic. Practice drills what the graph
          already holds.
        </DialogDescription>
      </DialogHeader>

      <Controller
        control={form.control}
        name="kind"
        render={({ field }) => (
          <ToggleGroup
            type="single"
            variant="outline"
            value={field.value}
            onValueChange={(next) => next && field.onChange(next)}
            aria-label="Chat kind"
            data-testid="new-chat-kind"
          >
            <ToggleGroupItem value="teach" data-testid="kind-teach">
              Teach
            </ToggleGroupItem>
            <ToggleGroupItem value="practice" data-testid="kind-practice">
              Practice
            </ToggleGroupItem>
          </ToggleGroup>
        )}
      />

      <div className="flex flex-col gap-1.5">
        <Label htmlFor="new-chat-topic">
          {kind === "teach" ? "Topic" : "Focus (optional with graph nodes)"}
        </Label>
        <Textarea
          id="new-chat-topic"
          dir="auto"
          rows={3}
          placeholder={
            kind === "teach"
              ? "e.g. how async Rust works under the hood"
              : "e.g. pinning and Unpin"
          }
          aria-invalid={topicError ? true : undefined}
          aria-describedby={topicError ? "new-chat-topic-error" : undefined}
          data-testid="new-chat-topic"
          {...form.register("topic")}
        />
        {topicError ? (
          <p
            id="new-chat-topic-error"
            role="alert"
            className="text-[12px] text-destructive"
          >
            {topicError}
          </p>
        ) : null}
      </div>

      <div className="flex flex-col gap-2">
        <Button
          type="button"
          variant="ghost"
          size="sm"
          className="self-start"
          aria-expanded={picker}
          onClick={() => setPicker((v) => !v)}
          data-testid="new-chat-use-nodes"
        >
          {picker ? "Hide graph nodes" : "Use nodes from the graph"}
          {seeds.length > 0 ? ` (${seeds.length})` : ""}
        </Button>
        {picker ? (
          <NodePicker ws={ws} selected={seeds} onChange={setSeeds} />
        ) : null}
      </div>

      <div
        className="border bg-muted/30 px-3 py-2 text-[12px]"
        data-testid="new-chat-roles"
      >
        <p className="mb-1 font-medium">What will run</p>
        {settings.isPending ? (
          <p className="text-muted-foreground">Reading settings…</p>
        ) : (
          <ul className="flex flex-col gap-0.5 text-muted-foreground">
            {ROLES[kind].map((role) => {
              const p = profiles[role]
              return (
                <li key={role}>
                  <span className="capitalize">{role}</span>:{" "}
                  {p
                    ? `${p.provider} / ${p.model}${p.effort ? ` (${p.effort})` : ""}`
                    : "not configured"}
                </li>
              )
            })}
          </ul>
        )}
      </div>

      {noAccount ? (
        <p
          role="alert"
          data-testid="new-chat-no-account"
          className="flex items-start gap-2 border border-warning/40 bg-warning/10 px-3 py-2 text-[12px]"
        >
          <TriangleAlertIcon
            aria-hidden
            className="mt-0.5 size-3.5 shrink-0 text-warning"
          />
          No model account is ready. The chat will be created but the tutor
          cannot answer until one is.
        </p>
      ) : null}

      <DialogFooter>
        <Button type="button" variant="outline" onClick={onClose}>
          Cancel
        </Button>
        <Button
          type="submit"
          disabled={create.isPending}
          data-testid="new-chat-submit"
        >
          {create.isPending ? <Spinner data-icon="inline-start" /> : null}
          Start
        </Button>
      </DialogFooter>
    </form>
  )
}
