import { useNavigate } from "@tanstack/react-router"
import { MonitorIcon, MoonIcon, SunIcon } from "lucide-react"

import {
  Command,
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command"
import { useHotkey } from "@/hooks/use-hotkey"
import { useTheme } from "@/hooks/use-theme"
import { usePalette } from "@/stores/palette"
import { useUi } from "@/stores/ui"

import { SECTIONS } from "@/lib/nav"
import { DEFAULT_WORKSPACE, findWorkspace } from "@/lib/static-workspaces"

/** Navigation + appearance commands. Feature-contributed commands arrive with the registry (F10). */
export function CommandPalette() {
  const open = usePalette((s) => s.open)
  const setOpen = usePalette((s) => s.setOpen)
  const toggle = usePalette((s) => s.toggle)
  const lastWorkspace = useUi((s) => s.lastWorkspace)
  const navigate = useNavigate()
  const { setTheme } = useTheme()
  const ws = (findWorkspace(lastWorkspace) ?? DEFAULT_WORKSPACE).id

  useHotkey("mod+k", toggle)

  const run = (action: () => void) => {
    setOpen(false)
    action()
  }

  return (
    <CommandDialog
      open={open}
      onOpenChange={setOpen}
      title="Command palette"
      description="Search screens and actions"
    >
      <Command>
        <CommandInput
          placeholder="Type a command or search…"
          data-testid="palette-input"
        />
        <CommandList>
          <CommandEmpty>No results.</CommandEmpty>
          <CommandGroup heading="Navigate">
            {SECTIONS.map(({ key, label, icon: Icon }) => (
              <CommandItem
                key={key}
                value={`go ${label}`}
                data-testid={`palette-go-${key}`}
                onSelect={() =>
                  run(
                    () => void navigate({ to: `/w/$ws/${key}`, params: { ws } })
                  )
                }
              >
                <Icon aria-hidden />
                {label}
              </CommandItem>
            ))}
            <CommandItem
              value="go accounts"
              onSelect={() => run(() => void navigate({ to: "/accounts" }))}
            >
              Accounts
            </CommandItem>
            <CommandItem
              value="go settings"
              onSelect={() => run(() => void navigate({ to: "/settings" }))}
            >
              Settings
            </CommandItem>
            <CommandItem
              value="go gallery"
              onSelect={() => run(() => void navigate({ to: "/gallery" }))}
            >
              Component gallery
            </CommandItem>
          </CommandGroup>
          <CommandGroup heading="Appearance">
            <CommandItem
              value="theme light"
              onSelect={() => run(() => setTheme("light"))}
            >
              <SunIcon aria-hidden />
              Light theme
            </CommandItem>
            <CommandItem
              value="theme dark"
              onSelect={() => run(() => setTheme("dark"))}
            >
              <MoonIcon aria-hidden />
              Dark theme
            </CommandItem>
            <CommandItem
              value="theme system"
              onSelect={() => run(() => setTheme("system"))}
            >
              <MonitorIcon aria-hidden />
              System theme
            </CommandItem>
          </CommandGroup>
        </CommandList>
      </Command>
    </CommandDialog>
  )
}
