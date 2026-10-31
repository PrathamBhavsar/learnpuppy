// Commands the graph screen adds to the palette while it is mounted (docs/web/06 §Command palette).
import {
  EyeIcon,
  LocateFixedIcon,
  MapIcon,
  SearchIcon,
  Table2Icon,
  XIcon,
} from "lucide-react"

import { type Command, useContributeCommands } from "@/lib/commands"

export type GraphCommandApi = {
  fit: () => void
  toggleHulls: () => void
  toggleMinimap: () => void
  toggleTable: () => void
  focusSearch: () => void
  clearSelection: () => void
  selectionCount: number
}

export function useGraphCommands(api: GraphCommandApi): void {
  const commands: Command[] = [
    {
      id: "graph-fit",
      group: "Actions",
      label: "Fit graph to view",
      keywords: ["zoom", "center", "reset view"],
      icon: LocateFixedIcon,
      shortcut: "f",
      run: api.fit,
    },
    {
      id: "graph-search",
      group: "Actions",
      label: "Search propositions in the graph",
      keywords: ["find", "filter", "highlight"],
      icon: SearchIcon,
      shortcut: "/",
      run: api.focusSearch,
    },
    {
      id: "graph-toggle-hulls",
      group: "Actions",
      label: "Toggle cluster outlines",
      keywords: ["hulls", "clusters"],
      icon: EyeIcon,
      run: api.toggleHulls,
    },
    {
      id: "graph-toggle-minimap",
      group: "Actions",
      label: "Toggle minimap",
      keywords: ["overview"],
      icon: MapIcon,
      run: api.toggleMinimap,
    },
    {
      id: "graph-toggle-table",
      group: "Actions",
      label: "Toggle chain table",
      keywords: ["chains", "export", "csv"],
      icon: Table2Icon,
      run: api.toggleTable,
    },
  ]
  if (api.selectionCount > 0)
    commands.push({
      id: "graph-clear-selection",
      group: "Actions",
      label: "Clear graph selection",
      keywords: ["deselect"],
      icon: XIcon,
      shortcut: "escape",
      run: api.clearSelection,
    })
  useContributeCommands("graph", commands)
}
