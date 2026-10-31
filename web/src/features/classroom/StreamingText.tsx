import { cn } from "@/lib/utils"

/** Fixed-size slices keep the fade stable: a slice mounts once, so only new text animates. */
const SLICE = 16

type StreamingTextProps = { text: string; className?: string }

/**
 * The in-flight reply as plain text (Markdown lands with the settled message, so half-written
 * fences never flicker). New slices fade in over 120 ms; a static block caret pulses at the end.
 */
export function StreamingText({ text, className }: StreamingTextProps) {
  const slices: string[] = []
  for (let i = 0; i < text.length; i += SLICE)
    slices.push(text.slice(i, i + SLICE))
  return (
    <div
      dir="auto"
      data-testid="streaming-text"
      aria-live="off"
      className={cn(
        "text-[14px] leading-[1.55] break-words whitespace-pre-wrap",
        className
      )}
    >
      {slices.map((slice, index) => (
        <span key={index} className={index === 0 ? undefined : "lp-chunk"}>
          {slice}
        </span>
      ))}
      <span aria-hidden className="lp-caret" />
    </div>
  )
}
