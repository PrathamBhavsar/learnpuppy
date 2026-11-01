import { type InfoContent, InfoTip } from "@/components/app/info/info"
import type { ReactNode } from "react"

type SectionProps = {
  id: string
  title: string
  description?: string
  children: ReactNode
  tone?: "default" | "danger"
  /** Adds an (i) next to the title that explains the card in plain words. */
  info?: InfoContent
}

export function Section({
  id,
  title,
  description,
  children,
  tone = "default",
  info,
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
        <div className="flex items-center gap-1.5">
          <h2
            id={`${id}-heading`}
            className="font-heading text-[15px] font-medium"
          >
            {title}
          </h2>
          {info ? <InfoTip info={info} testId={`info-${id}`} /> : null}
        </div>
        {description ? (
          <p className="text-[13px] text-muted-foreground">{description}</p>
        ) : null}
      </div>
      {children}
    </section>
  )
}
