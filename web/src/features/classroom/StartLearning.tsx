import { useNavigate } from "@tanstack/react-router"
import { GraduationCapIcon, SendIcon } from "lucide-react"
import { useState } from "react"

import { toApiError } from "@/api/errors"
import { useCreateChat } from "@/api/queries/chats"
import { Button } from "@/components/ui/button"
import { Spinner } from "@/components/ui/spinner"
import { Textarea } from "@/components/ui/textarea"
import { toastError } from "@/lib/toast"

import { COPY } from "./copy"

/** Empty `/learn`: name a topic and the chat is created and opened in one step. */
export function StartLearning({ ws }: { ws: string }) {
  const navigate = useNavigate()
  const create = useCreateChat(ws)
  const [topic, setTopic] = useState("")
  const [error, setError] = useState<string | null>(null)

  const start = () => {
    const value = topic.trim()
    if (value === "") {
      setError("Say what you want to learn")
      return
    }
    setError(null)
    create.mutate(
      { kind: "teach", topic: value },
      {
        onSuccess: ({ chatId }) =>
          void navigate({
            to: "/w/$ws/learn/$chat",
            params: { ws, chat: chatId },
          }),
        onError: (e) => {
          const field = toApiError(e).fields.find((f) => f.path === "topic")
          if (field) setError(field.message)
          else toastError(e, "Could not start the chat")
        },
      }
    )
  }

  return (
    <div
      data-testid="start-learning"
      className="mx-auto flex h-full max-w-xl flex-col items-center justify-center gap-4 px-6 text-center"
    >
      <GraduationCapIcon
        aria-hidden
        className="lp-float-in size-8 text-muted-foreground"
        strokeWidth={1.5}
      />
      <div className="flex flex-col gap-1">
        <h2 className="font-heading text-[16px] font-medium">
          {COPY.startTitle}
        </h2>
        <p className="text-[13px] text-muted-foreground">{COPY.startBody}</p>
      </div>
      <div className="flex w-full flex-col gap-2 text-start">
        <Textarea
          value={topic}
          onChange={(event) => setTopic(event.target.value)}
          onKeyDown={(event) => {
            if (event.key === "Enter" && !event.shiftKey) {
              event.preventDefault()
              if (!create.isPending) start()
            }
          }}
          dir="auto"
          rows={2}
          disabled={create.isPending}
          aria-label="Topic"
          aria-invalid={error ? true : undefined}
          placeholder={COPY.startPlaceholder}
          data-testid="start-topic"
        />
        {error ? (
          <p role="alert" className="text-[12px] text-destructive">
            {error}
          </p>
        ) : null}
        <Button
          onClick={start}
          disabled={create.isPending}
          className="self-end"
          data-testid="start-submit"
        >
          {create.isPending ? (
            <Spinner data-icon="inline-start" />
          ) : (
            <SendIcon data-icon="inline-start" className="rtl:-scale-x-100" />
          )}
          Start learning
        </Button>
      </div>
    </div>
  )
}
