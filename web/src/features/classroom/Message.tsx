import {
  CheckIcon,
  CopyIcon,
  MessageSquareQuoteIcon,
  OctagonXIcon,
  WrenchIcon,
} from "lucide-react"
import { useState } from "react"

import type { AnswerRecap, ClassroomMessage } from "@/api/pending-types"
import { Badge } from "@/components/ui/badge"
import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

import { AnswerVerdict } from "./AnswerVerdict"
import { COPY, formatTime, toolLabel } from "./copy"
import { LazyMarkdown } from "./markdown/LazyMarkdown"
import { QuestionRecap } from "./QuestionRecap"
import { ThinkingBlock } from "./ThinkingBlock"

type Verdict = {
  callId: string
  correct: boolean
  explanation?: string | null
}

type MessageProps = {
  message: ClassroomMessage
  verdict?: Verdict
  /** The answered question this message replies to, kept above the verdict (docs/web/06). */
  recap?: AnswerRecap
  /** Animate in (messages that arrive after the list first rendered). */
  animate: boolean
}

function CopyButton({ text }: { text: string }) {
  const [copied, setCopied] = useState(false)
  const copy = async () => {
    try {
      await navigator.clipboard.writeText(text)
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1500)
    } catch {
      /* clipboard denied */
    }
  }
  return (
    <Button
      variant="ghost"
      size="icon-xs"
      onClick={() => void copy()}
      aria-label="Copy message"
      data-testid="message-copy"
      className="opacity-0 group-focus-within/message:opacity-100 group-hover/message:opacity-100"
    >
      {copied ? <CheckIcon /> : <CopyIcon />}
    </Button>
  )
}

function CallChips({ calls }: { calls: ClassroomMessage["calls"] }) {
  if (calls.length === 0) return null
  return (
    <ul className="flex flex-wrap gap-1.5" aria-label="Tools used">
      {calls.map((call) => (
        <li key={call.id}>
          <Badge
            variant="outline"
            data-testid={`call-chip-${call.name}`}
            className="font-normal text-muted-foreground"
          >
            <WrenchIcon aria-hidden />
            {toolLabel(call.name)}
          </Badge>
        </li>
      ))}
    </ul>
  )
}

export function Message({ message, verdict, recap, animate }: MessageProps) {
  const rise = animate ? "lp-rise" : undefined
  const { role } = message
  const when =
    message.createdAt == null ? undefined : formatTime(message.createdAt)

  if (role === "compaction") {
    return (
      <div
        role="separator"
        data-testid="message-compaction"
        className={cn("flex items-center gap-3 py-1", rise)}
      >
        <span className="h-px flex-1 bg-border" />
        <span
          className="text-[11px] text-muted-foreground"
          title={message.text}
        >
          {COPY.compaction}
        </span>
        <span className="h-px flex-1 bg-border" />
      </div>
    )
  }

  if (role === "tool") {
    return (
      <div
        data-testid="message-tool"
        className={cn(
          "flex items-start gap-2 text-[12px] text-muted-foreground",
          rise
        )}
      >
        <WrenchIcon aria-hidden className="mt-0.5 size-3 shrink-0" />
        <LazyMarkdown className="min-w-0 text-[12px]">
          {message.text}
        </LazyMarkdown>
      </div>
    )
  }

  if (role === "user" || role === "steer") {
    return (
      <div
        data-testid={role === "steer" ? "message-steer" : "message-user"}
        data-message-id={message.id}
        className={cn("group/message flex flex-col items-end gap-1", rise)}
      >
        {recap ? (
          <QuestionRecap recap={recap} />
        ) : (
          <div className="flex max-w-[85%] items-start gap-1">
            <div className="order-last">
              <CopyButton text={message.text} />
            </div>
            <div
              title={when}
              className="min-w-0 border bg-secondary px-3 py-2 text-secondary-foreground"
            >
              {role === "steer" ? (
                <Badge variant="outline" className="mb-1 font-normal">
                  <MessageSquareQuoteIcon aria-hidden />
                  Steer
                </Badge>
              ) : null}
              <LazyMarkdown>{message.text}</LazyMarkdown>
            </div>
          </div>
        )}
        {verdict ? (
          <AnswerVerdict
            callId={verdict.callId}
            correct={verdict.correct}
            explanation={verdict.explanation}
          />
        ) : null}
      </div>
    )
  }

  return (
    <div
      data-testid="message-assistant"
      data-message-id={message.id}
      className={cn("group/message flex flex-col items-start gap-1.5", rise)}
    >
      {message.thinking ? <ThinkingBlock text={message.thinking} /> : null}
      <div className="flex max-w-full items-start gap-1" title={when}>
        <div className="min-w-0">
          {message.text.trim() ? (
            <LazyMarkdown>{message.text}</LazyMarkdown>
          ) : null}
        </div>
        {message.text.trim() ? <CopyButton text={message.text} /> : null}
      </div>
      <CallChips calls={message.calls} />
      {message.interrupted ? (
        <Badge
          variant="outline"
          data-testid="message-interrupted"
          className="font-normal text-warning"
        >
          <OctagonXIcon aria-hidden />
          {COPY.interrupted}
        </Badge>
      ) : null}
    </div>
  )
}

export { CallChips }
