import { useEffect, useRef, useSyncExternalStore } from "react"

import type { LucideIcon } from "lucide-react"

/** Palette groups in display order (docs/web/06 §Command palette). */
export const COMMAND_GROUPS = [
  "Navigate",
  "Workspaces",
  "Actions",
  "Appearance",
] as const

type CommandGroup = (typeof COMMAND_GROUPS)[number] | (string & {})

export type Command = {
  id: string
  group: CommandGroup
  label: string
  /** Extra search terms (matched by cmdk alongside the label). */
  keywords?: string[]
  icon?: LucideIcon
  /** Binding shown as a hint, e.g. `g l` or `mod+k`. */
  shortcut?: string
  run: () => void
}

type Source = { owner: string; commands: Command[] }

/**
 * Features contribute commands from their `contrib.ts` through `useContributeCommands`; the palette
 * only reads this registry, so features never import each other (docs/web/04 §Source layout).
 */
const sources = new Map<string, Source>()
const listeners = new Set<() => void>()
let snapshot: readonly Command[] = []

function publish() {
  snapshot = [...sources.values()].flatMap((s) => s.commands)
  for (const listener of listeners) listener()
}

function registerCommands(owner: string, commands: Command[]): () => void {
  const source: Source = { owner, commands }
  sources.set(owner, source)
  publish()
  return () => {
    if (sources.get(owner) === source) sources.delete(owner)
    publish()
  }
}

function subscribe(listener: () => void) {
  listeners.add(listener)
  return () => listeners.delete(listener)
}

function getCommands(): readonly Command[] {
  return snapshot
}

export function useCommands(): readonly Command[] {
  return useSyncExternalStore(subscribe, getCommands, getCommands)
}

/**
 * Registers `commands` for as long as the caller is mounted. `run` closures may capture fresh props:
 * the registry holds the latest list through a ref, re-registering only when ids/labels change.
 */
export function useContributeCommands(
  owner: string,
  commands: Command[]
): void {
  const latest = useRef(commands)
  useEffect(() => {
    latest.current = commands
  })
  const signature = commands
    .map(
      (c) => `${c.id}\u0000${c.group}\u0000${c.label}\u0000${c.shortcut ?? ""}`
    )
    .join("\u0001")
  useEffect(() => {
    const proxies: Command[] = latest.current.map((command, index) => ({
      ...command,
      run: () => latest.current[index]?.run(),
    }))
    return registerCommands(owner, proxies)
  }, [owner, signature])
}
