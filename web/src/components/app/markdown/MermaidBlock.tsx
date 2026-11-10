import { CheckIcon, CopyIcon } from "lucide-react"
import { useEffect, useState } from "react"

import { Button } from "@/components/ui/button"

import "./mermaid.css"
import { renderMermaid } from "./mermaid-render"

type State =
  | { key: string; svg: string; error?: undefined }
  | { key: string; error: string; svg?: undefined }

function useIsDark() {
  const [dark, setDark] = useState(() =>
    document.documentElement.classList.contains("dark")
  )
  useEffect(() => {
    const obs = new MutationObserver(() =>
      setDark(document.documentElement.classList.contains("dark"))
    )
    obs.observe(document.documentElement, {
      attributes: true,
      attributeFilter: ["class"],
    })
    return () => obs.disconnect()
  }, [])
  return dark
}

/** A ```mermaid fence: lazy-loaded SVG, or an error card with the source. */
export function MermaidBlock({ code }: { code: string }) {
  const dark = useIsDark()
  const [state, setState] = useState<State | null>(null)
  const [copied, setCopied] = useState(false)
  const key = `${dark}\u0000${code}`

  useEffect(() => {
    let cancelled = false
    renderMermaid(code, dark)
      .then((svg) => !cancelled && setState({ key, svg }))
      .catch((e: unknown) => {
        if (cancelled) return
        const error = e instanceof Error ? e.message : String(e)
        setState({ key, error })
      })
    return () => {
      cancelled = true
    }
  }, [code, dark, key])

  const copy = async () => {
    try {
      await navigator.clipboard.writeText(code)
      setCopied(true)
      window.setTimeout(() => setCopied(false), 1500)
    } catch {
      /* clipboard denied */
    }
  }

  const current = state?.key === key ? state : null

  if (current?.error !== undefined) {
    return (
      <div
        dir="ltr"
        role="alert"
        data-testid="mermaid-error"
        className="relative my-2 border border-destructive/50 bg-muted/40 text-[12px]"
      >
        <p className="px-3 pt-2 text-destructive">
          Diagram could not be rendered
        </p>
        <Button
          variant="ghost"
          size="icon-xs"
          className="absolute end-1 top-1"
          onClick={() => void copy()}
          aria-label="Copy diagram source"
          data-testid="mermaid-copy"
        >
          {copied ? <CheckIcon /> : <CopyIcon />}
        </Button>
        <pre className="overflow-x-auto px-3 pt-1 pb-2">
          <code>{code}</code>
        </pre>
      </div>
    )
  }

  return (
    <figure
      dir="ltr"
      data-testid="mermaid-block"
      className="my-2 border bg-muted/20"
    >
      {current?.svg !== undefined ? (
        <div
          className="lp-mermaid p-2"
          role="img"
          aria-label={`Diagram: ${code}`}
          dangerouslySetInnerHTML={{ __html: current.svg }}
        />
      ) : (
        <pre className="lp-mermaid overflow-x-auto p-2 text-[12px]">
          <code>{code}</code>
        </pre>
      )}
      <figcaption className="sr-only">{code}</figcaption>
    </figure>
  )
}
