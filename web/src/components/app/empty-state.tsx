import type { LucideIcon } from "lucide-react"
import { type ReactNode, useEffect, useRef } from "react"

import { cn } from "@/lib/utils"

type EmptyStateProps = {
  icon: LucideIcon
  title: string
  description?: string
  action?: ReactNode
  /** Move focus to the call to action after 300 ms (only if nothing else holds focus). Default true. */
  focusAction?: boolean
  className?: string
}

export function EmptyState({
  icon: Icon,
  title,
  description,
  action,
  focusAction = true,
  className,
}: EmptyStateProps) {
  const actionRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    if (!focusAction || !action) return
    const timer = window.setTimeout(() => {
      const active = document.activeElement
      const free =
        !active ||
        active === document.body ||
        active.hasAttribute("data-page-title")
      if (free)
        actionRef.current
          ?.querySelector<HTMLElement>("button, a[href]")
          ?.focus()
    }, 300)
    return () => window.clearTimeout(timer)
  }, [focusAction, action])

  return (
    <div
      role="status"
      data-testid="empty-state"
      className={cn(
        "flex flex-col items-center justify-center gap-3 px-6 py-16 text-center",
        className
      )}
    >
      <Icon
        aria-hidden
        className="lp-float-in size-8 text-muted-foreground"
        strokeWidth={1.5}
      />
      <div className="flex max-w-sm flex-col gap-1">
        <h2 className="font-heading text-[16px] font-medium">{title}</h2>
        {description ? (
          <p className="text-[13px] text-muted-foreground">{description}</p>
        ) : null}
      </div>
      {action ? (
        <div ref={actionRef} className="mt-1">
          {action}
        </div>
      ) : null}
    </div>
  )
}
