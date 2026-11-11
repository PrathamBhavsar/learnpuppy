import {
  CompassIcon,
  FolderPlusIcon,
  LayersIcon,
  MessageSquarePlusIcon,
  MonitorIcon,
  MoonIcon,
  PlayIcon,
  UploadIcon,
  RefreshCwIcon,
  SettingsIcon,
  SunIcon,
  SunMoonIcon,
  Trash2Icon,
  UsersRoundIcon,
} from "lucide-react"

import { useRelayout } from "@/api/queries/graph"
import { useActiveWorkspace } from "@/hooks/use-workspace"
import { useGoto } from "@/hooks/use-goto"
import { useTheme } from "@/hooks/use-theme"
import { type Command, useContributeCommands } from "@/lib/commands"
import { SECTIONS } from "@/lib/nav"
import { pathForWorkspace } from "@/lib/nav"
import { toastError, toastSuccess } from "@/lib/toast"
import { workspaceIcon } from "@/lib/workspace-visual"

const CHORDS: Record<string, string> = {
  graph: "g g",
  learn: "g l",
  review: "g r",
  digest: "g d",
  ingest: "g i",
}

/** The palette's built-in commands: navigation, workspace switching, actions, appearance. */
export function usePaletteCommands(currentPath: string): void {
  const { id: ws, workspaces } = useActiveWorkspace()
  const goto = useGoto()
  const { theme, resolvedTheme, setTheme } = useTheme()
  const relayout = useRelayout(ws ?? "")

  const commands: Command[] = []

  if (ws) {
    for (const { key, label, icon } of SECTIONS) {
      commands.push({
        id: `go-${key}`,
        group: "Navigate",
        label: `Go to ${label}`,
        keywords: [label, "open", "navigate"],
        icon,
        shortcut: CHORDS[key],
        run: () => goto(`/w/${ws}/${key}`),
      })
    }
    commands.push({
      id: "go-workspace-settings",
      group: "Navigate",
      label: "Go to Workspace settings",
      keywords: ["settings", "workspace", "preferences"],
      icon: SettingsIcon,
      run: () => goto(`/w/${ws}/settings`),
    })
  }
  commands.push(
    {
      id: "go-home",
      group: "Navigate",
      label: "All workspaces",
      keywords: ["home", "picker", "manage"],
      icon: CompassIcon,
      run: () => goto("/"),
    },
    {
      id: "go-accounts",
      group: "Navigate",
      label: "Go to Accounts",
      keywords: ["claude", "codex", "models", "login"],
      icon: UsersRoundIcon,
      run: () => goto("/accounts"),
    },
    {
      id: "go-settings",
      group: "Navigate",
      label: "Go to Settings",
      keywords: ["preferences", "appearance"],
      icon: SettingsIcon,
      run: () => goto("/settings"),
    },
    {
      id: "go-trash",
      group: "Navigate",
      label: "Go to Trash",
      keywords: ["deleted", "restore"],
      icon: Trash2Icon,
      run: () => goto("/trash"),
    },
    {
      id: "go-review-all",
      group: "Navigate",
      label: "Review all workspaces",
      keywords: ["due", "cards"],
      icon: LayersIcon,
      run: () => goto("/review-all"),
    }
  )

  workspaces.forEach((w, index) => {
    commands.push({
      id: `switch-${w.id}`,
      group: "Workspaces",
      label: `Switch to ${w.name}`,
      keywords: ["workspace", "switch", w.slug],
      icon: workspaceIcon(w.icon),
      shortcut: index < 9 ? `mod+${index + 1}` : undefined,
      run: () => goto(pathForWorkspace(w.id, currentPath)),
    })
  })
  commands.push({
    id: "new-workspace",
    group: "Workspaces",
    label: "New workspace",
    keywords: ["create", "add"],
    icon: FolderPlusIcon,
    run: () => goto("/?new=1"),
  })

  if (ws) {
    commands.push(
      {
        id: "action-new-chat",
        group: "Actions",
        label: "New chat",
        keywords: ["learn", "teach", "practice", "start"],
        icon: MessageSquarePlusIcon,
        run: () => goto(`/w/${ws}/learn`),
      },
      {
        id: "action-run-ingest",
        group: "Actions",
        label: "Run ingest",
        keywords: ["sessions", "transcripts", "import"],
        icon: PlayIcon,
        run: () => goto(`/w/${ws}/ingest`),
      },
      {
        id: "action-start-review",
        group: "Actions",
        label: "Start review",
        keywords: ["cards", "due", "spaced repetition"],
        icon: LayersIcon,
        run: () => goto(`/w/${ws}/review`),
      },
      {
        id: "action-upload-textbook",
        group: "Actions",
        label: "Upload a textbook",
        keywords: ["pdf", "book", "chapter", "reading check", "textbooks"],
        icon: UploadIcon,
        run: () => goto(`/w/${ws}/textbook?upload=1`),
      },
      {
        id: "action-relayout",
        group: "Actions",
        label: "Relayout graph",
        keywords: ["layout", "forceatlas", "recompute"],
        icon: RefreshCwIcon,
        run: () =>
          relayout.mutate(undefined, {
            onSuccess: () => toastSuccess("Graph layout recomputed"),
            onError: (error) =>
              toastError(error, "Could not relayout the graph"),
          }),
      }
    )
  }
  commands.push(
    {
      id: "action-toggle-theme",
      group: "Actions",
      label: "Toggle theme",
      keywords: ["dark", "light", "appearance"],
      icon: SunMoonIcon,
      run: () => setTheme(resolvedTheme === "dark" ? "light" : "dark"),
    },
    {
      id: "theme-light",
      group: "Appearance",
      label: "Light theme",
      icon: SunIcon,
      run: () => setTheme("light"),
    },
    {
      id: "theme-dark",
      group: "Appearance",
      label: "Dark theme",
      icon: MoonIcon,
      run: () => setTheme("dark"),
    },
    {
      id: "theme-system",
      group: "Appearance",
      label: theme === "system" ? "System theme (current)" : "System theme",
      icon: MonitorIcon,
      run: () => setTheme("system"),
    }
  )

  useContributeCommands("palette", commands)
}
