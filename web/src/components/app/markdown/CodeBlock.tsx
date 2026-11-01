import { CheckIcon, CopyIcon } from "lucide-react"
import { useEffect, useState } from "react"

import { Button } from "@/components/ui/button"

import { highlight, resolveLanguage } from "./highlight"

type CodeBlockProps = { code: string; lang?: string }

/** Fenced code: plain text first, swapped for the highlighted markup once shiki has loaded the language. */
export function CodeBlock({ code, lang }: CodeBlockProps) {
  const [html, setHtml] = useState<{ key: string; markup: string } | null>(null)
  const [copied, setCopied] = useState(false)
  const key = `${lang ?? ""}\u0000${code}`

  useEffect(() => {
    if (!resolveLanguage(lang)) return
    let cancelled = false
    void highlight(code, lang)
      .then((markup) => {
        if (!cancelled && markup) setHtml({ key, markup })
      })
      .catch(() => {
        /* unhighlighted code stays readable */
      })
    return () => {
      cancelled = true
    }
  }, [code, lang, key])

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(code)
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1500)
    } catch {
      /* clipboard denied: nothing to confirm */
    }
  }

  return (
    <div
      dir="ltr"
      data-testid="code-block"
      className="group/code relative my-2 border bg-muted/40 text-[12px]"
    >
      {lang ? (
        <span className="absolute start-2 top-1 text-[11px] text-muted-foreground">
          {lang}
        </span>
      ) : null}
      <Button
        variant="ghost"
        size="icon-xs"
        className="absolute end-1 top-1 opacity-0 group-focus-within/code:opacity-100 group-hover/code:opacity-100"
        onClick={() => void copy()}
        aria-label="Copy code"
        data-testid="code-copy"
      >
        {copied ? <CheckIcon /> : <CopyIcon />}
      </Button>
      {html?.key === key ? (
        <div
          className="lp-shiki overflow-x-auto px-3 pt-6 pb-2"
          dangerouslySetInnerHTML={{ __html: html.markup }}
        />
      ) : (
        <pre className="overflow-x-auto px-3 pt-6 pb-2">
          <code>{code}</code>
        </pre>
      )}
    </div>
  )
}
