import { useSyncExternalStore } from "react"

import { ShortcutKeys } from "@/components/app/shortcut-keys"
import { type Shortcut, getShortcuts, subscribeShortcuts } from "@/lib/keys"

function group(shortcuts: readonly Shortcut[]) {
  const visible = shortcuts.filter((s) => !s.hidden)
  const order = ["General", "Go to", "Workspaces"]
  const names = [...new Set(visible.map((s) => s.group))].sort((a, b) => {
    const ia = order.indexOf(a)
    const ib = order.indexOf(b)
    return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib) || a.localeCompare(b)
  })
  return names.map((name) => ({
    name,
    items: visible.filter((s) => s.group === name),
  }))
}

/** Read-only list generated from the hotkey registry, so it always matches what is bound. */
export function ShortcutsTable() {
  const shortcuts = useSyncExternalStore(
    subscribeShortcuts,
    getShortcuts,
    getShortcuts
  )
  const groups = group(shortcuts)
  return (
    <div data-testid="settings-shortcuts" className="grid gap-6 sm:grid-cols-2">
      {groups.map(({ name, items }) => (
        <section key={name} aria-label={name} className="flex flex-col gap-1">
          <h3 className="text-[12px] font-medium text-muted-foreground">
            {name}
          </h3>
          <ul role="list" className="flex flex-col">
            {items.map((s) => (
              <li
                key={s.id}
                data-testid={`settings-shortcut-${s.id}`}
                className="flex items-center justify-between gap-4 border-b py-1.5 text-[13px] last:border-b-0"
              >
                <span>{s.label}</span>
                <ShortcutKeys keys={s.keys} />
              </li>
            ))}
          </ul>
        </section>
      ))}
    </div>
  )
}
