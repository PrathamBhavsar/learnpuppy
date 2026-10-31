import { Kbd as KbdKey, KbdGroup } from "@/components/ui/kbd"
import { isMac } from "@/hooks/use-hotkey"

const GLYPHS: Record<string, [mac: string, other: string]> = {
  mod: ["⌘", "Ctrl"],
  shift: ["⇧", "Shift"],
  alt: ["⌥", "Alt"],
  enter: ["↵", "Enter"],
  esc: ["Esc", "Esc"],
  escape: ["Esc", "Esc"],
}

function glyph(key: string): string {
  const entry = GLYPHS[key.toLowerCase()]
  if (!entry) return key.length === 1 ? key.toUpperCase() : key
  return isMac ? entry[0] : entry[1]
}

/** Renders a shortcut such as `mod+k` as platform-correct key caps. */
export function Kbd({
  combo,
  className,
}: {
  combo: string
  className?: string
}) {
  return (
    <KbdGroup dir="ltr" className={className}>
      {combo.split("+").map((key) => (
        <KbdKey key={key}>{glyph(key)}</KbdKey>
      ))}
    </KbdGroup>
  )
}
