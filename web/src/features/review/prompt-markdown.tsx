import { Fragment, type ReactNode } from "react"

import { cn } from "@/lib/utils"

/** Inline spans: `code`, **bold**, *italic*. Plain text otherwise; never injects HTML. */
function inline(text: string, keyPrefix: string): ReactNode[] {
  const out: ReactNode[] = []
  const pattern = /(`[^`]+`|\*\*[^*]+\*\*|\*[^*\s][^*]*\*)/g
  let last = 0
  let n = 0
  for (const match of text.matchAll(pattern)) {
    const at = match.index ?? 0
    if (at > last) out.push(text.slice(last, at))
    const token = match[0]
    const key = `${keyPrefix}-${n++}`
    if (token.startsWith("`"))
      out.push(
        <code
          key={key}
          dir="ltr"
          className="bg-muted px-1 py-0.5 font-mono text-[0.92em]"
        >
          {token.slice(1, -1)}
        </code>
      )
    else if (token.startsWith("**"))
      out.push(<strong key={key}>{token.slice(2, -2)}</strong>)
    else out.push(<em key={key}>{token.slice(1, -1)}</em>)
    last = at + token.length
  }
  if (last < text.length) out.push(text.slice(last))
  return out
}

type Block = { kind: "p"; text: string } | { kind: "code"; text: string }

function blocks(source: string): Block[] {
  const result: Block[] = []
  const lines = source.split("\n")
  let paragraph: string[] = []
  let fence: string[] | null = null
  const flush = () => {
    if (paragraph.length) result.push({ kind: "p", text: paragraph.join(" ") })
    paragraph = []
  }
  for (const line of lines) {
    if (line.trimStart().startsWith("```")) {
      if (fence) {
        result.push({ kind: "code", text: fence.join("\n") })
        fence = null
      } else {
        flush()
        fence = []
      }
    } else if (fence) fence.push(line)
    else if (line.trim() === "") flush()
    else paragraph.push(line.trim())
  }
  if (fence) result.push({ kind: "code", text: fence.join("\n") })
  flush()
  return result
}

/** The small markdown subset cards use: paragraphs, fenced code, inline code, bold, italic. */
export function PromptMarkdown({
  source,
  className,
}: {
  source: string
  className?: string
}) {
  return (
    <div dir="auto" className={cn("flex flex-col gap-3", className)}>
      {blocks(source).map((block, i) =>
        block.kind === "code" ? (
          <pre
            key={i}
            dir="ltr"
            className="overflow-x-auto bg-muted p-3 font-mono text-[12px] leading-relaxed"
          >
            {block.text}
          </pre>
        ) : (
          <p key={i}>
            <Fragment>{inline(block.text, String(i))}</Fragment>
          </p>
        )
      )}
    </div>
  )
}
