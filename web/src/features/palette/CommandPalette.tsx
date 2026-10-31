import { useRouterState } from "@tanstack/react-router"
import { FileTextIcon, MessageSquareIcon } from "lucide-react"
import { Fragment, useMemo } from "react"

import { useChats } from "@/api/queries/chats"
import { useGlobalSearch } from "@/api/queries/workspaces"
import { ShortcutKeys } from "@/components/app/shortcut-keys"
import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
  CommandSeparator,
  CommandShortcut,
} from "@/components/ui/command"
import { useDebounced } from "@/hooks/use-debounced"
import { useGoto } from "@/hooks/use-goto"
import { useHotkey } from "@/hooks/use-hotkey"
import { useActiveWorkspace } from "@/hooks/use-workspace"
import {
  COMMAND_GROUPS,
  type Command as PaletteCommand,
  useCommands,
} from "@/lib/commands"
import { usePalette } from "@/stores/palette"

import { usePaletteCommands } from "./contrib"

const SEARCH_DEBOUNCE_MS = 150

function CommandRow({
  command,
  onRun,
  prefix = "",
}: {
  command: PaletteCommand
  onRun: (command: PaletteCommand) => void
  prefix?: string
}) {
  const Icon = command.icon
  return (
    <CommandItem
      value={`${prefix}${command.label}`}
      keywords={command.keywords}
      data-testid={`palette-cmd-${command.id}`}
      onSelect={() => onRun(command)}
    >
      {Icon ? <Icon aria-hidden /> : null}
      <span className="min-w-0 flex-1 truncate">{command.label}</span>
      {command.shortcut ? (
        <CommandShortcut>
          <ShortcutKeys keys={command.shortcut} />
        </CommandShortcut>
      ) : null}
    </CommandItem>
  )
}

