import type DOMPurifyType from "dompurify"

type Mermaid = typeof import("mermaid").default

let loaded: Promise<{ mermaid: Mermaid; purify: typeof DOMPurifyType }> | null =
  null
let seq = 0

/** Loads mermaid and DOMPurify together, once, in the mermaid chunk. */
function load() {
  loaded ??= Promise.all([import("mermaid"), import("dompurify")]).then(
    ([m, p]) => ({ mermaid: m.default, purify: p.default })
  )
  return loaded
}

/** Strips scripts, event handlers, `javascript:` links and foreign content from a rendered SVG. */
function sanitizeSvg(svg: string, purify: typeof DOMPurifyType): string {
  return purify.sanitize(svg, {
    USE_PROFILES: { svg: true, svgFilters: true },
    FORBID_TAGS: ["script", "foreignObject", "iframe", "object", "embed"],
    FORBID_ATTR: ["onerror", "onload", "onclick"],
    ADD_ATTR: ["target"],
  })
}

/** Renders a diagram to sanitised SVG markup; rejects on a parse error. */
export async function renderMermaid(
  source: string,
  dark: boolean
): Promise<string> {
  const { mermaid, purify } = await load()
  mermaid.initialize({
    startOnLoad: false,
    securityLevel: "strict",
    htmlLabels: false,
    flowchart: { htmlLabels: false },
    theme: dark ? "dark" : "default",
  })
  const id = `lp-mermaid-${++seq}`
  try {
    const { svg } = await mermaid.render(id, source)
    return sanitizeSvg(svg, purify)
  } finally {
    // mermaid leaves a scratch node behind on a failed render
    document.getElementById(`d${id}`)?.remove()
  }
}

export async function sanitizeForTest(svg: string): Promise<string> {
  return sanitizeSvg(svg, (await load()).purify)
}
