/** Normalize explicit TeX delimiters outside Markdown code; bare brackets are ordinary prose. */
export function mathSource(source: string): string {
  return source.replace(
    /(`{3,}|~{3,})[^\n]*\n[\s\S]*?(?:\n\1[^\n]*(?=\n|$)|$)|(`+)[^`]*?\2|\\\(([\s\S]*?)\\\)|\\\[([\s\S]*?)\\\]|\[([+-]?(?:\d+(?:\.\d+)?|∞)\s*,\s*[+-]?(?:\d+(?:\.\d+)?|∞))\)/g,
    (token, fence, inline, expression, display, interval) => {
      if (fence || inline) return token
      // Otherwise a later `](url)` can turn an interval into a Markdown link.
      if (interval !== undefined) return `\\[${interval})`
      return display !== undefined
        ? `\n\n$$\n${display}\n$$\n\n`
        : `$${expression}$`
    }
  )
}
