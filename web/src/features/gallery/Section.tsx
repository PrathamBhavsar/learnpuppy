import type { ReactNode } from "react"

import { cn } from "@/lib/utils"

export function Section({
  id,
  title,
  children,
  className,
}: {
  id: string
  title: string
  children: ReactNode
  className?: string
}) {
  return (
    <section
      aria-labelledby={`gallery-${id}-title`}
      data-testid={`gallery-${id}`}
      className="flex flex-col gap-3"
    >
      <h2
        id={`gallery-${id}-title`}
        className="border-b pb-2 font-heading text-[16px] font-medium"
      >
        {title}
      </h2>
      <div className={cn("flex flex-wrap items-start gap-4", className)}>
        {children}
      </div>
    </section>
  )
}

export function Specimen({
  label,
  children,
  className,
}: {
  label: string
  children: ReactNode
  className?: string
}) {
  return (
    <div className="flex min-w-0 flex-col gap-2">
      <span className="text-[11px] text-muted-foreground">{label}</span>
      <div className={cn("flex flex-wrap items-center gap-2", className)}>
        {children}
      </div>
    </div>
  )
}
