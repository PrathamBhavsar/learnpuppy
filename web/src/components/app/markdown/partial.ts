/**
 * Close the Markdown a still-streaming message has left open, so half-written syntax never
 * flashes raw. Use it only on in-flight text; settled messages render as written.
 */
const FENCE = /^ {0,3}(`{3,}|~{3,})[^\n]*$/

/** Split at the last unclosed fence (or the end): `head` is complete, `open` is its closer. */
function splitFences(text: string): {
  tail: string
  head: string
  closer: string
} {
  const lines = text.split("\n")
  let open: { marker: string; at: number } | null = null
  let offset = 0
  let tailStart = 0
  for (const line of lines) {
    const m = FENCE.exec(line)
    if (m) {
      const run = m[1] ?? ""
      if (!open) open = { marker: run, at: offset }
      else if (
        run[0] === open.marker[0] &&
        run.length >= open.marker.length &&
        line.trim() === run
      ) {
        open = null
        tailStart = offset + line.length + 1
      }
    }
    offset += line.length + 1
  }
  if (open) return { head: text, tail: "", closer: open.marker }
  return {
    head: text.slice(0, tailStart),
    tail: text.slice(tailStart),
    closer: "",
  }
}

const mask = (s: string) => s.replace(/`[^`\n]*`/g, (c) => "x".repeat(c.length))
const count = (s: string, re: RegExp) => s.match(re)?.length ?? 0

function completeTail(tail: string): string {
  let t = tail
  // A link cut off before its closing paren keeps its text and drops the target.
  t = t.replace(/\[([^\]\n]*)\]\([^)\n]*$/, "$1")
  const lastLine = () => t.slice(t.lastIndexOf("\n") + 1)

  // An odd backtick run on the last line is an unclosed inline code span.
  const ticks = count(lastLine().replace(/``+/g, ""), /`/g)
  if (ticks % 2 === 1) t += "`"

  const masked = mask(t)
  // `$$`: close an odd display block; drop a bare opener with nothing after it.
  if (count(masked, /\$\$/g) % 2 === 1) {
    const at = masked.lastIndexOf("$$")
    t =
      t.slice(at + 2).trim() === ""
        ? t.slice(0, at)
        : t + (t.endsWith("\n") ? "" : "\n") + "$$"
  } else {
    const line = mask(lastLine()).replace(/\$\$/g, "")
    const dollars = count(line, /\$/g)
    const last = line.lastIndexOf("$")
    // Currency ("$5") and a lone trailing `$` are prose, not an opened formula.
    if (dollars % 2 === 1 && /[^\s\d$]/.test(line[last + 1] ?? "")) t += "$"
  }

  // `**` is counted within the last paragraph, outside code and math.
  const para = t.slice(t.lastIndexOf("\n\n") + 2)
  const plain = mask(para).replace(/\$\$[\s\S]*?\$\$|\$[^$\n]*\$/g, (c) =>
    "x".repeat(c.length)
  )
  if (count(plain, /\*\*/g) % 2 === 1) {
    t = /\*\*\s*$/.test(t) ? t.replace(/\*\*\s*$/, "") : t + "**"
  }
  return t
}

export function completePartial(text: string): string {
  const { head, tail, closer } = splitFences(text)
  if (closer) return text + (text.endsWith("\n") ? "" : "\n") + closer
  return head + completeTail(tail)
}
