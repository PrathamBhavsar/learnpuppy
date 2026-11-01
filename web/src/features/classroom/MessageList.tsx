import { ArrowDownIcon } from "lucide-react"
import { AnimatePresence, motion } from "motion/react"
import { useState } from "react"

import type { ClassroomView } from "@/api/pending-types"
import { useStreamingTurn } from "@/hooks/use-streaming-turn"
import { spring } from "@/lib/motion"

import { COPY, toolLabel } from "./copy"
import { Message, CallChips } from "./Message"
import { StreamingText } from "./StreamingText"
import { ThinkingBlock } from "./ThinkingBlock"
import { useStickToBottom } from "./use-stick-to-bottom"

type MessageListProps = {
  chatId: string
  view: ClassroomView
}

export function MessageList({ chatId, view }: MessageListProps) {
  const live = useStreamingTurn(chatId)
  // Messages present on first render do not animate; later ones rise in.
  const [initialIds] = useState(
    () => new Set(view.messages.map((message) => message.id))
  )
  const verdicts = new Map(view.verdicts.map((v) => [v.callId, v]))
  const streaming = view.turn.state === "running"
  const liveText = live?.text ?? ""
  const liveThinking = live?.thinking ?? ""
  const { scrollRef, contentRef, atBottom, onScroll, jumpToLatest } =
    useStickToBottom(
      `${view.messages.length}:${liveText.length}:${liveThinking.length}:${streaming}`
    )

  return (
    <div className="relative min-h-0 flex-1">
      <div
        ref={scrollRef}
        onScroll={onScroll}
        data-testid="message-list"
        role="log"
        aria-label="Conversation"
        aria-live="polite"
        aria-relevant="additions"
        // A scroll region must be focusable so keyboard users can scroll it.
        // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
        tabIndex={0}
        className="lp-pane absolute inset-0 outline-none focus-visible:ring-1 focus-visible:ring-ring/50"
      >
        <div
          ref={contentRef}
          className="mx-auto flex w-full max-w-[72ch] flex-col gap-4 px-4 py-4"
        >
          {view.messages.map((message) => (
            <Message
              key={message.id}
              message={message}
              animate={!initialIds.has(message.id)}
              verdict={
                message.answersCallId
                  ? verdicts.get(message.answersCallId)
                  : undefined
              }
            />
          ))}
          {streaming ? (
            <div
              data-testid="message-streaming"
              className="flex flex-col items-start gap-1.5"
            >
              <ThinkingBlock text={liveThinking} live={liveText === ""} />
              {liveText ? <StreamingText text={liveText} /> : null}
              {live && live.calls.length > 0 ? (
                <CallChips calls={live.calls} />
              ) : null}
              <span className="sr-only">
                {live?.calls.map((c) => toolLabel(c.name)).join(", ")}
              </span>
            </div>
          ) : null}
        </div>
      </div>
      <AnimatePresence>
        {atBottom ? null : (
          <motion.button
            type="button"
            data-testid="jump-to-latest"
            onClick={jumpToLatest}
            initial={{ opacity: 0, y: 12 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0, y: 8 }}
            transition={spring.snappy}
            className="absolute start-1/2 bottom-3 flex -translate-x-1/2 items-center gap-1.5 border bg-background px-3 py-1.5 text-[12px] shadow-sm outline-none hover:bg-muted focus-visible:ring-1 focus-visible:ring-ring/50 rtl:translate-x-1/2"
          >
            <ArrowDownIcon aria-hidden className="size-3" />
            {COPY.jumpToLatest}
          </motion.button>
        )}
      </AnimatePresence>
    </div>
  )
}
