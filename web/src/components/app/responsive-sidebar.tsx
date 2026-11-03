import { PanelLeftOpenIcon } from "lucide-react"
import {
  cloneElement,
  useEffect,
  useState,
  useSyncExternalStore,
  type ReactElement,
} from "react"

import { Button } from "@/components/ui/button"
import {
  Sheet,
  SheetContent,
  SheetDescription,
  SheetTitle,
  SheetTrigger,
} from "@/components/ui/sheet"
import { useHotkey } from "@/hooks/use-hotkey"
import { useUi } from "@/stores/ui"

const query = "(max-width: 927px)"
const subscribe = (notify: () => void) => {
  const media = window.matchMedia(query)
  media.addEventListener("change", notify)
  return () => media.removeEventListener("change", notify)
}
const snapshot = () => window.matchMedia(query).matches

/** Radix owns the narrow-screen overlay, focus trap, Escape and focus restoration. */
export function ResponsiveSidebar({
  children,
}: {
  children: ReactElement<{ collapsed: boolean; onToggle: () => void }>
}) {
  const narrow = useSyncExternalStore(subscribe, snapshot, () => false)
  const [open, setOpen] = useState(false)
  const toggle = useUi((state) => state.toggleSidebar)
  const direction = useUi((state) => state.direction)
  useHotkey("mod+b", () => (narrow ? setOpen((value) => !value) : toggle()), {
    id: "sidebar-mod",
    label: "Toggle sidebar",
    group: "General",
  })
  useEffect(() => {
    if (!narrow) return
    // Route links close the drawer after selection without changing desktop preferences.
    const close = (event: MouseEvent) => {
      if (
        event.target instanceof Element &&
        event.target.closest('[data-testid="sidebar"] a')
      )
        setOpen(false)
    }
    document.addEventListener("click", close)
    return () => document.removeEventListener("click", close)
  }, [narrow])
  if (!narrow) return children
  return (
    <Sheet open={open} onOpenChange={setOpen}>
      <div className="shrink-0 border-e bg-sidebar p-1">
        <SheetTrigger asChild>
          <Button
            size="icon-sm"
            variant="ghost"
            aria-label="Open sidebar"
            data-testid="sidebar-drawer-toggle"
          >
            <PanelLeftOpenIcon className="rtl:-scale-x-100" />
          </Button>
        </SheetTrigger>
      </div>
      <SheetContent
        side={direction === "rtl" ? "right" : "left"}
        className="w-64 gap-0 p-0 [&>aside]:h-full"
        showCloseButton={false}
      >
        <SheetTitle className="sr-only">Navigation</SheetTitle>
        <SheetDescription className="sr-only">
          Workspace and app navigation
        </SheetDescription>
        {cloneElement(children, {
          collapsed: false,
          onToggle: () => setOpen(false),
        })}
      </SheetContent>
    </Sheet>
  )
}
