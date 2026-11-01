import { CircleHelpIcon, InfoIcon } from "lucide-react"
import { type MouseEvent, type ReactNode, useState } from "react"

import { Button } from "@/components/ui/button"
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog"
import {
  Tooltip,
  TooltipContent,
  TooltipTrigger,
} from "@/components/ui/tooltip"
import { cn } from "@/lib/utils"

/** What one part of the UI is and does, in plain words for a first-time user. */
export type InfoContent = {
  title: string
  /** One sentence: shown on hover/focus and at the top of the dialog. */
  short: string
  /** Longer explanation, one paragraph per entry. */
  body?: string[]
  /** "Good to know" bullets: shortcuts, limits, what happens next. */
  tips?: string[]
}

function InfoBody({ info }: { info: InfoContent }) {
  return (
    <div className="flex flex-col gap-3 text-[13px]" data-testid="info-body">
      {info.body?.map((p) => (
        <p key={p}>{p}</p>
      ))}
      {info.tips?.length ? (
        <div className="flex flex-col gap-1.5 border-s-2 border-border ps-3">
          <span className="font-medium">Good to know</span>
          <ul className="flex list-disc flex-col gap-1 ps-4 text-muted-foreground">
            {info.tips.map((t) => (
              <li key={t}>{t}</li>
            ))}
          </ul>
        </div>
      ) : null}
    </div>
  )
}

/**
 * A small ⓘ next to a label or control. Hover or focus shows the one-line explanation; a tap, click or
 * Enter opens a dialog with the full one. It never triggers the control it sits beside.
 */
export function InfoTip({
  info,
  className,
  testId,
}: {
  info: InfoContent
  className?: string
  testId?: string
}) {
  const [open, setOpen] = useState(false)
  const id = testId ?? `info-${info.title.toLowerCase().replace(/\W+/g, "-")}`
  const openDialog = (e: MouseEvent) => {
    e.preventDefault()
    e.stopPropagation()
    setOpen(true)
  }
  return (
    <>
      <Tooltip>
        <TooltipTrigger asChild>
          <button
            type="button"
            aria-label={`About ${info.title}`}
            data-testid={id}
            onClick={openDialog}
            className={cn(
              "inline-flex size-4 shrink-0 items-center justify-center text-muted-foreground outline-none hover:text-foreground focus-visible:ring-1 focus-visible:ring-ring",
              className
            )}
          >
            <InfoIcon className="size-3.5" aria-hidden />
          </button>
        </TooltipTrigger>
        <TooltipContent className="max-w-64">{info.short}</TooltipContent>
      </Tooltip>
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent data-testid="info-dialog">
          <DialogHeader>
            <DialogTitle>{info.title}</DialogTitle>
            <DialogDescription>{info.short}</DialogDescription>
          </DialogHeader>
          <InfoBody info={info} />
        </DialogContent>
      </Dialog>
    </>
  )
}

/** "About this screen": an overview plus every part of the screen explained, opened from the header. */
export function ScreenGuide({
  title,
  summary,
  parts,
  trigger,
  open: controlledOpen,
  onOpenChange,
}: {
  title: string
  summary: string
  parts: InfoContent[]
  trigger?: ReactNode
  /** Controlled mode (opened from a menu): no header button is rendered. */
  open?: boolean
  onOpenChange?: (open: boolean) => void
}) {
  const [innerOpen, setInnerOpen] = useState(false)
  const controlled = controlledOpen !== undefined
  const open = controlled ? controlledOpen : innerOpen
  const setOpen = controlled ? (onOpenChange ?? (() => {})) : setInnerOpen
  return (
    <>
      {controlled
        ? null
        : (trigger ?? (
            <Button
              variant="ghost"
              size="sm"
              onClick={() => setOpen(true)}
              data-testid="screen-guide-open"
              aria-label={`About this screen: ${title}`}
            >
              <CircleHelpIcon data-icon="inline-start" />
              About this screen
            </Button>
          ))}
      <Dialog open={open} onOpenChange={setOpen}>
        <DialogContent
          data-testid="screen-guide"
          className="max-h-[85dvh] overflow-y-auto sm:max-w-xl"
        >
          <DialogHeader>
            <DialogTitle>{title}</DialogTitle>
            <DialogDescription>{summary}</DialogDescription>
          </DialogHeader>
          <ul className="flex flex-col gap-4">
            {parts.map((p) => (
              <li key={p.title} className="flex flex-col gap-1">
                <span className="text-[13px] font-medium">{p.title}</span>
                <span className="text-[13px] text-muted-foreground">
                  {p.short}
                </span>
                <InfoBody info={{ ...p, short: "" }} />
              </li>
            ))}
          </ul>
        </DialogContent>
      </Dialog>
    </>
  )
}
