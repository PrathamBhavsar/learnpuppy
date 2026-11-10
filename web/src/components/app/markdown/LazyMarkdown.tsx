import { Suspense, lazy } from "react"

import { cn } from "@/lib/utils"

import { mathSource } from "./math-source"
import { completePartial } from "./partial"

const Markdown = lazy(() => import("./Markdown"))
const MarkdownMath = lazy(() => import("./MarkdownMath"))

/** `$x$` or `$$…$$`; plain dollar amounts ("$5 and $10") also match, which only costs a chunk fetch. */
const MATH = /\$\$[\s\S]+?\$\$|\$[^$\n]+\$/

type LazyMarkdownProps = {
  children: string
  className?: string
  /** The message is still arriving: close open syntax so nothing renders raw. */
  streaming?: boolean
}

/** Loads react-markdown (and KaTeX, if needed) on first use; shows the raw text meanwhile so nothing shifts. */
export function LazyMarkdown({
  children,
  className,
  streaming = false,
}: LazyMarkdownProps) {
  const text = streaming ? completePartial(children) : children
  const source = mathSource(text)
  const Renderer = MATH.test(source) ? MarkdownMath : Markdown
  return (
    <Suspense
      fallback={
        <div
          dir="auto"
          className={cn(
            "text-[14px] leading-[1.55] whitespace-pre-wrap",
            className
          )}
        >
          {text}
        </div>
      }
    >
      <Renderer className={className}>{source}</Renderer>
    </Suspense>
  )
}
