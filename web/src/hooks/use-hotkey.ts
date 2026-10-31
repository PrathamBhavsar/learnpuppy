import { useEffect, useRef, useState } from "react"

import {
  type Shortcut,
  isMac,
  matchesCombo,
  registerShortcut,
} from "@/lib/keys"

export { isMac, matchesCombo }

type Options = {
  enabled?: boolean
  /** Fire even while an input/textarea is focused (default: only with `mod`). */
  allowInInput?: boolean
  /** Set `label` to list the binding in the Shortcuts sheet. */
  label?: string
  group?: string
  id?: string
}

let anonymous = 0

/** Binds `combo` (`mod+k`, `[`, or a chord such as `g l`) while the component is mounted. */
export function useHotkey(
  combo: string,
  handler: (event: KeyboardEvent) => void,
  options: Options = {}
): void {
  const { enabled = true, allowInInput, label, group = "General", id } = options
  const handlerRef = useRef(handler)
  const enabledRef = useRef(enabled)
  const [fallbackId] = useState(() => `anon-${(anonymous += 1)}`)
  useEffect(() => {
    handlerRef.current = handler
    enabledRef.current = enabled
  })
  const shortcutId = id ?? fallbackId
  useEffect(() => {
    const shortcut: Shortcut = {
      id: shortcutId,
      keys: combo,
      label: label ?? "",
      group,
      hidden: label === undefined,
      allowInInput,
    }
    return registerShortcut(shortcut, handlerRef, enabledRef)
  }, [combo, shortcutId, label, group, allowInInput])
}

export type HotkeyBinding = {
  id: string
  keys: string
  handler: (event: KeyboardEvent) => void
  label?: string
  group?: string
  enabled?: boolean
  allowInInput?: boolean
}

/** Registers a dynamic list of bindings (e.g. one per workspace) from a single hook call. */
export function useHotkeys(bindings: HotkeyBinding[]): void {
  const latest = useRef(bindings)
  useEffect(() => {
    latest.current = bindings
  })
  const signature = bindings
    .map((b) =>
      [b.id, b.keys, b.label ?? "", b.group ?? "", b.allowInInput ?? ""].join(
        "\u0000"
      )
    )
    .join("\u0001")
  useEffect(() => {
    const disposers = latest.current.map((b, index) =>
      registerShortcut(
        {
          id: b.id,
          keys: b.keys,
          label: b.label ?? "",
          group: b.group ?? "General",
          hidden: b.label === undefined,
          allowInInput: b.allowInInput,
        },
        { current: (event) => latest.current[index]?.handler(event) },
        {
          get current() {
            return latest.current[index]?.enabled ?? true
          },
        }
      )
    )
    return () => disposers.forEach((dispose) => dispose())
  }, [signature])
}
