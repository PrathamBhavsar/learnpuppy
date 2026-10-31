import type { ComponentProps } from "react"
import ReactMarkdown from "react-markdown"
import remarkGfm from "remark-gfm"

import { cn } from "@/lib/utils"

import "../styles.css"
import { CodeBlock } from "./CodeBlock"

type Plugins = ComponentProps<typeof ReactMarkdown>

const COMPONENTS: Plugins["components"] = {
  a: ({ href, children }) => (
    <a
      href={href}
      target="_blank"
      rel="noopener noreferrer"
      className="text-primary underline underline-offset-2"
    >
      {children}
    </a>
  ),
  pre: ({ children }) => <>{children}</>,
  code: ({ className, children }) => {
    const text = String(children)
    const lang = /language-([\w+-]+)/.exec(className ?? "")?.[1]
    // A fence yields a `language-*` class or a trailing newline; inline code has neither.
    if (lang || text.endsWith("\n"))
      return <CodeBlock code={text.replace(/\n$/, "")} lang={lang} />
    return (
      <code className="bg-muted px-1 py-0.5 text-[0.92em]">{children}</code>
    )
  },
  table: ({ children }) => (
    <div className="my-2 overflow-x-auto">
      <table className="w-full border-collapse text-[13px]">{children}</table>
    </div>
  ),
  th: ({ children }) => (
    <th className="border bg-muted/40 px-2 py-1 text-start font-medium">
      {children}
    </th>
  ),
  td: ({ children }) => <td className="border px-2 py-1">{children}</td>,
}

const GFM = [remarkGfm]

export type MarkdownViewProps = {
  children: string
  className?: string
  remarkPlugins?: Plugins["remarkPlugins"]
  rehypePlugins?: Plugins["rehypePlugins"]
}

/** Chat Markdown: GFM and highlighted code. Message prose follows its own direction. */
export function MarkdownView({
  children,
  className,
  remarkPlugins,
  rehypePlugins,
}: MarkdownViewProps) {
  return (
    <div
      dir="auto"
      className={cn("lp-prose text-[14px] leading-[1.55]", className)}
    >
      <ReactMarkdown
        remarkPlugins={remarkPlugins ?? GFM}
        rehypePlugins={rehypePlugins}
        components={COMPONENTS}
      >
        {children}
      </ReactMarkdown>
    </div>
  )
}
