import { Fragment } from "react"

import { Kbd } from "@/components/app/kbd"
import { splitBinding } from "@/lib/keys"

/** Renders a registry binding: `mod+k` as key caps, `g l` as "G then L". */
export function ShortcutKeys({
  keys,
  className,
}: {
  keys: string
  className?: string
}) {
  const steps = splitBinding(keys)
  return (
    <span className="inline-flex items-center gap-1" dir="ltr">
      {steps.map((combo, i) => (
        <Fragment key={`${i}:${combo.join("+")}`}>
          {i > 0 ? (
            <span className="text-[10px] text-muted-foreground">then</span>
          ) : null}
          <Kbd combo={combo.join("+")} className={className} />
        </Fragment>
      ))}
    </span>
  )
}
