import { MonitorIcon, MoonIcon, SunIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuRadioGroup,
  DropdownMenuRadioItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import { useTheme } from "@/hooks/use-theme"
import { THEMES, type Theme, isTheme } from "@/lib/theme"

const LABELS: Record<Theme, string> = {
  light: "Light",
  dark: "Dark",
  system: "System",
}
const ICONS = { light: SunIcon, dark: MoonIcon, system: MonitorIcon } as const

/** Light / dark / system menu; the trigger icon morphs sun <-> moon (rotate + fade, 220 ms). */
export function ThemeToggle() {
  const { theme, setTheme } = useTheme()
  return (
    <DropdownMenu>
      <DropdownMenuTrigger asChild>
        <Button
          variant="ghost"
          size="icon"
          data-testid="theme-toggle"
          aria-label={`Theme: ${LABELS[theme]}`}
          className="relative"
        >
          <SunIcon className="scale-100 rotate-0 transition-[transform,opacity] duration-[var(--dur-base)] dark:scale-0 dark:-rotate-90" />
          <MoonIcon className="absolute scale-0 rotate-90 transition-[transform,opacity] duration-[var(--dur-base)] dark:scale-100 dark:rotate-0" />
        </Button>
      </DropdownMenuTrigger>
      <DropdownMenuContent align="end">
        <DropdownMenuRadioGroup
          value={theme}
          onValueChange={(v) => isTheme(v) && setTheme(v)}
        >
          {THEMES.map((t) => {
            const Icon = ICONS[t]
            return (
              <DropdownMenuRadioItem
                key={t}
                value={t}
                data-testid={`theme-option-${t}`}
              >
                <Icon aria-hidden />
                {LABELS[t]}
              </DropdownMenuRadioItem>
            )
          })}
        </DropdownMenuRadioGroup>
      </DropdownMenuContent>
    </DropdownMenu>
  )
}
