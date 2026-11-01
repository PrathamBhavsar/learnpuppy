import { CompassIcon, KeyboardIcon, LifeBuoyIcon, PlayIcon } from "lucide-react"
import { useState } from "react"

import { ScreenGuide } from "@/components/app/info/info"
import {
  SHELL_GUIDE_PARTS,
  SHELL_INFO,
} from "@/components/app/info/shell-content"
import { useTour } from "@/components/app/tour/tour-store"
import { Button } from "@/components/ui/button"
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from "@/components/ui/dropdown-menu"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { useShortcutsSheet } from "@/stores/shortcuts-sheet"

/** Topbar Help button: replay the tour, explain the app frame, open the shortcuts sheet. */
export function HelpMenu() {
  const replay = useTour((s) => s.replay)
  const setShortcutsOpen = useShortcutsSheet((s) => s.setOpen)
  const [guideOpen, setGuideOpen] = useState(false)
  // Let the menu finish closing (and give focus back) before another layer opens.
  const later = (fn: () => void) => () => window.setTimeout(fn, 60)
  return (
    <>
      <DropdownMenu>
        <Tooltip>
          <TooltipTrigger asChild>
            <DropdownMenuTrigger asChild>
              <Button
                variant="ghost"
                size="icon"
                data-testid="help-menu"
                data-tour="help"
                aria-label="Help"
              >
                <LifeBuoyIcon />
              </Button>
            </DropdownMenuTrigger>
          </TooltipTrigger>
          <TooltipContent>{SHELL_INFO.help.short}</TooltipContent>
        </Tooltip>
        <DropdownMenuContent align="end">
          <DropdownMenuItem
            onSelect={later(replay)}
            data-testid="help-replay-tour"
          >
            <PlayIcon /> Replay the tour
          </DropdownMenuItem>
          <DropdownMenuItem
            onSelect={later(() => setGuideOpen(true))}
            data-testid="help-guide"
          >
            <CompassIcon /> What is everything here?
          </DropdownMenuItem>
          <DropdownMenuItem
            onSelect={later(() => setShortcutsOpen(true))}
            data-testid="help-shortcuts"
          >
            <KeyboardIcon /> Keyboard shortcuts
          </DropdownMenuItem>
        </DropdownMenuContent>
      </DropdownMenu>
      <ScreenGuide
        title="Finding your way around"
        summary="The frame around every screen: the sidebar on the left and the bar along the top."
        parts={SHELL_GUIDE_PARTS}
        open={guideOpen}
        onOpenChange={setGuideOpen}
      />
    </>
  )
}
