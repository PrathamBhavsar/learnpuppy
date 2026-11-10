import { LazyMarkdown } from "@/components/app/markdown/LazyMarkdown"
import { cn } from "@/lib/utils"

type StreamingTextProps = { text: string; className?: string }

/**
 * The in-flight reply as Markdown, with open syntax closed by `completePartial` so a half-written
 * fence, formula or bold never shows raw. A static block caret pulses at the end.
 */
export function StreamingText({ text, className }: StreamingTextProps) {
  return (
    <div
      data-testid="streaming-text"
      aria-live="off"
      className={cn("break-words", className)}
    >
      <LazyMarkdown streaming>{text}</LazyMarkdown>
      <span aria-hidden className="lp-caret" />
    </div>
  )
}
