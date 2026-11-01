import {
  LocateFixedIcon,
  MinusIcon,
  PlusIcon,
  RefreshCwIcon,
} from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { Spinner } from "@/components/ui/spinner"

import { copy } from "../copy"

type Props = {
  onZoomIn: () => void
  onZoomOut: () => void
  onFit: () => void
  onRelayout: () => void
  relayoutPending: boolean
}

function ControlButton({
  label,
  testId,
  onClick,
  disabled,
  children,
}: {
  label: string
  testId: string
  onClick: () => void
  disabled?: boolean
  children: React.ReactNode
}) {
  return (
    <Tooltip>
      <TooltipTrigger asChild>
        <Button
          variant="outline"
          size="icon-sm"
          aria-label={label}
          data-testid={testId}
          onClick={onClick}
          disabled={disabled}
          className="bg-background"
        >
          {children}
        </Button>
      </TooltipTrigger>
      <TooltipContent side="left">{label}</TooltipContent>
    </Tooltip>
  )
}

/** Zoom, fit and relayout: square lyra buttons at the bottom end of the canvas. */
export function Controls({
  onZoomIn,
  onZoomOut,
  onFit,
  onRelayout,
  relayoutPending,
}: Props) {
  return (
    <div
      role="group"
      aria-label="Graph view controls"
      className="pointer-events-auto flex flex-col gap-1"
    >
      <ControlButton
        label={copy.zoomIn}
        testId="graph-zoom-in"
        onClick={onZoomIn}
      >
        <PlusIcon />
      </ControlButton>
      <ControlButton
        label={copy.zoomOut}
        testId="graph-zoom-out"
        onClick={onZoomOut}
      >
        <MinusIcon />
      </ControlButton>
      <ControlButton label={copy.fit} testId="graph-fit" onClick={onFit}>
        <LocateFixedIcon />
      </ControlButton>
      <ControlButton
        label={copy.relayout}
        testId="graph-relayout"
        onClick={onRelayout}
        disabled={relayoutPending}
      >
        {relayoutPending ? <Spinner /> : <RefreshCwIcon />}
      </ControlButton>
    </div>
  )
}
