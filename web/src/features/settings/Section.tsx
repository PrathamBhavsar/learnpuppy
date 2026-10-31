import type { ReactNode } from "react"

type SectionProps = {
  id: string
  title: string
  description?: string
  children: ReactNode
  tone?: "default" | "danger"
}

export function Section({
  id,
  title,
  description,
  children,
  tone = "default",
}: SectionProps) {
  return (
    <section
      aria-labelledby={`${id}-heading`}
      data-testid={`section-${id}`}
      className={
        tone === "danger"
          ? "flex flex-col gap-3 border border-destructive/50 p-4"
          : "flex flex-col gap-3 border p-4"
      }
    >
      <div className="flex flex-col gap-1">
        <h2
          id={`${id}-heading`}
          className="font-heading text-[15px] font-medium"
        >
          {title}
        </h2>
        {description ? (
          <p className="text-[13px] text-muted-foreground">{description}</p>
        ) : null}
      </div>
      {children}
    </section>
  )
}
