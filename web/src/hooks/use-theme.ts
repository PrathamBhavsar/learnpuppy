import { useUi } from "@/stores/ui"

export function useTheme() {
  const theme = useUi((s) => s.theme)
  const resolvedTheme = useUi((s) => s.resolvedTheme)
  const setTheme = useUi((s) => s.setTheme)
  return { theme, resolvedTheme, setTheme }
}

export function useDirectionPref() {
  const direction = useUi((s) => s.direction)
  const setDirection = useUi((s) => s.setDirection)
  return { direction, setDirection }
}
