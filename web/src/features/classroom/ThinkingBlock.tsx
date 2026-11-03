import { ChevronRightIcon } from "lucide-react"
import { useState } from "react"

import {
  Collapsible,
  CollapsibleContent,
  CollapsibleTrigger,
} from "@/components/ui/collapsible"

type ThinkingBlockProps = {
  text: string
  /** The model is still thinking: animated dots, label "Thinking…". */
  live?: boolean
}

/** Collapsed "Thinking…" row; opening it shows the reasoning text. Collapses again once the reply lands. */
export function ThinkingBlock({ text, live = false }: ThinkingBlockProps) {
  const [open, setOpen] = useState(false)
  if (!live && text.trim() === "") return null
  return (
    <Collapsible
      open={open}
      onOpenChange={setOpen}
      data-testid="thinking-block"
      className="w-full min-w-0 text-[12px] text-muted-foreground"
    >
      <CollapsibleTrigger
        className="flex items-center gap-1.5 py-0.5 outline-none focus-visible:ring-1 focus-visible:ring-ring/50"
        data-testid="thinking-toggle"
      >
        <ChevronRightIcon
          aria-hidden
          className={
            open
              ? "size-3 rotate-90 transition-transform rtl:-rotate-90"
              : "size-3 transition-transform rtl:-scale-x-100"
          }
        />
        {live ? (
          <>
            <span>Thinking</span>
            <span aria-hidden className="lp-dots">
              <span />
              <span />
              <span />
            </span>
          </>
        ) : (
          <span>Thought process</span>
        )}
      </CollapsibleTrigger>
      <CollapsibleContent className="max-h-64 overflow-y-auto overscroll-contain">
        <p
          dir="auto"
          className="mt-1 border-s ps-3 leading-[1.5] whitespace-pre-wrap"
        >
          {text || "…"}
        </p>
      </CollapsibleContent>
    </Collapsible>
  )
}
