import { useRouterState } from "@tanstack/react-router"

import { useGoto } from "@/hooks/use-goto"
import { type HotkeyBinding, useHotkeys } from "@/hooks/use-hotkey"
import { useActiveWorkspace } from "@/hooks/use-workspace"
import { isEditable } from "@/lib/keys"
import { pathForWorkspace } from "@/lib/nav"

const GO_TO = [
  ["g g", "graph", "Go to Graph"],
  ["g l", "learn", "Go to Learn"],
  ["g r", "review", "Go to Review"],
  ["g d", "digest", "Go to Digest"],
  ["g i", "ingest", "Go to Ingest"],
] as const

const SEARCH_SELECTOR = "[data-screen-search], input[type='search']"

/** The global bindings of docs/web/06 §Global shell; `⌘K`, `?` and `[` are owned by their widgets. */
export function useShellHotkeys(): void {
  const goto = useGoto()
  const { id: ws, workspaces } = useActiveWorkspace()
  const pathname = useRouterState({ select: (s) => s.location.pathname })

  const bindings: HotkeyBinding[] = GO_TO.map(([keys, section, label]) => ({
    id: `go-${section}`,
    keys,
    label,
    group: "Go to",
    enabled: Boolean(ws),
    handler: () => {
      if (ws) goto(`/w/${ws}/${section}`)
    },
  }))

  for (let n = 1; n <= 9; n += 1) {
    const target = workspaces[n - 1]
    bindings.push({
      id: `workspace-${n}`,
      keys: `mod+${n}`,
      label: target ? `Switch to ${target.name}` : undefined,
      group: "Workspaces",
      enabled: target !== undefined,
      handler: () => {
        if (target) goto(pathForWorkspace(target.id, pathname))
      },
    })
  }

  bindings.push(
    {
      id: "focus-search",
      keys: "/",
      label: "Focus the screen's search",
      group: "General",
      handler: () =>
        document.querySelector<HTMLElement>(SEARCH_SELECTOR)?.focus(),
    },
    {
      id: "blur",
      keys: "escape",
      label: "Close or blur",
      group: "General",
      allowInInput: true,
      handler: () => {
        const active = document.activeElement
        if (active instanceof HTMLElement && isEditable(active)) active.blur()
      },
    }
  )

  useHotkeys(bindings)
}
