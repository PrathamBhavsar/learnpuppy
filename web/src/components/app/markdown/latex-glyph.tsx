import "katex/dist/katex.min.css"

import { useEffect, useState } from "react"

const cache = new Map<string, string>()
let loader: Promise<typeof import("katex")> | undefined

/** Renders a LaTeX snippet to HTML once and caches it; KaTeX loads lazily on first use. */
async function render(tex: string): Promise<string> {
  const hit = cache.get(tex)
  if (hit !== undefined) return hit
  loader ??= import("katex")
  const katex = (await loader).default
  const html = katex.renderToString(tex, {
    throwOnError: false,
    output: "html",
    strict: "ignore",
  })
  cache.set(tex, html)
  return html
}

type Props = {
  /** LaTeX source, e.g. `\land` or `\implies`. */
  tex: string
  /** Plain-text reading for assistive tech (also the pre-load fallback). */
  label: string
  /** Glyph shown until KaTeX has loaded. */
  fallback: string
  className?: string
}

/** A single math glyph. Screen readers get `label`; the KaTeX markup is hidden from them. */
export function LatexGlyph({ tex, label, fallback, className }: Props) {
  const [html, setHtml] = useState<string | undefined>(() => cache.get(tex))
  useEffect(() => {
    let live = true
    void render(tex).then((h) => live && setHtml(h))
    return () => {
      live = false
    }
  }, [tex])
  return (
    <span role="img" aria-label={label} className={className}>
      {html === undefined ? (
        <span aria-hidden>{fallback}</span>
      ) : (
        <span aria-hidden dangerouslySetInnerHTML={{ __html: html }} />
      )}
    </span>
  )
}
