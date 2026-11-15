import type { LucideIcon } from "lucide-react"
import type { ReactNode } from "react"

import { cn } from "@/lib/utils"

type EventCardProps = {
  kind: "quiz" | "job" | "visual" | "steer" | "probe"
  icon: LucideIcon
  label: string
  /** One line; truncated, full text in the title. */
  summary: string
  status?: string
  trailing?: ReactNode
  children?: ReactNode
  className?: string
  testId?: string
}

/** The shared compact card: icon, kind label, one-line summary. Focusable so keyboard users can reach it. */
export function EventCard({
  kind,
  icon: Icon,
  label,
  summary,
  status,
  trailing,
  children,
  className,
  testId,
}: EventCardProps) {
  return (
    <div
      // Focusable on purpose: a card is a unit the keyboard can land on and read.
      // eslint-disable-next-line jsx-a11y/no-noninteractive-tabindex
      tabIndex={0}
      role="group"
      aria-label={`${label}: ${summary}`}
      data-testid={testId ?? `card-${kind}`}
      data-kind={kind}
      data-status={status}
      className={cn(
        "flex flex-col gap-2 border p-2.5 outline-none focus-visible:ring-1 focus-visible:ring-ring/50",
        className
      )}
    >
      <div className="flex items-start gap-2">
        <Icon
          aria-hidden
          className="mt-0.5 size-3.5 shrink-0 text-muted-foreground"
        />
        <div className="min-w-0 flex-1">
          <p className="truncate text-[13px]" title={summary}>
            {summary}
          </p>
          <p className="text-[11px] text-muted-foreground tabular-nums">
            {label}
          </p>
        </div>
        {trailing}
      </div>
      {children}
    </div>
  )
}
