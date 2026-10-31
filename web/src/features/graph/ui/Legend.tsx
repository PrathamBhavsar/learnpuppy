import { ChevronDownIcon } from "lucide-react"
import { useState } from "react"

import { Button } from "@/components/ui/button"
import { cn } from "@/lib/utils"

import { copy } from "../copy"
import { SOURCES } from "../model/scene"

import { SOURCE_BG } from "./source-style"

const KIND_SWATCH: Record<"claim" | "concept" | "procedure" | "decision", string> = {
  claim: "bg-muted-foreground",
  concept: "border border-muted-foreground bg-muted-foreground/20",
  procedure:
    "bg-muted-foreground after:absolute after:inset-[3px] after:bg-background",
  decision: "bg-muted-foreground outline outline-1 outline-offset-1 outline-muted-foreground/70",
}

export function Legend() {
  const [open, setOpen] = useState(true)
  return (
    <section
      aria-label={copy.legend.title}
      data-testid="graph-legend"
      className="pointer-events-auto w-[168px] border border-border bg-background/90 text-[11px] backdrop-blur-sm"
    >
      <Button
        variant="ghost"
        size="xs"
        aria-expanded={open}
        onClick={() => setOpen((v) => !v)}
        className="w-full justify-between rounded-none px-2 text-[11px] text-muted-foreground"
        data-testid="graph-legend-toggle"
      >
        {copy.legend.title}
        <ChevronDownIcon
          className={cn("transition-transform duration-[var(--dur-fast)]", open ? "" : "-rotate-90 rtl:rotate-90")}
        />
      </Button>
      {open ? (
        <div className="flex flex-col gap-2 px-2 pt-0.5 pb-2">
          <ul className="flex flex-col gap-1" aria-label={copy.legend.sources}>
            {SOURCES.map((source) => (
              <li key={source} className="flex items-center gap-2">
                <span aria-hidden className={cn("size-2.5", SOURCE_BG[source])} />
                {copy.legend.sourceNames[source]}
              </li>
            ))}
          </ul>
          <ul className="flex flex-col gap-1 border-t border-border pt-2" aria-label={copy.legend.kinds}>
            {(["claim", "concept", "procedure", "decision"] as const).map((kind) => (
              <li key={kind} className="flex items-center gap-2">
                <span aria-hidden className={cn("relative size-2.5 shrink-0", KIND_SWATCH[kind])} />
                {copy.legend[kind]}
              </li>
            ))}
          </ul>
          <p className="flex items-center gap-2 border-t border-border pt-2 text-muted-foreground">
            <span aria-hidden className="h-px w-2.5 shrink-0 bg-muted-foreground" />
            {copy.legend.chain}
          </p>
        </div>
      ) : null}
    </section>
  )
}
