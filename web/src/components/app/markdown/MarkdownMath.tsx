import "katex/dist/katex.min.css"

import rehypeKatex from "rehype-katex"
import remarkGfm from "remark-gfm"
import remarkMath from "remark-math"

import { type MarkdownViewProps, MarkdownView } from "./MarkdownView"

const REMARK = [remarkGfm, remarkMath]
const REHYPE = [rehypeKatex]

/** Markdown with `$inline$` and `$$block$$` math rendered by KaTeX (loaded only when a message has math). */
export default function MarkdownMath(props: MarkdownViewProps) {
  return (
    <MarkdownView {...props} remarkPlugins={REMARK} rehypePlugins={REHYPE} />
  )
}
