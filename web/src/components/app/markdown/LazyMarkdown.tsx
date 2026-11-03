import { Suspense, lazy } from "react"

import { cn } from "@/lib/utils"

import { mathSource } from "./math-source"

const Markdown = lazy(() => import("./Markdown"))
const MarkdownMath = lazy(() => import("./MarkdownMath"))

/** `$x$` or `$$…$$`; plain dollar amounts ("$5 and $10") also match, which only costs a chunk fetch. */
const MATH = /\$\$[\s\S]+?\$\$|\$[^$\n]+\$/

type LazyMarkdownProps = { children: string; className?: string }

/** Loads react-markdown (and KaTeX, if needed) on first use; shows the raw text meanwhile so nothing shifts. */
export function LazyMarkdown({ children, className }: LazyMarkdownProps) {
  const source = mathSource(children)
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
          {children}
        </div>
      }
    >
      <Renderer className={className}>{source}</Renderer>
    </Suspense>
  )
}