/** Global ⌘K palette (docs/web/06): commands from the registry, live node search, recent chats. */
export function CommandPalette() {
  const open = usePalette((s) => s.open)
  const setOpen = usePalette((s) => s.setOpen)
  const toggle = usePalette((s) => s.toggle)
  const query = usePalette((s) => s.query)
  const setQuery = usePalette((s) => s.setQuery)
  const scope = usePalette((s) => s.scope)
  const setScope = usePalette((s) => s.setScope)
  const recents = usePalette((s) => s.recents)
  const remember = usePalette((s) => s.remember)

  const pathname = useRouterState({ select: (s) => s.location.pathname })
  const { id: ws, workspaces } = useActiveWorkspace()
  const goto = useGoto()
  const commands = useCommands()
  usePaletteCommands(pathname)
  useHotkey("mod+k", toggle, {
    id: "palette",
    label: "Command palette",
    group: "General",
  })

  const term = useDebounced(query.trim(), SEARCH_DEBOUNCE_MS)
  const search = useGlobalSearch(open ? term : "")
  const chats = useChats(ws ?? "", open && Boolean(ws))

  const run = (command: PaletteCommand) => {
    remember(command.id)
    setOpen(false)
    command.run()
  }

  const grouped = useMemo(() => {
    const order = [...COMMAND_GROUPS] as string[]
    for (const c of commands) if (!order.includes(c.group)) order.push(c.group)
    return order
      .map((group) => ({
        group,
        items: commands.filter((c) => c.group === group),
      }))
      .filter((g) => g.items.length > 0)
  }, [commands])

  const recentCommands = recents
    .map((id) => commands.find((c) => c.id === id))
    .filter((c): c is PaletteCommand => c !== undefined)
  const recentChats = [...(chats.data ?? [])]
    .sort((a, b) => b.updatedAt - a.updatedAt)
    .slice(0, 5)

  const hits = (search.data ?? []).filter(
    (hit) => scope === "all" || hit.workspaceId === ws
  )
  const hitsByWorkspace = [...new Set(hits.map((h) => h.workspaceId))].map(
    (id) => ({
      id,
      name: workspaces.find((w) => w.id === id)?.name ?? id,
      hits: hits.filter((h) => h.workspaceId === id),
    })
  )
  const searching =
    term.length > 0 && (search.isFetching || term !== query.trim())

  return (
    <CommandDialog
      open={open}
      onOpenChange={setOpen}
      title="Command palette"
      description="Search screens, actions and nodes"
    >
      <Command
        loop
        label="Command palette"
        onKeyDown={(event) => {
          if (event.key === "Tab" && !event.shiftKey) {
            event.preventDefault()
            setScope(scope === "workspace" ? "all" : "workspace")
          }
        }}
      >
        <CommandInput
          value={query}
          onValueChange={setQuery}
          placeholder="Type a command or search nodes…"
          data-testid="palette-input"
        />
        <CommandList data-testid="palette-list">
          <CommandEmpty>
            {searching ? "Searching…" : "No results."}
          </CommandEmpty>

          {query.trim() === "" && recentCommands.length > 0 ? (
            <>
              <CommandGroup heading="Recent" data-testid="palette-group-recent">
                {recentCommands.map((c) => (
                  <CommandRow
                    key={c.id}
                    command={c}
                    onRun={run}
                    prefix="recent · "
                  />
                ))}
              </CommandGroup>
              <CommandSeparator />
            </>
          ) : null}

          {term !== "" && hitsByWorkspace.length > 0
            ? hitsByWorkspace.map((group) => (
                <Fragment key={group.id}>
                  <CommandGroup
                    heading={
                      scope === "all" ? `Nodes · ${group.name}` : "Nodes"
                    }
                    data-testid="palette-group-nodes"
                  >
                    {group.hits.map((hit) => (
                      <CommandItem
                        key={`${hit.workspaceId}:${hit.node.id}`}
                        value={`node ${hit.workspaceId} ${hit.node.id}`}
                        forceMount
                        data-testid={`palette-node-${hit.node.id}`}
                        onSelect={() => {
                          setOpen(false)
                          goto(
                            `/w/${hit.workspaceId}/graph?node=${hit.node.id}`
                          )
                        }}
                      >
                        <FileTextIcon aria-hidden />
                        <span className="min-w-0 flex-1 truncate">
                          {hit.node.text}
                        </span>
                        <span className="text-[11px] text-muted-foreground">
                          {hit.node.kind}
                        </span>
                      </CommandItem>
                    ))}
                  </CommandGroup>
                  <CommandSeparator />
                </Fragment>
              ))
            : null}

          {grouped.map(({ group, items }) => (
            <CommandGroup
              key={group}
              heading={group}
              data-testid={`palette-group-${group.toLowerCase()}`}
            >
              {items.map((c) => (
                <CommandRow key={c.id} command={c} onRun={run} />
              ))}
            </CommandGroup>
          ))}

          {recentChats.length > 0 ? (
            <CommandGroup
              heading="Recent chats"
              data-testid="palette-group-chats"
            >
              {recentChats.map((chat) => (
                <CommandItem
                  key={chat.id}
                  value={`chat ${chat.title}`}
                  data-testid={`palette-chat-${chat.id}`}
                  onSelect={() => {
                    setOpen(false)
                    goto(`/w/${ws}/learn/${chat.id}`)
                  }}
                >
                  <MessageSquareIcon aria-hidden />
                  <span className="min-w-0 flex-1 truncate">{chat.title}</span>
                  <span className="text-[11px] text-muted-foreground">
                    {chat.phase}
                  </span>
                </CommandItem>
              ))}
            </CommandGroup>
          ) : null}
        </CommandList>
        <div
          className="flex items-center gap-3 border-t px-3 py-2 text-[11px] text-muted-foreground"
          data-testid="palette-footer"
        >
          <span>↑↓ move</span>
          <span>↵ run</span>
          <span data-testid="palette-scope">
            Nodes: {scope === "all" ? "all workspaces" : "this workspace"} · Tab
            to switch
          </span>
        </div>
      </Command>
    </CommandDialog>
  )
}
