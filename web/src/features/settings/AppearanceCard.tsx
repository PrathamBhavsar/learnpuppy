import { MonitorIcon, MoonIcon, SunIcon } from "lucide-react"

import { Switch } from "@/components/ui/switch"
import { ToggleGroup, ToggleGroupItem } from "@/components/ui/toggle-group"
import { useDirectionPref, useTheme } from "@/hooks/use-theme"
import { type Density, type Theme, isReadingFont, isTheme } from "@/lib/theme"
import { useUi } from "@/stores/ui"

import { SettingRow } from "./SettingRow"
import { useSettingsPatch } from "./preferences"

import "./settings.css"

const THEMES: { value: Theme; label: string; icon: typeof SunIcon }[] = [
  { value: "light", label: "Light", icon: SunIcon },
  { value: "dark", label: "Dark", icon: MoonIcon },
  { value: "system", label: "System", icon: MonitorIcon },
]

export function AppearanceCard() {
  const { theme, setTheme } = useTheme()
  const { direction, setDirection } = useDirectionPref()
  const reduceMotion = useUi((s) => s.reduceMotion)
  const setReduceMotion = useUi((s) => s.setReduceMotion)
  const density = useUi((s) => s.density)
  const setDensity = useUi((s) => s.setDensity)
  const readingFont = useUi((s) => s.readingFont)
  const setReadingFont = useUi((s) => s.setReadingFont)
  const legacyClassroom = useUi((s) => s.legacyClassroom)
  const setLegacyClassroom = useUi((s) => s.setLegacyClassroom)
  const { patch } = useSettingsPatch()

  return (
    <div data-testid="settings-appearance" className="flex flex-col divide-y">
      <SettingRow label="Theme" description="Follow the system or pick one.">
        <ToggleGroup
          type="single"
          variant="outline"
          size="sm"
          spacing={0}
          aria-label="Theme"
          value={theme}
          onValueChange={(value) => {
            if (!isTheme(value)) return
            setTheme(value)
            patch({ ui: { theme: value } }, "theme")
          }}
        >
          {THEMES.map(({ value, label, icon: Icon }) => (
            <ToggleGroupItem
              key={value}
              value={value}
              data-testid={`theme-${value}`}
            >
              <Icon className="lp-theme-icon" aria-hidden />
              {label}
            </ToggleGroupItem>
          ))}
        </ToggleGroup>
      </SettingRow>

      <SettingRow
        label="Direction"
        description="Mirror the layout for right-to-left languages."
      >
        <ToggleGroup
          type="single"
          variant="outline"
          size="sm"
          spacing={0}
          aria-label="Direction"
          value={direction}
          onValueChange={(value) => {
            if (value !== "ltr" && value !== "rtl") return
            setDirection(value)
            patch({ ui: { direction: value } }, "direction")
          }}
        >
          <ToggleGroupItem value="ltr" data-testid="direction-ltr">
            LTR
          </ToggleGroupItem>
          <ToggleGroupItem value="rtl" data-testid="direction-rtl">
            RTL
          </ToggleGroupItem>
        </ToggleGroup>
      </SettingRow>

      <SettingRow
        label="Reduce motion"
        description="Fade instead of move, whatever the system setting says."
        htmlFor="reduce-motion"
      >
        <Switch
          id="reduce-motion"
          checked={reduceMotion}
          data-testid="reduce-motion"
          onCheckedChange={(on) => {
            setReduceMotion(on)
            patch({ ui: { reduceMotion: on } }, "motion")
          }}
        />
      </SettingRow>

      <SettingRow
        label="Density"
        description="Compact tightens spacing across the app."
      >
        <ToggleGroup
          type="single"
          variant="outline"
          size="sm"
          spacing={0}
          aria-label="Density"
          value={density}
          onValueChange={(value) => {
            if (value !== "comfortable" && value !== "compact") return
            const next: Density = value
            setDensity(next)
            patch({ ui: { density: next } }, "density")
          }}
        >
          <ToggleGroupItem
            value="comfortable"
            data-testid="density-comfortable"
          >
            Comfortable
          </ToggleGroupItem>
          <ToggleGroupItem value="compact" data-testid="density-compact">
            Compact
          </ToggleGroupItem>
        </ToggleGroup>
      </SettingRow>

      <SettingRow
        label="Reading font"
        description="The font for lessons, chat and the digest. Menus and code stay mono."
      >
        <ToggleGroup
          type="single"
          variant="outline"
          size="sm"
          spacing={0}
          aria-label="Reading font"
          value={readingFont}
          onValueChange={(value) => {
            if (!isReadingFont(value)) return
            setReadingFont(value)
            patch({ ui: { readingFont: value } }, "reading font")
          }}
        >
          <ToggleGroupItem value="sans" data-testid="reading-font-sans">
            Sans
          </ToggleGroupItem>
          <ToggleGroupItem value="serif" data-testid="reading-font-serif">
            Serif
          </ToggleGroupItem>
          <ToggleGroupItem value="mono" data-testid="reading-font-mono">
            Mono
          </ToggleGroupItem>
        </ToggleGroup>
      </SettingRow>

      <SettingRow
        label="Legacy classroom"
        description="Advanced: show the old chat-based Learn page in the sidebar and command palette. Courses are the main way to learn."
      >
        <Switch
          aria-label="Legacy classroom"
          data-testid="legacy-classroom"
          checked={legacyClassroom}
          onCheckedChange={setLegacyClassroom}
        />
      </SettingRow>
    </div>
  )
}
