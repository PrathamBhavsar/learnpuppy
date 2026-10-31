import { useSyncExternalStore } from "react"

import { ShortcutKeys } from "@/components/app/shortcut-keys"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import { useHotkey } from "@/hooks/use-hotkey"
import { type Shortcut, getShortcuts, subscribeShortcuts } from "@/lib/keys"
import { useShortcutsSheet } from "@/stores/shortcuts-sheet"

const GROUP_ORDER = ["General", "Go to", "Workspaces"]

function groupShortcuts(shortcuts: readonly Shortcut[]) {
  const visible = shortcuts.filter((s) => !s.hidden)
  const groups = [...new Set(visible.map((s) => s.group))].sort((a, b) => {
    const ia = GROUP_ORDER.indexOf(a)
    const ib = GROUP_ORDER.indexOf(b)
    return (ia === -1 ? 99 : ia) - (ib === -1 ? 99 : ib) || a.localeCompare(b)
  })
  return groups.map((group) => ({
    group,
    items: visible.filter((s) => s.group === group),
  }))
}

/** `?` sheet: generated from the hotkey registry, so it can never drift from what actually works. */
export function ShortcutsSheet() {
  const open = useShortcutsSheet((s) => s.open)
  const setOpen = useShortcutsSheet((s) => s.setOpen)
  const toggle = useShortcutsSheet((s) => s.toggle)
  const shortcuts = useSyncExternalStore(
    subscribeShortcuts,
    getShortcuts,
    getShortcuts
  )
  useHotkey("?", toggle, {
    id: "shortcuts",
    label: "Keyboard shortcuts",
    group: "General",
  })

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent
        data-testid="shortcuts-sheet"
        className="max-h-[80dvh] overflow-y-auto sm:max-w-lg"
      >
        <DialogHeader>
          <DialogTitle>Keyboard shortcuts</DialogTitle>
          <DialogDescription>Available anywhere in the app.</DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-4">
          {groupShortcuts(shortcuts).map(({ group, items }) => (
            <section
              key={group}
              aria-label={group}
              className="flex flex-col gap-1"
            >
              <h3 className="text-[12px] font-medium text-muted-foreground">
                {group}
              </h3>
              <ul className="flex flex-col" role="list">
                {items.map((s) => (
                  <li
                    key={s.id}
                    data-testid={`shortcut-${s.id}`}
                    className="flex items-center justify-between gap-4 py-1 text-[13px]"
                  >
                    <span>{s.label}</span>
                    <ShortcutKeys keys={s.keys} />
                  </li>
                ))}
              </ul>
            </section>
          ))}
        </div>
      </DialogContent>
    </Dialog>
  )
}
