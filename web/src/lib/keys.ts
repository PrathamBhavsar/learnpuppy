/**
 * Global hotkey registry (docs/web/06 §Global shell). Every binding registers here, so the single
 * window listener can resolve vim-style chords (`g l`) and the Shortcuts sheet is generated from the
 * same data that drives behaviour.
 */

export const isMac =
  typeof navigator !== "undefined" &&
  /mac|iphone|ipad/i.test(navigator.platform)

/** How long after the first key of a chord the second key still counts. */
export const CHORD_WINDOW_MS = 800

export type Shortcut = {
  id: string
  /** `mod+k`, `[`, `shift+/`, or a chord of combos separated by a space: `g l`. */
  keys: string
  label: string
  /** Heading in the Shortcuts sheet. */
  group: string
  /** Hidden bindings still work but are not listed. */
  hidden?: boolean
  /** Fire even while an input/textarea is focused (default: only for `mod` combos). */
  allowInInput?: boolean
}

type Entry = Shortcut & {
  handler: { current: (event: KeyboardEvent) => void }
  enabled: { current: boolean }
}

export function isEditable(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false
  return (
    target.isContentEditable ||
    target.closest("input, textarea, select, [contenteditable='true']") !== null
  )
}

function inDialog(target: EventTarget | null): boolean {
  return (
    target instanceof Element &&
    target.closest(
      '[data-testid="info-dialog"], [data-testid="screen-guide"]'
    ) !== null
  )
}

/** `combo` is a key with optional `mod+` (⌘ on macOS, Ctrl elsewhere), `shift+` and `alt+` prefixes. */
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

const entries = new Map<string, Entry>()
const listeners = new Set<() => void>()
let snapshot: readonly Shortcut[] = []
let pending: { first: string; at: number } | null = null
let installed = false

function publish() {
  snapshot = [...entries.values()].map(
    ({ id, keys, label, group, hidden, allowInInput }) => ({
      id,
      keys,
      label,
      group,
      hidden,
      allowInInput,
    })
  )
  for (const listener of listeners) listener()
}

export function subscribeShortcuts(listener: () => void): () => void {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

export function getShortcuts(): readonly Shortcut[] {
  return snapshot
}

/** Registers a shortcut; returns the unregister function. Re-registering an id replaces it. */
export function registerShortcut(
  shortcut: Shortcut,
  handler: Entry["handler"],
  enabled: Entry["enabled"]
): () => void {
  const entry: Entry = { ...shortcut, handler, enabled }
  entries.set(shortcut.id, entry)
  install()
  publish()
  return () => {
    if (entries.get(shortcut.id) === entry) entries.delete(shortcut.id)
    publish()
  }
}

function allowed(entry: Entry, event: KeyboardEvent): boolean {
  if (!entry.enabled.current) return false
  const allowInInput = entry.allowInInput ?? entry.keys.includes("mod")
  // Plain keys belong to an open info dialog, never to the screen behind it.
  if (!entry.keys.includes("mod") && inDialog(event.target)) return false
  return allowInInput || !isEditable(event.target)
}

function fire(entry: Entry, event: KeyboardEvent) {
  event.preventDefault()
  entry.handler.current(event)
}

function onKeyDown(event: KeyboardEvent): void {
  if (event.repeat || event.defaultPrevented) return
  const now = Date.now()
  const chord = pending && now - pending.at <= CHORD_WINDOW_MS ? pending : null
  pending = null

  if (chord) {
    for (const entry of entries.values()) {
      const [first, second] = entry.keys.split(" ")
      if (second && first === chord.first && matchesCombo(event, second)) {
        if (allowed(entry, event)) return fire(entry, event)
      }
    }
  }

  let chordFirst: string | null = null
  for (const entry of entries.values()) {
    const [first, second] = entry.keys.split(" ")
    if (!first || !matchesCombo(event, first) || !allowed(entry, event))
      continue
    if (second) chordFirst = first
    else return fire(entry, event)
  }
  if (chordFirst) pending = { first: chordFirst, at: now }
}

function install() {
  if (installed || typeof window === "undefined") return
  installed = true
  window.addEventListener("keydown", onKeyDown)
}

/** Test seam: forget the pending chord prefix. */
export function resetChord(): void {
  pending = null
}

/** Display parts for a binding: `["g", "l"]` for a chord, `["mod", "k"]` for a combo (see `ShortcutKeys`). */
export function splitBinding(keys: string): string[][] {
  return keys.split(" ").map((combo) => combo.split("+"))
}
