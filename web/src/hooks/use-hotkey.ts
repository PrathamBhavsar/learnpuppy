import { useEffect, useRef } from "react"

const isMac =
  typeof navigator !== "undefined" &&
  /mac|iphone|ipad/i.test(navigator.platform)

function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  return (
    target.isContentEditable ||
    target.closest("input, textarea, select, [contenteditable='true']") !== null
  )
}

/** `combo` is a key with optional `mod+` (⌘ on macOS, Ctrl elsewhere) and `shift+` prefixes, e.g. `[`, `mod+k`. */
export function matchesCombo(event: KeyboardEvent, combo: string): boolean {
  const parts = combo.toLowerCase().split("+")
  const key = parts[parts.length - 1]
  const wantMod = parts.includes("mod")
  const wantShift = parts.includes("shift")
  const mod = isMac ? event.metaKey : event.ctrlKey
  if (wantMod !== mod) return false
  if (event.altKey) return false
  if (!wantMod && (event.metaKey || event.ctrlKey)) return false
  if (wantShift && !event.shiftKey) return false
  return event.key.toLowerCase() === key
}

type Options = {
  enabled?: boolean
  /** Fire even while an input/textarea is focused (default: only with `mod`). */
  allowInInput?: boolean
}

export function useHotkey(
  combo: string,
  handler: (event: KeyboardEvent) => void,
  options: Options = {}
): void {
  const { enabled = true, allowInInput = combo.includes("mod") } = options
  const latest = useRef(handler)
  useEffect(() => {
    latest.current = handler
  })
  useEffect(() => {
    if (!enabled) return
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.repeat || event.defaultPrevented) return
      if (!allowInInput && isEditable(event.target)) return
      if (!matchesCombo(event, combo)) return
      event.preventDefault()
      latest.current(event)
    }
    window.addEventListener("keydown", onKeyDown)
    return () => window.removeEventListener("keydown", onKeyDown)
  }, [combo, enabled, allowInInput])
}

export { isMac }
